//! The account's Durable Object: device registry + sync hub, with WebSocket
//! hibernation so idle sockets cost nothing, plus Kak Nur's reminder alarm.

use std::cell::RefCell;
use std::time::Duration;

use kabar_protocol::DeviceId;
use kabar_protocol::cloud::{DeviceList, ErrorCode, PushTokenReply};
use kabar_protocol::sync::{SeqOp, ServerMsg};
use worker::{
    DurableObject, Env, Headers, Method, Request, Response, Result, State, WebSocket,
    WebSocketIncomingMessage, WebSocketPair, console_error, console_log, durable_object,
};

use super::do_sql::DoSql;
use super::{api_error, now_ms, post, post_json, random_hex};
use crate::blobs::{self, Image};
use crate::coach::{self, KakNur};
use crate::devices::{Devices, PushTarget, RefreshError};
use crate::fcm::{self, AccessToken, SendOutcome, ServiceAccount};
use crate::hub::{Conn, Hub};
use crate::internal::{
    BlobData, BlobGet, BlobPut, DEVICE_HEADER, Expiry, Refresh, Register, Revoke, SetPush,
};
use crate::sql::Sql;

/// Close code sent to a signed-out device.
const CLOSE_REVOKED: u16 = 4001;

thread_local! {
    /// Google access token for FCM, per service account, cached ~55 minutes.
    static FCM_TOKEN: RefCell<Option<(String, AccessToken)>> = const { RefCell::new(None) };
}

#[durable_object]
pub struct Account {
    state: State,
    env: Env,
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

    /// Sends stored ops to every ready socket except `except` (a conn id).
    fn broadcast(&self, ops: Vec<SeqOp>, except: Option<&str>) {
        if ops.is_empty() {
            return;
        }
        let msg = ServerMsg::Ops { ops, more: false };
        for other in self.state.get_websockets() {
            let Ok(Some(c)) = other.deserialize_attachment::<Conn>() else { continue };
            if c.ready && Some(c.conn_id.as_str()) != except {
                Self::send(&other, &msg);
            }
        }
    }

    fn service_account(&self) -> Option<ServiceAccount> {
        let json = self.env.secret("FCM_SERVICE_ACCOUNT").ok()?.to_string();
        if json.trim().is_empty() {
            return None;
        }
        ServiceAccount::from_json(&json).map_err(|e| console_error!("{e}")).ok()
    }

    /// Arms the reminder alarm unless one is pending. Each alarm re-arms the
    /// next while some device can get pushes (at most 96 a day).
    async fn ensure_alarm(&self) -> Result<()> {
        let storage = self.state.storage();
        if storage.get_alarm().await?.is_none() {
            let now = now_ms();
            storage.set_alarm(Duration::from_millis(coach::next_tick_ms(now) - now)).await?;
        }
        Ok(())
    }

    async fn fcm_token(sa: &ServiceAccount, now: u64) -> Result<String> {
        let cached = FCM_TOKEN.with_borrow(|c| {
            c.as_ref()
                .filter(|(email, t)| *email == sa.client_email && t.use_until_ms > now)
                .map(|(_, t)| t.token.clone())
        });
        if let Some(t) = cached {
            return Ok(t);
        }
        let assertion = sa.assertion(now).map_err(js_err)?;
        let headers = Headers::new();
        headers.set("content-type", "application/x-www-form-urlencoded")?;
        let (status, _, body) = post(fcm::TOKEN_URL, headers, &fcm::token_request_body(&assertion)).await?;
        let token = fcm::parse_token_response(status, &body, now).map_err(js_err)?;
        let out = token.token.clone();
        FCM_TOKEN.with_borrow_mut(|c| *c = Some((sa.client_email.clone(), token)));
        Ok(out)
    }

    /// One push to one device.
    async fn send_push(
        sa: &ServiceAccount,
        target: &PushTarget,
        due: &coach::Due,
        now: u64,
    ) -> Result<SendOutcome> {
        let token = Self::fcm_token(sa, now).await?;
        let headers = Headers::new();
        headers.set("authorization", &format!("Bearer {token}"))?;
        let (status, _, body) =
            post_json(&sa.send_url(), headers, &fcm::message(&target.token, due.notice())).await?;
        if status == 401 {
            // Revoked early: mint a fresh one next time.
            FCM_TOKEN.with_borrow_mut(|c| *c = None);
        }
        Ok(fcm::classify_send(status, &body))
    }

