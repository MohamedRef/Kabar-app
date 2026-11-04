//! A local stand-in for Kabar Cloud, for the sync engine's tests: plain HTTP
//! for the API and a WebSocket on `/v1/sync`, on 127.0.0.1. The WebSocket
//! runs Kabar Cloud's real sync hub (`kabar_server::hub`) on in-memory
//! SQLite; sign-in, tokens and devices are a small fake with the server's
//! answers and error codes. Nothing leaves the machine.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use kabar_protocol::DeviceId;
use kabar_protocol::cloud::{
    ApiError, DeviceInfo, DeviceList, DeviceSummary, ErrorCode, GoogleSignIn, PushTokenReply,
    PushTokenRequest, RefreshRequest, Session, UsageReport,
};
use kabar_protocol::cloud::{PasswordLogin, PasswordSignIn, SetPassword, normalize_username, valid_username};
use kabar_protocol::sync::{ServerMsg, SyncErrorCode};
use kabar_server::hub::{Conn, Hub};
use kabar_server::login::{Checked, Login};
use kabar_server::sql::{Row, Sql, SqlError, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

/// The hub's storage: in-memory SQLite behind the server's `Sql` trait.
struct MemSql(rusqlite::Connection);

impl Sql for MemSql {
    fn exec(&self, sql: &str, params: &[Value]) -> Result<Vec<Row>, SqlError> {
        use rusqlite::types::{Value as V, ValueRef};
        let err = |e: rusqlite::Error| SqlError(e.to_string());
        let mut stmt = self.0.prepare(sql).map_err(err)?;
        let params: Vec<V> = params
            .iter()
            .map(|p| match p {
                Value::Null => V::Null,
                Value::Int(i) => V::Integer(*i),
                Value::Real(f) => V::Real(*f),
                Value::Text(s) => V::Text(s.clone()),
            })
            .collect();
        let n = stmt.column_count();
        let mut rows = stmt.query(rusqlite::params_from_iter(params)).map_err(err)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            let mut r = Vec::with_capacity(n);
            for i in 0..n {
                r.push(match row.get_ref(i).map_err(err)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(i) => Value::Int(i),
                    ValueRef::Real(f) => Value::Real(f),
                    ValueRef::Text(t) => Value::Text(String::from_utf8_lossy(t).into_owned()),
                    ValueRef::Blob(_) => return Err(SqlError("blobs are not used".into())),
                });
            }
            out.push(r);
        }
        Ok(out)
    }
}

struct Device {
    info: DeviceInfo,
    signed_in_at_ms: u64,
    last_sync_ms: Option<u64>,
    revoked: bool,
    push_token: Option<String>,
}

struct Socket {
    device: DeviceId,
    ready: Arc<AtomicBool>,
    tx: UnboundedSender<Message>,
}

#[derive(Default)]
struct Tokens {
    /// access token → device
    access: BTreeMap<String, DeviceId>,
    /// refresh token → device
    refresh: BTreeMap<String, DeviceId>,
}

/// Switches a test flips to make the cloud misbehave.
#[derive(Default)]
pub struct Knobs {
    /// Hello is answered with `unsupported_protocol`.
    pub outdated: AtomicBool,
    /// Access tokens are issued already expired (every call refreshes).
    pub short_tokens: AtomicBool,
    /// The WebSocket handshake answers 401 this many more times.
    pub refuse_sockets: AtomicU64,
    /// Every HTTP call but sign-in answers this status (0: off).
    pub http_status: AtomicU64,
    /// The status a refused handshake answers (0: 401).
    pub socket_status: AtomicU64,
    /// Pushes are acked with every op rejected for good.
    pub reject_pushes: AtomicBool,
}

