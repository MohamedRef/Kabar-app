//! Kabar Cloud calls made outside the sync thread: the AI proxy and the TTS
//! proxy. The sync engine owns the session; a call asks it for a fresh
//! access token ([`Command::Access`]), then does its HTTP on a short-lived
//! thread so the UI never waits on the network.

use std::cell::RefCell;
use std::rc::Rc;

use kabar_protocol::ai::{EngineCall, EngineError, EngineReply};
use kabar_protocol::cloud::{ErrorCode, TtsRequest};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use super::Command;
use super::api::{Api, CloudError};

/// Why Kabar Cloud couldn't be asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unreachable {
    /// This build has no `KABAR_CLOUD_URL`.
    NotConfigured,
    SignedOut,
    Offline,
    /// A free quota is used up (TTS characters this month).
    QuotaExhausted,
    Server(String),
}

impl std::fmt::Display for Unreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => f.write_str("Kabar Cloud isn't set up in this build"),
            Self::SignedOut => f.write_str("not signed in to Kabar Cloud"),
            Self::Offline => f.write_str("offline"),
            Self::QuotaExhausted => f.write_str("the free quota is used up"),
            Self::Server(m) => write!(f, "Kabar Cloud: {m}"),
        }
    }
}

impl From<CloudError> for Unreachable {
    fn from(e: CloudError) -> Self {
        match e {
            CloudError::Network(_) => Self::Offline,
            CloudError::Api { code: ErrorCode::QuotaExhausted, .. } => Self::QuotaExhausted,
            CloudError::Api { code: ErrorCode::Unauthorized | ErrorCode::DeviceRevoked, .. } => {
                Self::SignedOut
            }
            other => Self::Server(other.to_string()),
        }
    }
}

/// A handle on the sync thread, filled in once it starts. Cheap to clone;
/// UI-thread only (the router and the WebView are single-threaded).
#[derive(Clone, Default)]
pub struct CloudLink(Rc<RefCell<Option<UnboundedSender<Command>>>>);

impl CloudLink {
    pub fn connect(&self, commands: UnboundedSender<Command>) {
        *self.0.borrow_mut() = Some(commands);
    }

    async fn access(&self, force: bool) -> Result<(Api, String), Unreachable> {
        let commands = self.0.borrow().clone().ok_or(Unreachable::NotConfigured)?;
        let (reply, answer) = oneshot::channel();
        commands.send(Command::Access { force, reply }).map_err(|_| Unreachable::NotConfigured)?;
        answer.await.map_err(|_| Unreachable::NotConfigured)?.ok_or(Unreachable::SignedOut)
    }

    /// Runs `f` with a fresh token, once more with a forced refresh if the
    /// token was refused.
    async fn with_token<T: Send + 'static>(
        &self,
        f: impl Fn(&Api, &str) -> Result<T, CloudError> + Send + Sync + Clone + 'static,
    ) -> Result<T, Unreachable> {
        for force in [false, true] {
            let (api, token) = self.access(force).await?;
            let g = f.clone();
            match off_thread(move || g(&api, &token)).await {
                Err(CloudError::Api { code: ErrorCode::Unauthorized, .. }) if !force => continue,
                result => return result.map_err(Unreachable::from),
            }
        }
        unreachable!("the loop returns on its second pass")
    }

    /// One engine run through `POST /v1/ai`.
    pub async fn ai(&self, call: EngineCall) -> Result<EngineReply, EngineError> {
        let result = self.with_token(move |api, token| api.ai(token, &call)).await;
        match result {
            Ok(reply) => reply,
            Err(why) => Err(EngineError::Unavailable { reason: why.to_string() }),
        }
    }

    /// MP3 for `req` through `POST /v1/tts`.
    pub async fn tts(&self, req: TtsRequest) -> Result<Vec<u8>, Unreachable> {
        self.with_token(move |api, token| api.tts(token, &req)).await
    }
}

impl CloudLink {
    /// Uploads a PNG or JPEG for the vision tasks and returns the
    /// `EngineCall::image_sha256` that names it. Upload right before each
    /// vision call: it is cheap to repeat, and the server keeps images only
    /// a week.
    pub async fn put_blob(&self, bytes: Vec<u8>) -> Result<String, Unreachable> {
        let sha = sha256_hex(&bytes);
        let name = sha.clone();
        let bytes = std::sync::Arc::new(bytes);
        self.with_token(move |api, token| api.put_blob(token, &name, &bytes)).await?;
        Ok(sha)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Runs blocking work on its own thread and awaits it from any executor.
pub async fn off_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        tx.send(f()).ok();
    });
    rx.await.expect("worker thread panicked")
}
