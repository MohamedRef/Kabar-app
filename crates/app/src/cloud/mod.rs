//! Kabar Cloud from the app's side: sign-in, live sync and the Devices
//! screen. The [`engine::Engine`] runs on its own thread, tokio runtime and
//! database connection on the Mac and Android, and as a task on the page's
//! one thread (sharing the UI's database) on the web; the UI talks to it
//! through channels either way.

pub mod api;
#[cfg(test)]
mod converge;
#[cfg(test)]
#[allow(clippy::disallowed_macros)] // the e2e run prints its timings
mod e2e;
pub mod engine;
#[cfg(test)]
mod engine_tests;
#[cfg(test)]
mod fake;
pub mod link;
pub mod password;
// Google's desktop sign-in needs a loopback server; the web signs in with a
// username and password.
#[cfg(not(target_arch = "wasm32"))]
pub mod oauth;
pub mod session;
mod socket;

use std::collections::BTreeSet;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use kabar_protocol::DeviceId;
use kabar_protocol::cloud::{DeviceInfo, DeviceSummary, UsageReport};
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[cfg(not(target_arch = "wasm32"))]
use self::oauth::GoogleClient;

/// The web build has no Google sign-in (docs/WEB.md); the type keeps the
/// config one shape everywhere.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub struct GoogleClient {
    pub client_id: String,
    pub client_secret: Option<String>,
}

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
    /// The web build's server is baked in at build time (`KABAR_CLOUD_URL`).
    #[cfg(target_arch = "wasm32")]
    pub fn from_env() -> Option<Self> {
        let base_url = option_env!("KABAR_CLOUD_URL").filter(|v| !v.trim().is_empty())?;
        Some(Self { base_url: base_url.trim().to_string(), google: None })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env() -> Option<Self> {
        Self::from_vars(|name| {
            let built = match name {
                "KABAR_CLOUD_URL" => option_env!("KABAR_CLOUD_URL"),
                "KABAR_GOOGLE_CLIENT_ID" => option_env!("KABAR_GOOGLE_CLIENT_ID"),
                "KABAR_GOOGLE_CLIENT_SECRET" => option_env!("KABAR_GOOGLE_CLIENT_SECRET"),
                _ => None,
            };
            std::env::var(name).ok().or(built.map(String::from))
        })
    }

    /// The config from `var` (run time first, then build time); blank
    /// values count as unset.
    #[cfg(not(target_arch = "wasm32"))]
    fn from_vars(var: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let var = |name: &str| var(name).filter(|v| !v.trim().is_empty());
        let base_url = var("KABAR_CLOUD_URL")?;
        let google = var("KABAR_GOOGLE_CLIENT_ID")
            .map(|client_id| GoogleClient { client_id, client_secret: var("KABAR_GOOGLE_CLIENT_SECRET") });
        Some(Self { base_url, google })
    }
}

#[derive(Debug)]
pub enum Command {
    SignInGoogle,
    /// Username + password, on a device without Google (and the web app).
    /// `remember: false` (someone else's device) keeps the session only until
    /// the app or the browser tab closes.
    SignInPassword {
        username: String,
        password: Secret,
        remember: bool,
    },
    /// Read the account's username (`Event::Login`).
    LoadLogin,
    /// Set or change the username and password, from a signed-in device.
    SetLogin {
        username: String,
        password: Secret,
    },
    RemoveLogin,
    /// Sign in with an ID token obtained elsewhere (the end-to-end check, and
    /// Android's Credential Manager once its bridge lands).
    #[cfg_attr(not(test), allow(dead_code))]
    SignInWithIdToken(String),
    SignOut,
    LoadDevices,
    RevokeDevice(DeviceId),
    /// Read the cost dashboard's counters (`GET /v1/usage`).
    LoadUsage,
    /// Send this device's FCM token (`""`: no pushes here), answered with
    /// [`Event::Push`].
    PushToken(String),
    /// A local write landed in the outbox.
    Kick,
    /// Offer the held changes to Kabar Cloud again.
    SendHeldAgain,
    /// Let the held changes go: they stay here but won't reach the others.
    ForgetHeld,
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

/// A password on its way to the engine: never printed.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(…)")
    }
}

/// The account's username and password, as Settings › Account shows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoginStatus {
    /// `None` until Kabar Cloud has answered.
    pub username: Option<Option<String>>,
    /// A change is on its way.
    pub saving: bool,
    /// The last change went through.
    pub saved: bool,
    pub problem: Option<Problem>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Status(CloudStatus),
    Devices(Result<Vec<DeviceSummary>, Problem>),
    Usage(Result<UsageReport, Problem>),
    /// Remote ops changed these tables.
    Changed(BTreeSet<String>),
    /// The browser is open for Google sign-in, or a password is being checked.
    SigningIn(bool),
    Login(LoginStatus),
    /// Kabar Cloud's answer to [`Command::PushToken`]: whether it now pushes
    /// this device's reminders (`None`: it couldn't be asked).
    Push {
        token: String,
        pushed: Option<bool>,
    },
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
    /// Google said yes; the rest of the sign-in waits for the network
    /// (Android gives a backgrounded app none) and finishes by itself.
    SignInWaiting,
    SignedOutRemotely,
    Network,
    Server,
    Outdated,
    /// Username and password don't match.
    WrongPassword,
    /// Too many wrong passwords: the next try counts in this many minutes.
    Locked(u64),
    UsernameTaken,
    /// The username isn't 3–32 of a-z 0-9 . _ - (checked before sending).
    BadUsername,
    /// The new password is shorter than `MIN_PASSWORD_CHARS`.
    ShortPassword,
    /// The two new passwords differ (Settings only).
    Mismatch,
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
    /// Changes Kabar Cloud refused for good, or this build couldn't read,
    /// kept on the device (`store/held.rs`).
    pub held: u64,
    pub last_sync_ms: Option<u64>,
    pub problem: Option<Problem>,
    /// Signed in on someone else's device: the session ends with the app or tab.
    pub temporary: bool,
}

