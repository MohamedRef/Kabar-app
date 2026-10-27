//! Kabar Cloud's HTTP API (`docs/CLOUD.md` §2). Blocking; the engine calls it
//! from tokio's blocking pool.

use std::time::Duration;

use kabar_protocol::DeviceId;
use kabar_protocol::ai::{AiReject, EngineCall, EngineError, EngineReply};
use kabar_protocol::cloud::{
    ApiError, DeviceList, ErrorCode, GoogleSignIn, RefreshRequest, Session, TtsRequest, UsageReport,
};
use serde::de::DeserializeOwned;
use ureq::http::Response;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CloudError {
    #[error("network: {0}")]
    Network(String),
    #[error("{status} {code:?}: {message}")]
    Api { status: u16, code: ErrorCode, message: String },
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
    agent: ureq::Agent,
    base: String,
}

impl std::fmt::Debug for Api {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api").field("base", &self.base).finish_non_exhaustive()
    }
}

impl Api {
    pub fn new(base_url: &str) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(20)))
            .build();
        Self { agent: config.into(), base: base_url.trim_end_matches('/').to_string() }
    }

    /// `wss://…/v1/sync` (or `ws://` for a local server).
    pub fn sync_url(&self) -> String {
        let rest = self.base.strip_prefix("https://").map(|r| format!("wss://{r}"));
        rest.unwrap_or_else(|| self.base.replacen("http://", "ws://", 1)) + "/v1/sync"
    }

    pub fn sign_in_google(&self, body: &GoogleSignIn) -> Result<Session, CloudError> {
        let res = self.agent.post(format!("{}/v1/auth/google", self.base)).send_json(body);
        read(res)
    }

    pub fn refresh(&self, refresh_token: &str) -> Result<Session, CloudError> {
        let body = RefreshRequest { refresh_token: refresh_token.into() };
        read(self.agent.post(format!("{}/v1/auth/refresh", self.base)).send_json(&body))
    }

    pub fn sign_out(&self, access: &str) -> Result<(), CloudError> {
        let res =
            self.agent.post(format!("{}/v1/auth/signout", self.base)).header("authorization", bearer(access));
        read_empty(res.send_empty())
    }

    pub fn devices(&self, access: &str) -> Result<DeviceList, CloudError> {
        read(
            self.agent
                .get(format!("{}/v1/devices", self.base))
                .header("authorization", bearer(access))
                .call(),
        )
    }

    /// `GET /v1/usage`: the cost dashboard's counters.
    pub fn usage(&self, access: &str) -> Result<UsageReport, CloudError> {
        read(self.agent.get(format!("{}/v1/usage", self.base)).header("authorization", bearer(access)).call())
    }

    pub fn revoke(&self, access: &str, device: &DeviceId) -> Result<(), CloudError> {
        let url = format!("{}/v1/devices/{}", self.base, device.0);
        read_empty(self.agent.delete(url).header("authorization", bearer(access)).call())
    }
}

impl Api {
    /// `POST /v1/ai`: the proxy runs one engine on an already-rendered prompt.
    /// The outer error is about reaching Kabar Cloud; the inner one is the
    /// engine's (quota, rate limit, switched off), which the router handles.
    pub fn ai(
        &self,
        access: &str,
        call: &EngineCall,
    ) -> Result<Result<EngineReply, EngineError>, CloudError> {
        let res = self
            .agent
            .post(format!("{}/v1/ai", self.base))
            .header("authorization", bearer(access))
            .config()
            .timeout_global(Some(Duration::from_secs(90)))
            .build()
            .send_json(call);
        let mut res = res.map_err(|e| CloudError::Network(e.to_string()))?;
        let status = res.status().as_u16();
        let text = res.body_mut().read_to_string().map_err(|e| CloudError::Decode(e.to_string()))?;
        if (200..300).contains(&status) {
            return serde_json::from_str(&text).map(Ok).map_err(|e| CloudError::Decode(e.to_string()));
        }
        match serde_json::from_str::<EngineError>(&text) {
            Ok(e) => Ok(Err(e)),
            Err(_) => Err(api_error(status, &text)),
        }
    }

    /// `POST /v1/ai/reject` → `204`: drops a 👎'd answer from the shared cache.
    pub fn ai_reject(&self, access: &str, reject: &AiReject) -> Result<(), CloudError> {
        let res = self
            .agent
            .post(format!("{}/v1/ai/reject", self.base))
            .header("authorization", bearer(access))
            .send_json(reject);
        read_empty(res)
    }

    /// `POST /v1/tts` → MP3 bytes.
    pub fn tts(&self, access: &str, req: &TtsRequest) -> Result<Vec<u8>, CloudError> {
        let res = self
            .agent
            .post(format!("{}/v1/tts", self.base))
            .header("authorization", bearer(access))
            .send_json(req);
        let mut res = check(res)?;
        res.body_mut()
            .with_config()
            .limit(MAX_AUDIO_BYTES)
            .read_to_vec()
            .map_err(|e| CloudError::Decode(e.to_string()))
    }
}

impl Api {
    /// `PUT /v1/blobs/{sha256}`: an image for the vision tasks, kept a week.
    pub fn put_blob(&self, access: &str, sha256: &str, bytes: &[u8]) -> Result<(), CloudError> {
        let res = self
            .agent
            .put(format!("{}/v1/blobs/{sha256}", self.base))
            .header("authorization", bearer(access))
            .header("content-type", "application/octet-stream")
            .send(bytes);
        read_empty(res)
    }
}

/// Far above the proxy's 5,000-character requests (about 1 MB of MP3).
const MAX_AUDIO_BYTES: u64 = 16 * 1024 * 1024;

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

type Res = Result<Response<ureq::Body>, ureq::Error>;

fn read<T: DeserializeOwned>(res: Res) -> Result<T, CloudError> {
    let mut res = check(res)?;
    res.body_mut().read_json().map_err(|e| CloudError::Decode(e.to_string()))
}

fn read_empty(res: Res) -> Result<(), CloudError> {
    check(res).map(drop)
}

fn check(res: Res) -> Result<Response<ureq::Body>, CloudError> {
    let mut res = res.map_err(|e| CloudError::Network(e.to_string()))?;
    let status = res.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(res);
    }
    let text = res.body_mut().read_to_string().unwrap_or_default();
    Err(api_error(status, &text))
}

/// Error bodies are [`ApiError`]; anything else (a proxy page, a 502 from
/// Cloudflare) still gets a code from the status.
pub fn api_error(status: u16, body: &str) -> CloudError {
    match serde_json::from_str::<ApiError>(body) {
        Ok(e) => CloudError::Api { status, code: e.code, message: e.message },
        Err(_) => {
            let code = match status {
                401 => ErrorCode::Unauthorized,
                403 => ErrorCode::NotAllowed,
                404 => ErrorCode::NotFound,
                _ => ErrorCode::Internal,
            };
            CloudError::Api { status, code, message: body.chars().take(200).collect() }
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
}