pub struct State {
    hub: Mutex<MemSql>,
    /// Each username's own store, like its Durable Object, and the
    /// account's username.
    logins: Mutex<BTreeMap<String, MemSql>>,
    username: Mutex<Option<String>>,
    tokens: Mutex<Tokens>,
    devices: Mutex<BTreeMap<DeviceId, Device>>,
    sockets: Mutex<Vec<Socket>>,
    next: AtomicU64,
    pub knobs: Knobs,
    /// Every HTTP request, as `"METHOD /path"`.
    pub requests: Mutex<Vec<String>>,
    /// Blobs uploaded, by name.
    pub blobs: Mutex<BTreeMap<String, Vec<u8>>>,
}

pub struct FakeCloud {
    pub url: String,
    pub state: Arc<State>,
}

impl FakeCloud {
    /// Starts the server on its own thread; it lives until the test ends.
    pub fn start() -> Self {
        let db = MemSql(rusqlite::Connection::open_in_memory().unwrap());
        Hub::new(&db).unwrap();
        let state = Arc::new(State {
            hub: Mutex::new(db),
            logins: Mutex::default(),
            username: Mutex::default(),
            tokens: Mutex::default(),
            devices: Mutex::default(),
            sockets: Mutex::default(),
            next: AtomicU64::new(1),
            knobs: Knobs::default(),
            requests: Mutex::default(),
            blobs: Mutex::default(),
        });
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let shared = state.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async move {
                let listener = TcpListener::from_std(listener).unwrap();
                loop {
                    let Ok((stream, _)) = listener.accept().await else { continue };
                    tokio::spawn(serve(stream, shared.clone()));
                }
            });
        });
        Self { url, state }
    }

    /// An ID token that signs in `email`.
    pub fn id_token(email: &str) -> String {
        format!("ok:{email}")
    }

    /// Ops in the hub's log.
    pub fn head_seq(&self) -> u64 {
        Hub::open(&*self.state.hub.lock().unwrap()).head_seq().unwrap()
    }

    /// The hub's current payload of a row.
    pub fn row(&self, table: &str, row_id: &str) -> Option<serde_json::Value> {
        Hub::open(&*self.state.hub.lock().unwrap()).current_payload(table, row_id).unwrap()
    }

    /// Closes every socket of `device` with `code`, like the hub does.
    pub fn close_sockets(&self, device: &DeviceId, code: u16) {
        for s in self.state.sockets.lock().unwrap().iter().filter(|s| &s.device == device) {
            let frame = CloseFrame { code: code.into(), reason: "".into() };
            s.tx.send(Message::Close(Some(frame))).ok();
        }
    }

    /// How many sockets are open now.
    pub fn sockets(&self) -> usize {
        let mut sockets = self.state.sockets.lock().unwrap();
        sockets.retain(|s| !s.tx.is_closed());
        sockets.len()
    }

    /// Sends `text` as is to every socket of `device`.
    pub fn send_text(&self, device: &DeviceId, text: &str) {
        for s in self.state.sockets.lock().unwrap().iter().filter(|s| &s.device == device) {
            s.tx.send(Message::text(text.to_string())).ok();
        }
    }

    /// Signs `device` out on the cloud's side only (its sockets stay open).
    pub fn revoke(&self, device: &DeviceId) {
        self.state.revoke(device);
    }

    pub fn push_token(&self, device: &DeviceId) -> Option<String> {
        self.state.devices.lock().unwrap().get(device).and_then(|d| d.push_token.clone())
    }

    /// Drops every access token (refresh tokens stay), like a signing-key
    /// rotation: the next call is refused until the device refreshes.
    pub fn forget_access_tokens(&self) {
        self.state.tokens.lock().unwrap().access.clear();
    }

    pub fn requested(&self, what: &str) -> usize {
        self.state.requests.lock().unwrap().iter().filter(|r| *r == what).count()
    }
}

fn now_ms() -> u64 {
    super::engine::now_ms()
}

