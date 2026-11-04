//! Kabar Cloud's HTTP API (`docs/CLOUD.md` §2). Every call is async over one
//! transport: blocking `ureq` on tokio's blocking pool on the Mac and
//! Android, the browser's `fetch` in the web app (docs/WEB.md).

use std::time::Duration;

use kabar_protocol::DeviceId;
use kabar_protocol::ai::{AiReject, EngineCall, EngineError, EngineReply};
use kabar_protocol::cloud::{
    ApiError, DeviceList, ErrorCode, GoogleSignIn, PasswordLogin, PasswordSignIn, PushTokenReply,
    PushTokenRequest, RefreshRequest, Session, SetPassword, TtsRequest, UsageReport,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CloudError {
    #[error("network: {0}")]
    Network(String),
    /// `retry_after_ms` comes with [`ErrorCode::TooManyTries`].
    #[error("{status} {code:?}: {message}")]
    Api { status: u16, code: ErrorCode, message: String, retry_after_ms: Option<u64> },
    #[error("unexpected response: {0}")]
    Decode(String),
}

impl CloudError {
    /// The session is gone for good: sign in again.
    pub fn is_signed_out(&self) -> bool {
        matches!(self, Self::Api { code: ErrorCode::Unauthorized | ErrorCode::DeviceRevoked, .. })
    }
}

#[derive(Clone)]
pub struct Api {
    #[cfg(not(target_arch = "wasm32"))]
    agent: ureq::Agent,
    base: String,
}

impl std::fmt::Debug for Api {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api").field("base", &self.base).finish_non_exhaustive()
    }
}

/// One request: method, path under the base URL, bearer token and body.
pub struct Call {
    method: &'static str,
    path: String,
    access: Option<String>,
    body: Option<(Vec<u8>, &'static str)>,
    timeout: Duration,
    /// Longest body read; more is an error.
    limit: u64,
}

/// The status and body of an answer (any status).
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Default limits: the server's answers are small JSON.
const TIMEOUT: Duration = Duration::from_secs(20);
const MAX_JSON_BYTES: u64 = 4 * 1024 * 1024;
/// Far above the proxy's 5,000-character requests (about 1 MB of MP3).
const MAX_AUDIO_BYTES: u64 = 16 * 1024 * 1024;

impl Call {
    fn new(method: &'static str, path: impl Into<String>) -> Self {
        Self { method, path: path.into(), access: None, body: None, timeout: TIMEOUT, limit: MAX_JSON_BYTES }
    }

    fn auth(mut self, access: &str) -> Self {
        self.access = Some(access.to_owned());
        self
    }

    fn json(mut self, body: &impl Serialize) -> Self {
        self.body = Some((serde_json::to_vec(body).unwrap_or_default(), "application/json"));
        self
    }
}

impl Api {
    pub fn new(base_url: &str) -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            agent: ureq::Agent::config_builder().http_status_as_error(false).build().into(),
            base: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// `wss://…/v1/sync` (or `ws://` for a local server).
    pub fn sync_url(&self) -> String {
        let rest = self.base.strip_prefix("https://").map(|r| format!("wss://{r}"));
        rest.unwrap_or_else(|| self.base.replacen("http://", "ws://", 1)) + "/v1/sync"
    }

    pub async fn sign_in_google(&self, body: &GoogleSignIn) -> Result<Session, CloudError> {
        read(self.send(Call::new("POST", "/v1/auth/google").json(body)).await?)
    }

    /// `POST /v1/auth/password`: a device without Google joins the account.
    pub async fn sign_in_password(&self, body: &PasswordSignIn) -> Result<Session, CloudError> {
        read(self.send(Call::new("POST", "/v1/auth/password").json(body)).await?)
    }

    pub async fn login(&self, access: &str) -> Result<PasswordLogin, CloudError> {
        read(self.send(Call::new("GET", "/v1/login").auth(access)).await?)
    }

    pub async fn set_login(&self, access: &str, body: &SetPassword) -> Result<PasswordLogin, CloudError> {
        read(self.send(Call::new("POST", "/v1/login").auth(access).json(body)).await?)
    }

