//! The account's Durable Object: device registry + sync hub, with WebSocket
//! hibernation so idle sockets cost nothing.

use kabar_protocol::DeviceId;
use kabar_protocol::cloud::{DeviceList, ErrorCode};
use kabar_protocol::sync::ServerMsg;
use worker::{
    DurableObject, Env, Method, Request, Response, Result, State, WebSocket, WebSocketIncomingMessage,
    WebSocketPair, console_error, durable_object,
};

use super::do_sql::DoSql;
use super::{api_error, now_ms, random_hex};
use crate::devices::{Devices, RefreshError};
use crate::hub::{Conn, Hub};
use crate::internal::{DEVICE_HEADER, Expiry, Refresh, Register, Revoke};
use crate::sql::Sql;

/// Close code sent to a signed-out device.
const CLOSE_REVOKED: u16 = 4001;

#[durable_object]
pub struct Account {
    state: State,
}

impl Account {
    fn db(&self) -> DoSql {
        DoSql(self.state.storage().sql())
    }

    fn send(ws: &WebSocket, msg: &ServerMsg) {
        if let Err(e) = ws.send(msg) {
            console_error!("send failed: {e}");
        }
    }

    fn close_device(&self, device: &DeviceId) {
        for ws in self.state.get_websockets_with_tag(&device.0) {
            Self::send(&ws, &ServerMsg::Revoked);
            let _ = ws.close(Some(CLOSE_REVOKED), Some("signed out"));
        }
    }

    fn device_header(req: &Request) -> Result<DeviceId> {
        req.headers()
            .get(DEVICE_HEADER)?
            .map(DeviceId)
            .ok_or_else(|| worker::Error::RustError("missing device header".into()))
    }

    fn accept_socket(&self, req: &Request) -> Result<Response> {
        let device = Self::device_header(req)?;
        let db = self.db();
        if !Devices::open(&db).is_active(&device).map_err(sql_err)? {
            return api_error(ErrorCode::DeviceRevoked, "this device was signed out");
        }
        let pair = WebSocketPair::new()?;
        let conn = Conn { device: device.clone(), conn_id: random_hex(8), ready: false };
        self.state.accept_websocket_with_tags(&pair.server, &[&device.0]);
        pair.server.serialize_attachment(&conn)?;
        Devices::open(&db).touch(&device, now_ms(), false).map_err(sql_err)?;
        Response::from_websocket(pair.client)
    }
}

fn sql_err(e: crate::sql::SqlError) -> worker::Error {
    worker::Error::RustError(e.to_string())
}

impl DurableObject for Account {
    fn new(state: State, _env: Env) -> Self {
        let db = DoSql(state.storage().sql());
        // Synchronous, so it finishes before any event is delivered.
        if let Err(e) = db.exec_batch(crate::devices::SCHEMA).and_then(|_| db.exec_batch(crate::hub::SCHEMA))
        {
            console_error!("migration failed: {e}");
        }
        Self { state }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let db = self.db();
        let devices = Devices::open(&db);
        let now = now_ms();
        match (req.method(), req.path().as_str()) {
            (Method::Get, "/ws") => self.accept_socket(&req),
            (Method::Post, "/register") => {
                let body: Register = req.json().await?;
                let expires =
                    devices.register(&body.info, &body.email, &body.refresh_hash, now).map_err(sql_err)?;
                Response::from_json(&Expiry { refresh_expires_ms: expires, email: body.email })
            }
            (Method::Post, "/refresh") => {
                let body: Refresh = req.json().await?;
                match devices.refresh(&body.device, &body.presented_hash, &body.new_hash, now) {
                    Ok(r) => {
                        Response::from_json(&Expiry { refresh_expires_ms: r.expires_ms, email: r.email })
                    }
                    Err(RefreshError::Sql(e)) => Err(sql_err(e)),
                    Err(e @ (RefreshError::Revoked | RefreshError::Reused)) => {
                        self.close_device(&body.device);
                        api_error(ErrorCode::DeviceRevoked, e.to_string())
                    }
                    Err(e) => api_error(ErrorCode::Unauthorized, e.to_string()),
                }
            }
            (Method::Get, "/devices") => {
                let current = Self::device_header(&req)?;
                let list = devices.list(&current).map_err(sql_err)?;
                Response::from_json(&DeviceList { devices: list })
            }
            (Method::Get, "/active") => {
                let device = Self::device_header(&req)?;
                if devices.is_active(&device).map_err(sql_err)? {
                    Response::empty()
                } else {
                    api_error(ErrorCode::DeviceRevoked, "this device was signed out")
                }
            }
            (Method::Post, "/revoke") => {
                let body: Revoke = req.json().await?;
                if devices.revoke(&body.device, now).map_err(sql_err)? {
                    self.close_device(&body.device);
                    Response::empty()
                } else {
                    api_error(ErrorCode::NotFound, "no such device")
                }
            }
            _ => api_error(ErrorCode::NotFound, "not found"),
        }
    }

    async fn websocket_message(&self, ws: WebSocket, message: WebSocketIncomingMessage) -> Result<()> {
        let WebSocketIncomingMessage::String(text) = message else {
            Self::send(
                &ws,
                &ServerMsg::Error {
                    code: kabar_protocol::sync::SyncErrorCode::BadMessage,
                    message: "text frames only".into(),
                },
            );
            return Ok(());
        };
        let Some(mut conn) = ws.deserialize_attachment::<Conn>()? else {
            let _ = ws.close(Some(1011), Some("lost connection state"));
            return Ok(());
        };
        let db = self.db();
        let devices = Devices::open(&db);
        if !devices.is_active(&conn.device).map_err(sql_err)? {
            Self::send(&ws, &ServerMsg::Revoked);
            let _ = ws.close(Some(CLOSE_REVOKED), Some("signed out"));
            return Ok(());
        }
        let was_ready = conn.ready;
        let now = now_ms();
        let outcome = Hub::open(&db).handle(&mut conn, &text, now).map_err(sql_err)?;
        if conn.ready != was_ready {
            ws.serialize_attachment(&conn)?;
        }
        for reply in &outcome.replies {
            Self::send(&ws, reply);
        }
        if !outcome.broadcast.is_empty() {
            let msg = ServerMsg::Ops { ops: outcome.broadcast, more: false };
            for other in self.state.get_websockets() {
                let Ok(Some(c)) = other.deserialize_attachment::<Conn>() else { continue };
                if c.ready && c.conn_id != conn.conn_id {
                    Self::send(&other, &msg);
                }
            }
        }
        devices.touch(&conn.device, now, outcome.synced).map_err(sql_err)?;
        Ok(())
    }

    async fn websocket_close(
        &self,
        ws: WebSocket,
        code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        // Complete the closing handshake. 1005/1006 can't be sent back.
        let code =
            if (1000..=4999).contains(&code) && code != 1005 && code != 1006 { code as u16 } else { 1000 };
        let _ = ws.close(Some(code), Some("bye"));
        Ok(())
    }

    async fn websocket_error(&self, _ws: WebSocket, error: worker::Error) -> Result<()> {
        console_error!("websocket error: {error}");
        Ok(())
    }
}