impl State {
    fn issue(&self, device: &DeviceId, email: Option<String>) -> Session {
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        let now = now_ms();
        let access_token = format!("a{n}-{}", device.0);
        let refresh_token = format!("r{n}-{}", device.0);
        let mut tokens = self.tokens.lock().unwrap();
        tokens.access.insert(access_token.clone(), device.clone());
        tokens.refresh.insert(refresh_token.clone(), device.clone());
        let short = self.knobs.short_tokens.load(Ordering::SeqCst);
        Session {
            account_id: "acc".into(),
            device_id: device.clone(),
            email,
            access_token,
            access_expires_at_ms: if short { now } else { now + 15 * 60_000 },
            refresh_token,
            refresh_expires_at_ms: now + 30 * 86_400_000,
        }
    }

    /// The device a bearer token belongs to, if it is still signed in.
    fn device_of(&self, access: Option<&str>) -> Result<DeviceId, (u16, ErrorCode)> {
        let token = access.and_then(|a| a.strip_prefix("Bearer ")).ok_or((401, ErrorCode::Unauthorized))?;
        let device =
            self.tokens.lock().unwrap().access.get(token).cloned().ok_or((401, ErrorCode::Unauthorized))?;
        match self.devices.lock().unwrap().get(&device) {
            Some(d) if !d.revoked => Ok(device),
            _ => Err((401, ErrorCode::DeviceRevoked)),
        }
    }

    fn revoke(&self, device: &DeviceId) {
        if let Some(d) = self.devices.lock().unwrap().get_mut(device) {
            d.revoked = true;
            d.push_token = None;
        }
        let mut tokens = self.tokens.lock().unwrap();
        tokens.access.retain(|_, d| d != device);
        tokens.refresh.retain(|_, d| d != device);
    }

    /// One HTTP request: `(status, JSON body)`.
    fn route(&self, method: &str, path: &str, auth: Option<&str>, body: &[u8]) -> (u16, Vec<u8>) {
        let json = |status: u16, v: &dyn erased::Json| (status, v.to_json());
        let fail = |status: u16, code: ErrorCode| {
            json(status, &ApiError { code, message: format!("{code:?}"), retry_after_ms: None })
        };
        let forced = self.knobs.http_status.load(Ordering::SeqCst) as u16;
        if forced != 0 && path != "/v1/auth/google" {
            return (forced, b"<html>bad gateway</html>".to_vec());
        }
        match (method, path) {
            ("POST", "/v1/auth/google") => {
                let Ok(req) = serde_json::from_slice::<GoogleSignIn>(body) else {
                    return fail(400, ErrorCode::InvalidRequest);
                };
                if req.id_token == "boom" {
                    return fail(500, ErrorCode::Internal);
                }
                let Some(email) = req.id_token.strip_prefix("ok:") else {
                    return fail(403, ErrorCode::NotAllowed);
                };
                let id = req.device.device_id.clone();
                self.devices.lock().unwrap().insert(
                    id.clone(),
                    Device {
                        info: req.device,
                        signed_in_at_ms: now_ms(),
                        last_sync_ms: None,
                        revoked: false,
                        push_token: None,
                    },
                );
                json(200, &self.issue(&id, Some(email.to_string())))
            }
            ("POST", "/v1/auth/password") => {
                let Ok(req) = serde_json::from_slice::<PasswordSignIn>(body) else {
                    return fail(400, ErrorCode::InvalidRequest);
                };
                let username = normalize_username(&req.username);
                let key = kabar_server::login::parse_key(&req.password_key);
                let logins = self.logins.lock().unwrap();
                let (Some(db), Some(key), true) = (logins.get(&username), key, valid_username(&username))
                else {
                    return fail(401, ErrorCode::WrongPassword);
                };
                match Login::new(db).unwrap().check(&key, now_ms()).unwrap() {
                    Checked::Ok { email, .. } => {
                        drop(logins);
                        let id = req.device.device_id.clone();
                        self.devices.lock().unwrap().insert(
                            id.clone(),
                            Device {
                                info: req.device,
                                signed_in_at_ms: now_ms(),
                                last_sync_ms: None,
                                revoked: false,
                                push_token: None,
                            },
                        );
                        json(200, &self.issue(&id, Some(email)))
                    }
                    Checked::Wrong => fail(401, ErrorCode::WrongPassword),
                    Checked::Locked { retry_after_ms } => json(
                        429,
                        &ApiError {
                            code: ErrorCode::TooManyTries,
                            message: "too many wrong passwords".into(),
                            retry_after_ms: Some(retry_after_ms),
                        },
                    ),
                }
            }
            ("POST", "/v1/auth/refresh") => {
                let Ok(req) = serde_json::from_slice::<RefreshRequest>(body) else {
                    return fail(400, ErrorCode::InvalidRequest);
                };
                let device = self.tokens.lock().unwrap().refresh.remove(&req.refresh_token);
                let Some(device) = device else { return fail(401, ErrorCode::DeviceRevoked) };
                let email =
                    self.devices.lock().unwrap().get(&device).map(|_| "learner@example.com".to_string());
                json(200, &self.issue(&device, email))
            }
            _ => {
                let device = match self.device_of(auth) {
                    Ok(d) => d,
                    Err((status, code)) => return fail(status, code),
                };
                self.authed(method, path, &device, body)
            }
        }
    }