    pub async fn remove_login(&self, access: &str) -> Result<PasswordLogin, CloudError> {
        read(self.send(Call::new("DELETE", "/v1/login").auth(access)).await?)
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<Session, CloudError> {
        let body = RefreshRequest { refresh_token: refresh_token.into() };
        read(self.send(Call::new("POST", "/v1/auth/refresh").json(&body)).await?)
    }

    pub async fn sign_out(&self, access: &str) -> Result<(), CloudError> {
        read_empty(self.send(Call::new("POST", "/v1/auth/signout").auth(access)).await?)
    }

    pub async fn devices(&self, access: &str) -> Result<DeviceList, CloudError> {
        read(self.send(Call::new("GET", "/v1/devices").auth(access)).await?)
    }

    /// `GET /v1/usage`: the cost dashboard's counters.
    pub async fn usage(&self, access: &str) -> Result<UsageReport, CloudError> {
        read(self.send(Call::new("GET", "/v1/usage").auth(access)).await?)
    }

    /// `PUT /v1/devices/me/push`: this device's FCM token (`""` turns push
    /// off for it).
    pub async fn set_push_token(&self, access: &str, token: &str) -> Result<PushTokenReply, CloudError> {
        let body = PushTokenRequest { token: token.into() };
        read(self.send(Call::new("PUT", "/v1/devices/me/push").auth(access).json(&body)).await?)
    }

    pub async fn revoke(&self, access: &str, device: &DeviceId) -> Result<(), CloudError> {
        read_empty(self.send(Call::new("DELETE", format!("/v1/devices/{}", device.0)).auth(access)).await?)
    }
}

impl Api {
    /// `POST /v1/ai`: the proxy runs one engine on an already-rendered prompt.
    /// The outer error is about reaching Kabar Cloud; the inner one is the
    /// engine's (quota, rate limit, switched off), which the router handles.
    pub async fn ai(
        &self,
        access: &str,
        call: &EngineCall,
    ) -> Result<Result<EngineReply, EngineError>, CloudError> {
        let mut req = Call::new("POST", "/v1/ai").auth(access).json(call);
        req.timeout = Duration::from_secs(90);
        let reply = self.send(req).await?;
        let text = String::from_utf8_lossy(&reply.body);
        if (200..300).contains(&reply.status) {
            return serde_json::from_str(&text).map(Ok).map_err(|e| CloudError::Decode(e.to_string()));
        }
        match serde_json::from_str::<EngineError>(&text) {
            Ok(e) => Ok(Err(e)),
            Err(_) => Err(api_error(reply.status, &text)),
        }
    }

    /// `POST /v1/ai/reject` → `204`: drops a 👎'd answer from the shared cache.
    pub async fn ai_reject(&self, access: &str, reject: &AiReject) -> Result<(), CloudError> {
        read_empty(self.send(Call::new("POST", "/v1/ai/reject").auth(access).json(reject)).await?)
    }

    /// `POST /v1/tts` → MP3 bytes.
    pub async fn tts(&self, access: &str, req: &TtsRequest) -> Result<Vec<u8>, CloudError> {
        let mut call = Call::new("POST", "/v1/tts").auth(access).json(req);
        call.limit = MAX_AUDIO_BYTES;
        check(self.send(call).await?)
    }

    /// `PUT /v1/blobs/{sha256}`: an image for the vision tasks, kept a week.
    pub async fn put_blob(&self, access: &str, sha256: &str, bytes: &[u8]) -> Result<(), CloudError> {
        let mut call = Call::new("PUT", format!("/v1/blobs/{sha256}")).auth(access);
        call.body = Some((bytes.to_vec(), "application/octet-stream"));
        read_empty(self.send(call).await?)
    }
}

impl Api {
    /// Sends `call`; `Err` only when Kabar Cloud couldn't be reached or the
    /// answer couldn't be read. Any HTTP status is a [`Reply`].
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn send(&self, call: Call) -> Result<Reply, CloudError> {
        use std::io::Read as _;
        let agent = self.agent.clone();
        let url = format!("{}{}", self.base, call.path);
        crate::platform::time::blocking(move || {
            let mut req = ureq::http::Request::builder().method(call.method).uri(&url);
            if let Some(access) = &call.access {
                req = req.header("authorization", bearer(access));
            }
            let net = |e: &dyn std::fmt::Display| CloudError::Network(e.to_string());
            // GET and DELETE go without a body (not even an empty one).
            let mut res = match call.body {
                Some((body, kind)) => {
                    let req = req.header("content-type", kind).body(body).map_err(|e| net(&e))?;
                    agent.run(agent.configure_request(req).timeout_global(Some(call.timeout)).build())
                }
                None => {
                    let req = req.body(()).map_err(|e| net(&e))?;
                    agent.run(agent.configure_request(req).timeout_global(Some(call.timeout)).build())
                }
            }
            .map_err(|e| net(&e))?;
            let status = res.status().as_u16();
            let mut bytes = Vec::new();
            res.body_mut()
                .as_reader()
                .take(call.limit + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| CloudError::Network(e.to_string()))?;
            if bytes.len() as u64 > call.limit {
                return Err(CloudError::Decode("answer too large".into()));
            }
            Ok(Reply { status, body: bytes })
        })
        .await
    }
}

/// The browser's `fetch`, with a time limit; answers `[status, bytes]`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export async function cloud_fetch(method, url, access, body, kind, timeout_ms) {
  const headers = {};
  if (access) headers["authorization"] = "Bearer " + access;
  if (kind) headers["content-type"] = kind;
  const init = { method, headers, mode: "cors", credentials: "omit", cache: "no-store" };
  if (kind) init.body = body.slice();
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeout_ms);
  init.signal = ctl.signal;
  try {
    const res = await fetch(url, init);
    const bytes = new Uint8Array(await res.arrayBuffer());
    return [res.status, bytes];
  } finally {
    clearTimeout(timer);
  }
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn cloud_fetch(
        method: &str,
        url: &str,
        access: Option<String>,
        body: &[u8],
        kind: &str,
        timeout_ms: f64,
    ) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;
}