    /// The alarm's work: decide, push to every device, record what went out.
    async fn remind(&self, sa: &ServiceAccount, targets: &[PushTarget], now: u64) -> Result<()> {
        let db = self.db();
        let tick = coach::tick(&db, &KakNur, now).map_err(sql_err)?;
        if let Some(why) = tick.silence {
            console_log!("reminder: silent ({why:?})");
        }
        for due in &tick.due {
            let mut delivered = false;
            for target in targets {
                match Self::send_push(sa, target, due, now).await {
                    Ok(SendOutcome::Sent) => delivered = true,
                    Ok(SendOutcome::Unregistered) => {
                        Devices::open(&db).drop_push_token(&target.device, &target.token).map_err(sql_err)?;
                    }
                    Ok(SendOutcome::Failed(why)) => console_error!("fcm to {}: {why}", target.device),
                    Err(e) => console_error!("fcm to {}: {e}", target.device),
                }
            }
            if delivered && let Some(op) = coach::commit(&db, &KakNur, due, now_ms()).map_err(sql_err)? {
                self.broadcast(vec![op], None);
            }
        }
        Ok(())
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

fn js_err(e: impl std::fmt::Display) -> worker::Error {
    worker::Error::RustError(e.to_string())
}

impl DurableObject for Account {
    fn new(state: State, env: Env) -> Self {
        let db = DoSql(state.storage().sql());
        // Synchronous, so it finishes before any event is delivered.
        if let Err(e) = crate::devices::migrate(&db)
            .and_then(|_| db.exec_batch(crate::hub::SCHEMA))
            .and_then(|_| db.exec_batch(blobs::SCHEMA))
        {
            console_error!("migration failed: {e}");
        }
        Self { state, env }
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
                self.ensure_alarm().await?;
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
            (Method::Post, "/push") => {
                let device = Self::device_header(&req)?;
                let body: SetPush = req.json().await?;
                if !devices.set_push_token(&device, body.token.as_deref()).map_err(sql_err)? {
                    return api_error(ErrorCode::DeviceRevoked, "this device was signed out");
                }
                let push_configured = self.service_account().is_some();
                if body.token.is_some() && push_configured {
                    self.ensure_alarm().await?;
                }
                Response::from_json(&PushTokenReply { registered: body.token.is_some(), push_configured })
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
            (Method::Post, "/blob/put") => {
                let body: BlobPut = req.json().await?;
                let image = Image { mime: body.mime, base64: body.data_b64 };
                blobs::put(&db, &body.sha256, &image, now).map_err(sql_err)?;
                Response::empty()
            }
            (Method::Post, "/blob/get") => {
                let body: BlobGet = req.json().await?;
                match blobs::get(&db, &body.sha256, now).map_err(sql_err)? {
                    Some(img) => Response::from_json(&BlobData { mime: img.mime, data_b64: img.base64 }),
                    None => api_error(ErrorCode::BlobMissing, "upload the image again"),
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
        self.broadcast(outcome.broadcast, Some(&conn.conn_id));
        devices.touch(&conn.device, now, outcome.synced).map_err(sql_err)?;
        Ok(())
    }

    /// Kak Nur's 15-minute tick. Stops re-arming while no device has a push
    /// token or Firebase isn't set up; the next `PUT /v1/devices/me/push`
    /// arms it again, so an idle account costs nothing.
    async fn alarm(&self) -> Result<Response> {
        let now = now_ms();
        let targets = Devices::open(&self.db()).push_targets().map_err(sql_err)?;
        let Some(sa) = self.service_account().filter(|_| !targets.is_empty()) else {
            return Response::empty();
        };
        // Re-arm first, so a failure below never stops the reminders.
        self.state.storage().set_alarm(Duration::from_millis(coach::next_tick_ms(now) - now)).await?;
        if let Err(e) = self.remind(&sa, &targets, now).await {
            console_error!("reminder tick: {e}");
        }
        Response::empty()
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