    fn authed(&self, method: &str, path: &str, me: &DeviceId, body: &[u8]) -> (u16, Vec<u8>) {
        let json = |status: u16, v: &dyn erased::Json| (status, v.to_json());
        let fail = |status: u16, code: ErrorCode| {
            json(status, &ApiError { code, message: format!("{code:?}"), retry_after_ms: None })
        };
        match (method, path) {
            ("POST", "/v1/auth/signout") => {
                self.revoke(me);
                (204, Vec::new())
            }
            ("GET", "/v1/devices") => {
                let devices = self.devices.lock().unwrap();
                let list = DeviceList {
                    devices: devices
                        .values()
                        .map(|d| DeviceSummary {
                            device_id: d.info.device_id.clone(),
                            name: d.info.name.clone(),
                            model: d.info.model.clone(),
                            platform: d.info.platform,
                            signed_in_at_ms: d.signed_in_at_ms,
                            last_seen_ms: d.signed_in_at_ms,
                            last_sync_ms: d.last_sync_ms,
                            current: &d.info.device_id == me,
                            revoked: d.revoked,
                        })
                        .collect(),
                };
                json(200, &list)
            }
            ("GET", "/v1/login") => {
                json(200, &PasswordLogin { username: self.username.lock().unwrap().clone() })
            }
            ("POST", "/v1/login") => {
                let Ok(req) = serde_json::from_slice::<SetPassword>(body) else {
                    return fail(400, ErrorCode::InvalidRequest);
                };
                let username = normalize_username(&req.username);
                let key = kabar_server::login::parse_key(&req.password_key);
                let (true, Some(key)) = (valid_username(&username), key) else {
                    return fail(400, ErrorCode::InvalidRequest);
                };
                let salt = format!("{:032x}", self.next.fetch_add(1, Ordering::SeqCst));
                let verifier = kabar_server::login::verifier(&salt, &key);
                let mut logins = self.logins.lock().unwrap();
                let db = logins
                    .entry(username.clone())
                    .or_insert_with(|| MemSql(rusqlite::Connection::open_in_memory().unwrap()));
                if !Login::new(db)
                    .unwrap()
                    .set("acc", "learner@example.com", &salt, &verifier, now_ms())
                    .unwrap()
                {
                    return fail(409, ErrorCode::UsernameTaken);
                }
                let mut current = self.username.lock().unwrap();
                if let Some(old) = current.take().filter(|old| *old != username)
                    && let Some(db) = logins.get(&old)
                {
                    Login::new(db).unwrap().clear("acc").unwrap();
                }
                *current = Some(username);
                json(200, &PasswordLogin { username: current.clone() })
            }
            ("DELETE", "/v1/login") => {
                if let Some(old) = self.username.lock().unwrap().take()
                    && let Some(db) = self.logins.lock().unwrap().get(&old)
                {
                    Login::new(db).unwrap().clear("acc").unwrap();
                }
                json(200, &PasswordLogin { username: None })
            }
            ("GET", "/v1/usage") => {
                let report = UsageReport {
                    services: Vec::new(),
                    platform: Vec::new(),
                    spend_usd_cents: 0,
                    hard_stops: true,
                    generated_at_ms: 1_762_000_000_000,
                };
                json(200, &report)
            }
            ("PUT", "/v1/devices/me/push") => {
                let Ok(req) = serde_json::from_slice::<PushTokenRequest>(body) else {
                    return fail(400, ErrorCode::InvalidRequest);
                };
                let registered = !req.token.is_empty();
                if let Some(d) = self.devices.lock().unwrap().get_mut(me) {
                    d.push_token = registered.then_some(req.token);
                }
                json(200, &PushTokenReply { registered, push_configured: true })
            }
            ("POST", "/v1/ai") => (
                503,
                serde_json::to_vec(&kabar_protocol::ai::EngineError::Unavailable {
                    reason: "no Gemini key on this Kabar Cloud".into(),
                })
                .unwrap(),
            ),
            ("POST", "/v1/ai/reject") => (204, Vec::new()),
            ("POST", "/v1/tts") => fail(501, ErrorCode::NotConfigured),
            _ if method == "PUT" && path.starts_with("/v1/blobs/") => {
                self.blobs.lock().unwrap().insert(path["/v1/blobs/".len()..].to_string(), body.to_vec());
                (204, Vec::new())
            }
            _ if method == "DELETE" && path.starts_with("/v1/devices/") => {
                let target = DeviceId(path["/v1/devices/".len()..].to_string());
                if !self.devices.lock().unwrap().contains_key(&target) {
                    return fail(404, ErrorCode::NotFound);
                }
                self.revoke(&target);
                // Like the hub: tell its sockets, then close them with 4001.
                for s in self.sockets.lock().unwrap().iter().filter(|s| s.device == target) {
                    s.tx.send(Message::text(serde_json::to_string(&ServerMsg::Revoked).unwrap())).ok();
                    let frame = CloseFrame { code: 4001.into(), reason: "revoked".into() };
                    s.tx.send(Message::Close(Some(frame))).ok();
                }
                (204, Vec::new())
            }
            _ => fail(404, ErrorCode::NotFound),
        }
    }
}