#[cfg(target_arch = "wasm32")]
impl Api {
    /// The web app: the same request through `fetch` (Kabar Cloud answers
    /// the web app's origin with CORS, crates/server/src/web.rs).
    pub async fn send(&self, call: Call) -> Result<Reply, CloudError> {
        use wasm_bindgen::JsCast as _;
        let url = format!("{}{}", self.base, call.path);
        let (body, kind) = call.body.unwrap_or_default();
        let answer =
            cloud_fetch(call.method, &url, call.access, &body, kind, call.timeout.as_millis() as f64)
                .await
                .map_err(|e| CloudError::Network(format!("{e:?}")))?;
        let pair: js_sys::Array = answer.dyn_into().map_err(|_| CloudError::Decode("fetch answer".into()))?;
        let status = pair.get(0).as_f64().unwrap_or(0.0) as u16;
        let body = js_sys::Uint8Array::new(&pair.get(1)).to_vec();
        if body.len() as u64 > call.limit {
            return Err(CloudError::Decode("answer too large".into()));
        }
        Ok(Reply { status, body })
    }
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn read<T: DeserializeOwned>(reply: Reply) -> Result<T, CloudError> {
    serde_json::from_slice(&check(reply)?).map_err(|e| CloudError::Decode(e.to_string()))
}

fn read_empty(reply: Reply) -> Result<(), CloudError> {
    check(reply).map(drop)
}

/// The body of a 2xx answer, else the server's error.
fn check(reply: Reply) -> Result<Vec<u8>, CloudError> {
    if (200..300).contains(&reply.status) {
        return Ok(reply.body);
    }
    Err(api_error(reply.status, &String::from_utf8_lossy(&reply.body)))
}

/// Error bodies are [`ApiError`]; anything else (a proxy page, a 502 from
/// Cloudflare) still gets a code from the status.
pub fn api_error(status: u16, body: &str) -> CloudError {
    match serde_json::from_str::<ApiError>(body) {
        Ok(e) => {
            CloudError::Api { status, code: e.code, message: e.message, retry_after_ms: e.retry_after_ms }
        }
        Err(_) => {
            let code = match status {
                401 => ErrorCode::Unauthorized,
                403 => ErrorCode::NotAllowed,
                404 => ErrorCode::NotFound,
                _ => ErrorCode::Internal,
            };
            CloudError::Api { status, code, message: body.chars().take(200).collect(), retry_after_ms: None }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_url_follows_the_scheme() {
        assert_eq!(
            Api::new("https://kabar-cloud.x.workers.dev/").sync_url(),
            "wss://kabar-cloud.x.workers.dev/v1/sync"
        );
        assert_eq!(Api::new("http://127.0.0.1:8787").sync_url(), "ws://127.0.0.1:8787/v1/sync");
    }

    #[test]
    fn errors_keep_the_server_code() {
        let e = api_error(401, r#"{"code":"device_revoked","message":"signed out"}"#);
        assert!(e.is_signed_out());
        assert!(matches!(e, CloudError::Api { code: ErrorCode::DeviceRevoked, .. }));
        let e = api_error(403, "<html>blocked</html>");
        assert!(matches!(e, CloudError::Api { code: ErrorCode::NotAllowed, .. }));
        assert!(!e.is_signed_out());
    }

    #[test]
    fn a_lockout_keeps_its_wait_and_a_wrong_password_is_not_a_sign_out() {
        let e = api_error(429, r#"{"code":"too_many_tries","message":"wait","retry_after_ms":120000}"#);
        assert!(matches!(
            e,
            CloudError::Api { code: ErrorCode::TooManyTries, retry_after_ms: Some(120_000), .. }
        ));
        let e = api_error(401, r#"{"code":"wrong_password","message":"no"}"#);
        assert!(!e.is_signed_out(), "a typo must not sign the device out");
    }
}