impl CloudStatus {
    pub fn not_configured(device_id: DeviceId) -> Self {
        Self {
            configured: false,
            email: None,
            device_id,
            sync: SyncState::SignedOut,
            pending: 0,
            held: 0,
            last_sync_ms: None,
            problem: None,
            temporary: false,
        }
    }
}

pub struct CloudHandle {
    pub commands: UnboundedSender<Command>,
    pub events: UnboundedReceiver<Event>,
}

#[cfg(target_arch = "wasm32")]
/// Starts the engine as a task on the page's thread, sharing `db`.
pub fn start(
    config: CloudConfig,
    db: crate::state::Db,
    device: DeviceInfo,
    vault: Box<dyn session::SessionVault>,
) -> Result<CloudHandle, String> {
    use tokio::sync::mpsc::unbounded_channel;
    let (cmd_tx, cmd_rx) = unbounded_channel();
    let (ev_tx, ev_rx) = unbounded_channel();
    let api = api::Api::new(&config.base_url);
    let engine = engine::Engine::new(engine::EngineStore(db), api, config.google, device, vault, ev_tx);
    wasm_bindgen_futures::spawn_local(engine.run(cmd_rx));
    Ok(CloudHandle { commands: cmd_tx, events: ev_rx })
}

#[cfg(not(target_arch = "wasm32"))]
/// Starts the engine on its own thread. `db_path` is opened a second time
/// (SQLite WAL), so the UI's connection never waits on the network.
pub fn start(
    config: CloudConfig,
    db_path: PathBuf,
    device: DeviceInfo,
    vault: Box<dyn session::SessionVault>,
) -> Result<CloudHandle, String> {
    let store = crate::store::Store::open(&db_path).map_err(|e| e.to_string())?;
    // Off the UI thread, sync can wait longer for the UI's lock.
    store.set_busy_timeout(crate::store::SYNC_BUSY).map_err(|e| e.to_string())?;
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
            let engine =
                engine::Engine::new(engine::EngineStore(store), api, config.google, device, vault, ev_tx);
            runtime.block_on(engine.run(cmd_rx));
        })
        .map_err(|e| e.to_string())?;
    Ok(CloudHandle { commands: cmd_tx, events: ev_rx })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn vars(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| (*v).to_string())
    }

    #[test]
    fn the_cloud_is_configured_by_its_url_and_google_is_optional() {
        assert!(CloudConfig::from_vars(vars(&[])).is_none(), "no URL: sync is off");
        assert!(
            CloudConfig::from_vars(vars(&[("KABAR_CLOUD_URL", "  ")])).is_none(),
            "blank counts as unset"
        );

        let c =
            CloudConfig::from_vars(vars(&[("KABAR_CLOUD_URL", "https://kabar-cloud.example.workers.dev")]))
                .unwrap();
        assert_eq!(c.base_url, "https://kabar-cloud.example.workers.dev");
        assert!(c.google.is_none());

        let c = CloudConfig::from_vars(vars(&[
            ("KABAR_CLOUD_URL", "http://127.0.0.1:8787"),
            ("KABAR_GOOGLE_CLIENT_ID", "123-desk.apps.googleusercontent.com"),
            ("KABAR_GOOGLE_CLIENT_SECRET", ""),
        ]))
        .unwrap();
        let g = c.google.unwrap();
        assert_eq!(g.client_id, "123-desk.apps.googleusercontent.com");
        assert_eq!(g.client_secret, None, "a blank secret is no secret");
    }

    #[test]
    fn a_status_without_a_cloud_says_so() {
        let s = CloudStatus::not_configured(DeviceId("s24u".into()));
        assert!(!s.configured);
        assert_eq!((s.sync, s.pending, s.problem), (SyncState::SignedOut, 0, None));
    }

    #[test]
    fn the_engine_needs_a_database_it_can_open() {
        let dir = tempfile::tempdir().unwrap();
        let info = DeviceInfo {
            device_id: DeviceId("mac".into()),
            name: "Mac".into(),
            model: None,
            platform: kabar_protocol::cloud::Platform::Macos,
        };
        let config = CloudConfig { base_url: "http://127.0.0.1:1".into(), google: None };
        // A folder is not a database.
        let err = start(config, dir.path().to_path_buf(), info, Box::new(session::MemoryVault::default()));
        assert!(err.is_err());
    }
}