/// Lets `route` serialize any reply type through one closure.
mod erased {
    pub trait Json {
        fn to_json(&self) -> Vec<u8>;
    }
    impl<T: serde::Serialize> Json for T {
        fn to_json(&self) -> Vec<u8> {
            serde_json::to_vec(self).unwrap()
        }
    }
}

/// Reads the request head without consuming it, so a WebSocket handshake can
/// still read it.
async fn peek_head(stream: &TcpStream) -> Option<String> {
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let n = stream.peek(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        let text = String::from_utf8_lossy(&buf[..n]).into_owned();
        if text.contains("\r\n\r\n") {
            return Some(text);
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines()
        .skip(1)
        .take_while(|l| !l.is_empty())
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim()))
}

async fn serve(mut stream: TcpStream, state: Arc<State>) {
    let Some(head) = peek_head(&stream).await else { return };
    let mut parts = head.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    if path == "/v1/sync" {
        return socket(stream, state).await;
    }
    state.requests.lock().unwrap().push(format!("{method} {path}"));
    let head_len = head.find("\r\n\r\n").unwrap() + 4;
    let body_len: usize = header(&head, "content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut raw = vec![0u8; head_len + body_len];
    if stream.read_exact(&mut raw).await.is_err() {
        return;
    }
    let auth = header(&head, "authorization").map(str::to_string);
    let (status, body) = state.route(&method, &path, auth.as_deref(), &raw[head_len..]);
    let reply = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes()).await;
    let _ = stream.write_all(&body).await;
    let _ = stream.shutdown().await;
}

