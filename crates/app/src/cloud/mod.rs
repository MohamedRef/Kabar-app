//! Kabar Cloud from the app's side: sign-in, live sync and the Devices
//! screen. A background thread runs the [`engine::Engine`] on its own tokio
//! runtime and database connection; the UI talks to it through channels.

pub mod api;
#[cfg(test)]
#[allow(clippy::disallowed_macros)] // the e2e run prints its timings
mod e2e;
pub mod engine;
pub mod link;
pub mod oauth;
pub mod session;

use std::collections::BTreeSet;
use std::path::PathBuf;

use kabar_protocol::DeviceId;
use kabar_protocol::cloud::{DeviceInfo, DeviceSummary, UsageReport};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use self::oauth::GoogleClient;

/// Where Kabar Cloud is and which Google client signs in. Set at build time
/// from `.env` (`KABAR_CLOUD_URL`, `KABAR_GOOGLE_CLIENT_ID`,
/// `KABAR_GOOGLE_CLIENT_SECRET`); a run-time `KABAR_CLOUD_URL` overrides the
/// URL for local development.
#[derive(Clone, Debug)]
pub struct CloudConfig {
    pub base_url: String,
    pub google: Option<GoogleClient>,
}

impl CloudConfig {
    pub fn from_env() -> Option<Self> {
        let var = |name: &str, built: Option<&'static str>| {
            std::env::var(name).ok().or(built.map(String::from)).filter(|v| !v.trim().is_empty())
        };
        let base_url = var("KABAR_CLOUD_URL", option_env!("KABAR_CLOUD_URL"))?;
        let google = var("KABAR_GOOGLE_CLIENT_ID", option_env!("KABAR_GOOGLE_CLIENT_ID")).map(|client_id| {
            GoogleClient {
                client_id,
                client_secret: var("KABAR_GOOGLE_CLIENT_SECRET", option_env!("KABAR_GOOGLE_CLIENT_SECRET")),
            }
        });
        Some(Self { base_url, google })
    }
}

#[derive(Debug)]
pub enum Command {
    SignInGoogle,
    /// Sign in with an ID token obtained elsewhere (the end-to-end check, and
    /// Android's Credential Manager once its bridge lands).
    #[cfg_attr(not(test), allow(dead_code))]
    SignInWithIdToken(String),
    SignOut,
    LoadDevices,
    RevokeDevice(DeviceId),
    /// Read the cost dashboard's counters (`GET /v1/usage`).
    LoadUsage,
    /// A local write landed in the outbox.
    Kick,
    /// The app went to the background (`true`: push what's waiting, then
    /// close the socket so a phone in a pocket keeps its radio asleep) or came
    /// back (`false`: reconnect and catch up).
    Background(bool),
    /// A fresh access token for calls made off the sync thread (AI, TTS).
    /// `None` when signed out. `force` refreshes even an unexpired token
    /// (after a 401).
    Access {
        force: bool,
        reply: tokio::sync::oneshot::Sender<Option<(api::Api, String)>>,
    },
    #[cfg_attr(not(test), allow(dead_code))]
    Shutdown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Status(CloudStatus),
    Devices(Result<Vec<DeviceSummary>, Problem>),
    Usage(Result<UsageReport, Problem>),
    /// Remote ops changed these tables.
    Changed(BTreeSet<String>),
    /// The browser is open for Google sign-in.
    SigningIn(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncState {
    SignedOut,
    Connecting,
    CatchingUp,
    Live,
    /// Waiting to reconnect.
    Offline,
    /// Needs an app update or a new sign-in.
    Stopped,
}

/// Something the user should hear about, in their language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    NotConfigured,
    NotAllowed,
    SignInCancelled,
    SignInFailed,
    SignedOutRemotely,
    Network,
    Server,
    Outdated,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CloudStatus {
    /// This build knows a Kabar Cloud URL.
    pub configured: bool,
    pub email: Option<String>,
    pub device_id: DeviceId,
    pub sync: SyncState,
    /// Local changes not yet acked.
    pub pending: u64,
    pub last_sync_ms: Option<u64>,
    pub problem: Option<Problem>,
}

impl CloudStatus {
    pub fn not_configured(device_id: DeviceId) -> Self {
        Self {
            configured: false,
            email: None,
            device_id,
            sync: SyncState::SignedOut,
            pending: 0,
            last_sync_ms: None,
            problem: None,
        }
    }
}

pub struct CloudHandle {
    pub commands: UnboundedSender<Command>,
    pub events: UnboundedReceiver<Event>,
}

/// Starts the engine on its own thread. `db_path` is opened a second time
/// (SQLite WAL), so the UI's connection never waits on the network.
pub fn start(
    config: CloudConfig,
    db_path: PathBuf,
    device: DeviceInfo,
    vault: Box<dyn session::SessionVault>,
) -> Result<CloudHandle, String> {
    let store = crate::store::Store::open(&db_path).map_err(|e| e.to_string())?;
    let (cmd_tx, cmd_rx) = unbounded_channel();
    let (ev_tx, ev_rx) = unbounded_channel();
    std::thread::Builder::new()
        .name("kabar-cloud".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(r) => r,
                Err(e) => {
                    crate::diag::report!("no runtime for sync: {e}");
                    return;
                }
            };
            let api = api::Api::new(&config.base_url);
            let engine = engine::Engine::new(store, api, config.google, device, vault, ev_tx);
            runtime.block_on(engine.run(cmd_rx));
        })
        .map_err(|e| e.to_string())?;
    Ok(CloudHandle { commands: cmd_tx, events: ev_rx })
}