// The handshake callback's error type is fixed by tungstenite.
#[allow(clippy::result_large_err)]
async fn socket(stream: TcpStream, state: Arc<State>) {
    let mut device = None;
    let check = |req: &Request, res: Response| -> Result<Response, ErrorResponse> {
        let refuse = state.knobs.refuse_sockets.load(Ordering::SeqCst);
        let auth = req.headers().get("authorization").and_then(|v| v.to_str().ok());
        match state.device_of(auth) {
            Ok(d) if refuse == 0 => {
                device = Some(d);
                Ok(res)
            }
            _ => {
                state.knobs.refuse_sockets.store(refuse.saturating_sub(1), Ordering::SeqCst);
                let mut no = ErrorResponse::new(Some("refused".into()));
                let status = match state.knobs.socket_status.load(Ordering::SeqCst) {
                    0 => 401,
                    s => s as u16,
                };
                *no.status_mut() =
                    tokio_tungstenite::tungstenite::http::StatusCode::from_u16(status).unwrap();
                Err(no)
            }
        }
    };
    let Ok(ws) = tokio_tungstenite::accept_hdr_async(stream, check).await else { return };
    let device = device.unwrap();
    let (mut sink, mut source) = ws.split();
    let (tx, mut rx) = unbounded_channel::<Message>();
    let ready = Arc::new(AtomicBool::new(false));
    state.sockets.lock().unwrap().push(Socket {
        device: device.clone(),
        ready: ready.clone(),
        tx: tx.clone(),
    });
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let close = matches!(msg, Message::Close(_));
            if sink.send(msg).await.is_err() || close {
                break;
            }
        }
    });
    let mut conn = Conn {
        device: device.clone(),
        conn_id: format!("c{}", state.next.fetch_add(1, Ordering::SeqCst)),
        ready: false,
    };
    while let Some(Ok(msg)) = source.next().await {
        let Message::Text(text) = msg else { continue };
        if state.knobs.outdated.load(Ordering::SeqCst) && text.contains("\"hello\"") {
            let msg = ServerMsg::Error {
                code: SyncErrorCode::UnsupportedProtocol,
                message: "update the app".into(),
            };
            tx.send(Message::text(serde_json::to_string(&msg).unwrap())).ok();
            continue;
        }
        if state.knobs.reject_pushes.load(Ordering::SeqCst)
            && let Ok(kabar_protocol::sync::ClientMsg::Push { batch, ops }) = serde_json::from_str(&text)
        {
            let rejected = ops
                .iter()
                .map(|o| kabar_protocol::sync::Rejected {
                    op_id: o.id.clone(),
                    reason: kabar_protocol::sync::RejectReason::PayloadTooLarge,
                })
                .collect();
            let ack = ServerMsg::Ack { batch, done: vec![], rejected, head_seq: 0 };
            tx.send(Message::text(serde_json::to_string(&ack).unwrap())).ok();
            continue;
        }
        let outcome = {
            let db = state.hub.lock().unwrap();
            Hub::open(&*db).handle(&mut conn, &text, now_ms()).unwrap()
        };
        ready.store(conn.ready, Ordering::SeqCst);
        if outcome.synced
            && let Some(d) = state.devices.lock().unwrap().get_mut(&device)
        {
            d.last_sync_ms = Some(now_ms());
        }
        for reply in &outcome.replies {
            tx.send(Message::text(serde_json::to_string(reply).unwrap())).ok();
        }
        if !outcome.broadcast.is_empty() {
            let page = ServerMsg::Ops { ops: outcome.broadcast, more: false };
            let text = serde_json::to_string(&page).unwrap();
            for s in state.sockets.lock().unwrap().iter() {
                if !Arc::ptr_eq(&s.ready, &ready) && s.ready.load(Ordering::SeqCst) {
                    s.tx.send(Message::text(text.clone())).ok();
                }
            }
        }
    }
    drop(tx);
    state.sockets.lock().unwrap().retain(|s| !Arc::ptr_eq(&s.ready, &ready));
    writer.abort();
}
