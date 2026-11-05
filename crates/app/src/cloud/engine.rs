//! The sync engine: one task that owns the session and the WebSocket. It
//! drives [`SyncClient`] and reports to the UI. On the Mac and Android it runs
//! on its own thread with its own database connection; on the web it runs on
//! the page's one thread and shares the UI's in-memory database
//! ([`EngineStore`]).

use std::time::Duration;

use kabar_core::sync_client::{Backoff, Effect, Phase, SyncClient};
use kabar_protocol::cloud::{
    DeviceInfo, DeviceSummary, ErrorCode, GoogleSignIn, PasswordLogin, PasswordSignIn, Session, SetPassword,
};
use kabar_protocol::sync::{MAX_OPS_PER_PUSH, ServerMsg};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use web_time::Instant;

use super::api::{Api, CloudError};
use super::password::{self, Invalid};
use super::session::{Auth, SessionVault};
use super::socket::{ConnectError, Frame, Socket};
use super::{CloudStatus, Command, Event, GoogleClient, LoginStatus, Problem, SyncState};
use crate::store::Store;

/// The engine's database: its own connection where there are threads, the
/// UI's shared one on the web (an in-memory database can't be opened twice).
/// Borrowed only between awaits, so the UI never finds it taken.
#[cfg(not(target_arch = "wasm32"))]
pub struct EngineStore(pub Store);
#[cfg(target_arch = "wasm32")]
pub struct EngineStore(pub crate::state::Db);

impl EngineStore {
    fn with<T>(&mut self, f: impl FnOnce(&mut Store) -> T) -> T {
        #[cfg(not(target_arch = "wasm32"))]
        {
            f(&mut self.0)
        }
        #[cfg(target_arch = "wasm32")]
        {
            f(&mut self.0.0.borrow_mut())
        }
    }
}

/// Keeps NAT mappings and the Cloudflare edge from dropping an idle socket.
/// Protocol pings are answered by the runtime, so they don't wake the hub.
#[cfg(not(test))]
const PING_EVERY: Duration = Duration::from_secs(30);
#[cfg(test)]
const PING_EVERY: Duration = Duration::from_millis(300);
/// A socket that hears nothing this long after a ping is half-open (a
/// laptop lid, a Wi-Fi switch, a NAT drop): it's dropped and reopened.
#[cfg(not(test))]
const ANSWER_WITHIN: Duration = Duration::from_secs(10);
#[cfg(test)]
const ANSWER_WITHIN: Duration = Duration::from_secs(2);
/// The web can't send protocol pings, and a hub ping wakes the Durable
/// Object, so an idle web socket is checked only this often (and whenever a
/// push waits for its ack).
const IDLE_CHECK_EVERY: Duration = Duration::from_secs(5 * 60);
/// A socket that hasn't opened by then counts as a failed try (a black-holed
/// network can otherwise hold the handshake for minutes).
#[cfg(not(test))]
const CONNECT_WITHIN: Duration = Duration::from_secs(15);
#[cfg(test)]
const CONNECT_WITHIN: Duration = Duration::from_secs(1);
/// Writes from screens that don't send [`Command::Kick`] still go up this soon.
const OUTBOX_EVERY: Duration = Duration::from_secs(2);

/// How long a sign-in Google already approved keeps retrying: Google's codes
/// live about ten minutes, its ID tokens an hour.
const CODE_LIFE: Duration = Duration::from_secs(9 * 60);
const ID_TOKEN_LIFE: Duration = Duration::from_secs(50 * 60);

/// A sign-in Google approved that still has to reach Google's token endpoint
/// or Kabar Cloud. Android keeps a backgrounded app off the network, and the
/// redirect lands while Chrome is still in front, so the network half waits
/// and retries until Kabar is back in front ([`Command::Background`]) or the
/// grant runs out. (Adapted from batna-app main: crates/app/src/cloud/engine.rs.)
struct PendingSignIn {
    step: SignInStep,
    until: Instant,
    retry_at: Instant,
    attempts: u32,
}

#[cfg(not(target_arch = "wasm32"))]
type Authorization = super::oauth::Authorization;
/// The web has no Google sign-in (docs/WEB.md).
#[cfg(target_arch = "wasm32")]
type Authorization = ();

enum SignInStep {
    /// A code for Google's token endpoint (the Mac and Android).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Code(Authorization),
    IdToken(String),
}

impl PendingSignIn {
    fn new(step: SignInStep, life: Duration) -> Self {
        let now = Instant::now();
        Self { step, until: now + life, retry_at: now, attempts: 0 }
    }

    /// 1, 2, 4, 8, 15, 15… seconds: quick enough to finish right after the
    /// app comes back even if no foreground signal arrives, cheap while it can't.
    fn schedule_retry(&mut self) {
        const DELAYS: [u64; 5] = [1, 2, 4, 8, 15];
        let secs = DELAYS[(self.attempts as usize).min(DELAYS.len() - 1)];
        self.attempts += 1;
        self.retry_at = Instant::now() + Duration::from_secs(secs);
    }
}

/// A socket being opened. It runs beside the commands, so AI, voices and
/// sign-out never wait behind a slow handshake.
type Opening = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Socket, ConnectError>>>>;

pub struct Engine {
    store: EngineStore,
    api: Api,
    google: Option<GoogleClient>,
    device: DeviceInfo,
    auth: Option<Auth>,
    /// Kept while signed out, for the next sign-in.
    vault: Option<Box<dyn SessionVault>>,
    client: SyncClient,
    ws: Option<Socket>,
    opening: Option<Opening>,
    /// When the socket last heard anything.
    last_heard: Instant,
    /// When a ping went out that nothing has answered yet.
    asked_at: Option<Instant>,
    backoff: Backoff,
    retry_at: Instant,
    /// Set after `Revoked`, `Outdated` or sign-out: no reconnects until sign-in.
    stopped: bool,
    /// The app is in the background: no socket until it's back.
    background: bool,
    /// The last status sent, so an unchanged one isn't sent again.
    last_status: Option<CloudStatus>,
    force_refresh: bool,
    problem: Option<Problem>,
    pending_sign_in: Option<PendingSignIn>,
    /// The usual vault, set aside while a sign-in on someone else's device
    /// keeps its session in a forgetful one; back after sign-out.
    spare_vault: Option<Box<dyn SessionVault>>,
    login: LoginStatus,
    events: UnboundedSender<Event>,
}

impl Engine {
    pub fn new(
        mut store: EngineStore,
        api: Api,
        google: Option<GoogleClient>,
        device: DeviceInfo,
        vault: Box<dyn SessionVault>,
        events: UnboundedSender<Event>,
    ) -> Self {
        let client = SyncClient::new(store.with(|s| s.last_seq()).unwrap_or(0));
        let (auth, vault) = match Auth::restore(vault, now_ms()) {
            Ok(auth) => (Some(auth), None),
            Err(vault) => (None, Some(vault)),
        };
        Self {
            store,
            api,
            google,
            device,
            auth,
            vault,
            client,
            ws: None,
            opening: None,
            last_heard: Instant::now(),
            asked_at: None,
            backoff: Backoff::default(),
            retry_at: Instant::now(),
            stopped: false,
            background: false,
            last_status: None,
            force_refresh: false,
            problem: None,
            pending_sign_in: None,
            spare_vault: None,
            login: LoginStatus::default(),
            events,
        }
    }

    pub async fn run(mut self, mut commands: UnboundedReceiver<Command>) {
        // Both timers only run while a socket is open.
        let mut next_ping = Instant::now() + PING_EVERY;
        let mut next_outbox = Instant::now() + OUTBOX_EVERY;
        self.emit_status();
        loop {
            if self.pending_sign_in.as_ref().is_some_and(|p| Instant::now() >= p.retry_at) {
                self.finish_sign_in().await;
                self.emit_status();
            }
            if self.ws.is_none()
                && self.opening.is_none()
                && self.wants_socket()
                && Instant::now() >= self.retry_at
            {
                self.connect().await;
                self.emit_status();
            }
            let mut wait_until = if self.ws.is_some() {
                let answer_by = self.asked_at.map(|t| t + ANSWER_WITHIN);
                next_ping.min(next_outbox).min(answer_by.unwrap_or(next_ping))
            } else if self.opening.is_some() {
                Instant::now() + Duration::from_secs(3600)
            } else if self.wants_socket() {
                self.retry_at
            } else {
                Instant::now() + Duration::from_secs(3600)
            };
            if let Some(p) = &self.pending_sign_in {
                wait_until = wait_until.min(p.retry_at);
            }
            let wait = crate::platform::time::sleep(wait_until.saturating_duration_since(Instant::now()));
            tokio::select! {
                biased;
                cmd = commands.recv() => match cmd {
                    None | Some(Command::Shutdown) => break,
                    Some(cmd) => self.command(cmd).await,
                },
                frame = next_frame(&mut self.ws) => self.frame(frame).await,
                opened = opened(&mut self.opening) => {
                    self.opening = None;
                    self.opened(opened).await;
                    let now = Instant::now();
                    (next_ping, next_outbox) = (now + PING_EVERY, now + OUTBOX_EVERY);
                }
                () = wait => {
                    let now = Instant::now();
                    if self.asked_at.is_some_and(|t| now >= t + ANSWER_WITHIN) {
                        crate::diag::report!("sync socket: no answer to a ping, reconnecting");
                        if let Some(ws) = self.ws.take() {
                            ws.close().await;
                        }
                        self.drop_socket();
                    }
                    if self.ws.is_some() && now >= next_outbox {
                        next_outbox = now + OUTBOX_EVERY;
                        self.pump().await;
                    }
                    if now >= next_ping {
                        next_ping = now + PING_EVERY;
                        self.ping(now).await;
                    }
                }
            }
            if !self.wants_socket() {
                // Signed out or in the background: a handshake in flight is dropped.
                self.opening = None;
            }
            if self.ws.is_some() && !leads() {
                // A newer tab took over: it syncs from now on.
                if let Some(ws) = self.ws.take() {
                    ws.close().await;
                }
                self.client.on_close();
            }
            self.emit_status();
        }
        if let Some(ws) = self.ws.take() {
            ws.close().await;
        }
    }

    fn wants_socket(&self) -> bool {
        self.auth.is_some() && !self.stopped && !self.background && leads()
    }

    /// Checks the socket is still there. Native sends a protocol ping, which
    /// the Cloudflare runtime answers without waking the hub; the browser
    /// can't, so the web asks the hub itself, but only while a push waits for
    /// its ack or after a long quiet spell.
    async fn ping(&mut self, now: Instant) {
        let Some(ws) = &mut self.ws else { return };
        let sent = if cfg!(target_arch = "wasm32") {
            let waiting = !self.client.in_flight().is_empty() && now >= self.last_heard + ANSWER_WITHIN;
            if self.asked_at.is_some() || !(waiting || now >= self.last_heard + IDLE_CHECK_EVERY) {
                return;
            }
            let ping = serde_json::to_string(&kabar_protocol::sync::ClientMsg::Ping).unwrap_or_default();
            ws.send(ping).await
        } else {
            ws.ping().await
        };
        match sent {
            Ok(()) => {
                self.asked_at.get_or_insert(now);
            }
            Err(e) => {
                crate::diag::report!("sync ping: {e}");
                self.drop_socket();
            }
        }
    }

    // --- commands --------------------------------------------------------

    async fn command(&mut self, cmd: Command) {
        match cmd {
            Command::SignInGoogle => self.sign_in_google().await,
            Command::SignInWithIdToken(token) => self.sign_in(token).await,
            Command::SignInPassword { username, password, remember } => {
                self.problem = None;
                self.pending_sign_in = None;
                self.events.send(Event::SigningIn(true)).ok();
                self.sign_in_password(username, password.0, remember).await;
                self.events.send(Event::SigningIn(false)).ok();
            }
            Command::LoadLogin => {
                let result = self.with_access(|api, access| async move { api.login(&access).await }).await;
                self.login_answer(result);
            }
            Command::SetLogin { username, password } => self.set_login(username, password.0).await,
            Command::RemoveLogin => {
                self.login_busy();
                let result =
                    self.with_access(|api, access| async move { api.remove_login(&access).await }).await;
                self.login_answer(result);
            }
            Command::SignOut => {
                if let Some(access) = self.access(false).await {
                    let _ = self.api.sign_out(&access).await;
                }
                self.sign_out_locally();
            }
            Command::LoadDevices => self.load_devices().await,
            Command::LoadUsage => self.load_usage().await,
            Command::PushToken(token) => self.set_push_token(token).await,
            Command::RevokeDevice(device) => {
                if device == self.device.device_id {
                    return Box::pin(self.command(Command::SignOut)).await;
                }
                if let Some(access) = self.access(false).await
                    && let Err(e) = self.api.revoke(&access, &device).await
                {
                    self.cloud_error(&e);
                }
                self.load_devices().await;
            }
            Command::Kick => {
                self.retry_sign_in_now();
                self.pump().await
            }
            Command::Background(true) if !self.background => {
                self.pump().await;
                self.background = true;
                if let Some(ws) = self.ws.take() {
                    ws.close().await;
                    self.client.on_close();
                }
            }
            Command::Background(false) if self.background => {
                // Back in front: a sign-in Google approved meanwhile finishes now.
                self.retry_sign_in_now();
                self.background = false;
                self.backoff.reset();
                self.retry_at = Instant::now();
            }
            Command::Background(going) => {
                if !going {
                    self.retry_sign_in_now();
                }
            }
            Command::Access { force, reply } => {
                let access = self.access(force).await;
                reply.send(access.map(|token| (self.api.clone(), token))).ok();
            }
            Command::Shutdown => {}
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn sign_in_google(&mut self) {
        use super::oauth::{self, OAuthError};
        let Some(google) = self.google.clone() else {
            self.problem = Some(Problem::NotConfigured);
            return;
        };
        self.problem = None;
        self.pending_sign_in = None;
        self.events.send(Event::SigningIn(true)).ok();
        let authorization = oauth::google_authorization(&google).await;
        self.events.send(Event::SigningIn(false)).ok();
        match authorization {
            Ok(auth) => {
                self.pending_sign_in = Some(PendingSignIn::new(SignInStep::Code(auth), CODE_LIFE));
                self.finish_sign_in().await;
            }
            Err(OAuthError::Cancelled) => self.problem = Some(Problem::SignInCancelled),
            Err(e) => {
                crate::diag::report!("Google sign-in: {e}");
                self.problem = Some(Problem::SignInFailed);
            }
        }
    }

    /// A waiting sign-in tries again right away (the app came back to the front).
    fn retry_sign_in_now(&mut self) {
        if let Some(p) = &mut self.pending_sign_in {
            p.retry_at = Instant::now();
        }
    }

    /// Runs the network half of a sign-in Google approved: the code
    /// exchange, then Kabar Cloud. A network failure keeps it pending and
    /// retries ([`PendingSignIn`]); anything else ends it.
    async fn finish_sign_in(&mut self) {
        let Some(pending) = self.pending_sign_in.take() else { return };
        #[cfg(not(target_arch = "wasm32"))]
        let mut pending = pending;
        if Instant::now() >= pending.until {
            crate::diag::report!("Kabar Cloud sign-in: gave up waiting for the network");
            self.problem = Some(Problem::SignInFailed);
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let SignInStep::Code(auth) = &pending.step {
            use super::oauth::{self, OAuthError};
            let Some(google) = self.google.clone() else {
                self.problem = Some(Problem::NotConfigured);
                return;
            };
            let auth = auth.clone();
            let exchanged = tokio::task::spawn_blocking(move || oauth::exchange_code(&google, &auth))
                .await
                .unwrap_or_else(|e| Err(OAuthError::Failed(e.to_string())));
            match exchanged {
                Ok(id_token) => pending = PendingSignIn::new(SignInStep::IdToken(id_token), ID_TOKEN_LIFE),
                Err(OAuthError::Network(e)) => {
                    crate::diag::report!("Google sign-in: network: {e}");
                    self.wait_for_network(pending);
                    return;
                }
                Err(e) => {
                    crate::diag::report!("Google sign-in: {e}");
                    self.problem = Some(Problem::SignInFailed);
                    return;
                }
            }
        }
        let SignInStep::IdToken(id_token) = &pending.step else { return };
        let body = GoogleSignIn { id_token: id_token.clone(), device: self.device.clone() };
        match self.api.sign_in_google(&body).await {
            Ok(session) => self.begin_session(session, true),
            Err(CloudError::Network(e)) => {
                crate::diag::report!("Kabar Cloud sign-in: network: {e}");
                self.wait_for_network(pending);
            }
            Err(e) => {
                crate::diag::report!("Kabar Cloud sign-in: {e}");
                self.problem = Some(match &e {
                    CloudError::Api { code: ErrorCode::NotAllowed, .. } => Problem::NotAllowed,
                    _ => Problem::SignInFailed,
                });
            }
        }
    }

    fn wait_for_network(&mut self, mut pending: PendingSignIn) {
        pending.schedule_retry();
        self.pending_sign_in = Some(pending);
        self.problem = Some(Problem::SignInWaiting);
    }

    /// The web build signs in with a username and password (docs/WEB.md).
    #[cfg(target_arch = "wasm32")]
    async fn sign_in_google(&mut self) {
        let _ = &self.google;
        self.problem = Some(Problem::NotConfigured);
    }

    async fn sign_in(&mut self, id_token: String) {
        let body = GoogleSignIn { id_token, device: self.device.clone() };
        match self.api.sign_in_google(&body).await {
            Ok(session) => self.begin_session(session, true),
            Err(e) => {
                crate::diag::report!("Kabar Cloud sign-in: {e}");
                self.problem = Some(match &e {
                    CloudError::Api { code: ErrorCode::NotAllowed, .. } => Problem::NotAllowed,
                    CloudError::Network(_) => Problem::Network,
                    _ => Problem::SignInFailed,
                });
            }
        }
    }

    /// Keeps a session Kabar Cloud just gave and starts syncing.
    /// `remember: false` keeps it until the app or tab closes
    /// ([`super::session::not_remembered`]).
    fn begin_session(&mut self, session: Session, remember: bool) {
        if self.auth.is_some() {
            // A sign-in over a live session: the old one is put away first,
            // so its vault is the one this session is kept in.
            self.sign_out_locally();
        }
        let usual = self.vault.take().unwrap_or_else(|| Box::new(super::session::MemoryVault::default()));
        let vault = if remember {
            usual
        } else {
            // An older session for this device mustn't outlive this one.
            usual.clear();
            self.spare_vault = Some(usual);
            super::session::not_remembered()
        };
        match Auth::new(session, vault) {
            Ok(auth) => self.auth = Some(auth),
            Err(vault) => {
                crate::diag::report!("Kabar Cloud: sign-in session was not saved");
                self.vault = Some(self.spare_vault.take().unwrap_or(vault));
                self.problem = Some(Problem::SignInFailed);
                return;
            }
        }
        self.problem = None;
        self.stopped = false;
        self.opening = None;
        self.client = SyncClient::new(self.store.with(|s| s.last_seq()).unwrap_or(0));
        self.backoff.reset();
        self.retry_at = Instant::now();
    }

    async fn sign_in_password(&mut self, username: String, password: String, remember: bool) {
        let key = match stretch(username.clone(), password).await {
            Ok(k) => k,
            Err(e) => {
                crate::diag::report!("password sign-in: {e}");
                self.problem = Some(Problem::SignInFailed);
                return;
            }
        };
        let body = PasswordSignIn { username, password_key: key, device: self.device.clone() };
        match self.api.sign_in_password(&body).await {
            Ok(session) => self.begin_session(session, remember),
            Err(e) => {
                crate::diag::report!("password sign-in: {e}");
                self.problem = Some(password_problem(&e).unwrap_or(match e {
                    CloudError::Network(_) => Problem::Network,
                    CloudError::Api { code: ErrorCode::NotAllowed, .. } => Problem::NotAllowed,
                    _ => Problem::SignInFailed,
                }));
            }
        }
    }

    /// Runs a call with an access token; a 401 refreshes once and retries
    /// (this device's clock may run behind the server's). `None` when there
    /// is no token (the reason is recorded).
    async fn with_access<T, F>(&mut self, call: impl Fn(Api, String) -> F) -> Option<Result<T, CloudError>>
    where
        F: std::future::Future<Output = Result<T, CloudError>>,
    {
        let mut force = false;
        loop {
            let access = self.access(force).await?;
            match call(self.api.clone(), access).await {
                Err(CloudError::Api { code: ErrorCode::Unauthorized, .. }) if !force => force = true,
                result => return Some(result),
            }
        }
    }

    fn login_busy(&mut self) {
        self.login.saving = true;
        self.login.saved = false;
        self.login.problem = None;
        self.events.send(Event::Login(self.login.clone())).ok();
    }

    /// Settings shows what Kabar Cloud said about the username, or why it
    /// couldn't say.
    fn login_answer(&mut self, result: Option<Result<PasswordLogin, CloudError>>) {
        let was_saving = std::mem::take(&mut self.login.saving);
        match result {
            Some(Ok(l)) => {
                self.login.username = Some(l.username);
                self.login.saved = was_saving;
                self.login.problem = None;
            }
            Some(Err(e)) => {
                self.login.problem = Some(password_problem(&e).unwrap_or_else(|| {
                    self.cloud_error(&e);
                    self.problem.unwrap_or(Problem::Server)
                }));
            }
            None => self.login.problem = Some(self.problem.unwrap_or(Problem::Network)),
        }
        self.events.send(Event::Login(self.login.clone())).ok();
    }

    async fn set_login(&mut self, username: String, password: String) {
        if let Err(why) = password::check_new(&username, &password) {
            self.login.problem = Some(match why {
                Invalid::Username => Problem::BadUsername,
                Invalid::TooShort => Problem::ShortPassword,
            });
            self.login.saved = false;
            self.events.send(Event::Login(self.login.clone())).ok();
            return;
        }
        self.login_busy();
        let key = match stretch(username.clone(), password).await {
            Ok(k) => k,
            Err(e) => {
                crate::diag::report!("setting the password: {e}");
                self.login.saving = false;
                self.login.problem = Some(Problem::Server);
                self.events.send(Event::Login(self.login.clone())).ok();
                return;
            }
        };
        let body = SetPassword { username, password_key: key };
        let call = move |api: Api, access: String| {
            let body = body.clone();
            async move { api.set_login(&access, &body).await }
        };
        let result = self.with_access(call).await;
        self.login_answer(result);
    }

    fn sign_out_locally(&mut self) {
        self.pending_sign_in = None;
        self.drop_socket();
        if let Some(auth) = self.auth.take() {
            let forgetful = auth.forget();
            self.vault = Some(self.spare_vault.take().unwrap_or(forgetful));
        }
        self.login = LoginStatus::default();
        self.events.send(Event::Login(LoginStatus::default())).ok();
        self.stopped = false;
        self.events.send(Event::Devices(Ok(Vec::new()))).ok();
    }

    async fn load_devices(&mut self) {
        let Some(access) = self.access(false).await else { return };
        let result = self.api.devices(&access).await;
        let devices: Result<Vec<DeviceSummary>, Problem> = match result {
            Ok(list) => Ok(list.devices),
            Err(e) => {
                self.cloud_error(&e);
                Err(self.problem.unwrap_or(Problem::Network))
            }
        };
        self.events.send(Event::Devices(devices)).ok();
    }

    async fn load_usage(&mut self) {
        let Some(access) = self.access(false).await else {
            self.events.send(Event::Usage(Err(self.problem.unwrap_or(Problem::Network)))).ok();
            return;
        };
        let usage = match self.api.usage(&access).await {
            Ok(report) => Ok(report),
            Err(e) => {
                self.cloud_error(&e);
                Err(self.problem.unwrap_or(Problem::Network))
            }
        };
        self.events.send(Event::Usage(usage)).ok();
    }

    async fn set_push_token(&mut self, token: String) {
        let pushed = match self.access(false).await {
            Some(access) => {
                match self.api.set_push_token(&access, &token).await {
                    // Registered and the Cloud has its Firebase key: it rings here.
                    Ok(reply) => Some(reply.registered && reply.push_configured && !token.is_empty()),
                    // Quietly: local reminders keep ringing until it works.
                    Err(e) if e.is_signed_out() => {
                        self.cloud_error(&e);
                        None
                    }
                    Err(e) => {
                        crate::diag::report!("Kabar Cloud push token: {e}");
                        None
                    }
                }
            }
            None => None,
        };
        self.events.send(Event::Push { token, pushed }).ok();
    }

    /// A fresh access token, or `None` (with the reason recorded).
    async fn access(&mut self, force: bool) -> Option<String> {
        if !leads() {
            // Only the newest tab touches the session: two tabs refreshing
            // one refresh token sign each other out (#562).
            return None;
        }
        let result = self.auth.as_mut()?.access_token(&self.api, now_ms(), force).await;
        match result {
            Ok(token) => Some(token),
            Err(e) => {
                self.cloud_error(&e);
                None
            }
        }
    }

    fn cloud_error(&mut self, e: &CloudError) {
        crate::diag::report!("Kabar Cloud: {e}");
        if e.is_signed_out() {
            self.problem = Some(Problem::SignedOutRemotely);
            self.sign_out_locally();
        } else {
            self.problem = Some(match e {
                CloudError::Network(_) => Problem::Network,
                _ => Problem::Server,
            });
        }
    }

    // --- socket ----------------------------------------------------------

    async fn connect(&mut self) {
        let force = std::mem::take(&mut self.force_refresh);
        let Some(access) = self.access(force).await else {
            self.schedule_retry();
            return;
        };
        let url = self.api.sync_url();
        self.opening = Some(Box::pin(async move {
            let within = crate::platform::time::sleep(CONNECT_WITHIN);
            tokio::select! {
                opened = Socket::connect(&url, &access) => opened,
                () = within => Err(ConnectError::Failed("the sync socket didn't open in time".into())),
            }
        }));
    }

    async fn opened(&mut self, opened: Result<Socket, ConnectError>) {
        match opened {
            Ok(ws) => {
                self.ws = Some(ws);
                self.problem = None;
                self.last_heard = Instant::now();
                self.asked_at = None;
                let effects = self.client.on_open();
                self.run_effects(effects).await;
            }
            Err(e) => {
                match e {
                    // Expired or revoked: refreshing tells which.
                    ConnectError::Unauthorized => self.force_refresh = true,
                    ConnectError::Failed(_) => {
                        self.problem = Some(Problem::Network);
                        // A browser doesn't say why a socket failed: the next
                        // try refreshes, in case the token was refused.
                        self.force_refresh = cfg!(target_arch = "wasm32");
                    }
                }
                crate::diag::report!("sync connect: {e}");
                self.schedule_retry();
            }
        }
    }

    fn schedule_retry(&mut self) {
        let jitter = (now_ms() % 1000) as f64 / 1000.0;
        self.retry_at = Instant::now() + Duration::from_millis(self.backoff.next_delay_ms(jitter));
    }

    fn drop_socket(&mut self) {
        self.ws = None;
        self.opening = None;
        self.asked_at = None;
        self.client.on_close();
        if self.wants_socket() {
            self.schedule_retry();
        }
    }

    async fn frame(&mut self, frame: Frame) {
        // Anything heard, a pong included, proves the socket is alive.
        self.last_heard = Instant::now();
        self.asked_at = None;
        let text = match frame {
            Frame::Text(text) => text,
            Frame::Closed(code) => {
                // 4001: signed out from another device (after `revoked`).
                if code == Some(4001) {
                    self.revoked();
                }
                self.drop_socket();
                return;
            }
            Frame::Other => return,
            Frame::Error(e) => {
                crate::diag::report!("sync socket: {e}");
                self.drop_socket();
                return;
            }
        };
        match serde_json::from_str::<ServerMsg>(&text) {
            Ok(msg) => {
                let effects = self.client.on_message(msg);
                self.run_effects(effects).await;
                if self.client.phase() == Phase::Live {
                    self.backoff.reset();
                }
                self.pump().await;
            }
            Err(e) => crate::diag::report!("unreadable sync message: {e}"),
        }
    }

    async fn run_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Send(msg) => self.send(&msg).await,
                Effect::Apply { ops, last_seq } => {
                    match self.store.with(|s| s.apply_remote(&ops, last_seq)) {
                        Ok(changed) => {
                            self.touch_last_sync();
                            if !changed.is_empty() {
                                self.events.send(Event::Changed(changed)).ok();
                            }
                        }
                        Err(e) => {
                            // Not saved, so the next hello brings them again.
                            crate::diag::report!("applying remote ops: {e}");
                            self.drop_socket();
                            return;
                        }
                    }
                }
                Effect::SaveLastSeq(seq) => {
                    let _ = self.store.with(|s| s.set_last_seq(seq));
                }
                Effect::Acked { done, rejected } => {
                    for r in &rejected {
                        crate::diag::report!("op {} rejected: {:?}", r.op_id, r.reason);
                    }
                    let ids =
                        done.iter().map(String::as_str).chain(rejected.iter().map(|r| r.op_id.as_str()));
                    if let Err(e) = self.store.with(|s| s.remove_from_outbox(ids)) {
                        crate::diag::report!("outbox: {e}");
                    }
                    self.touch_last_sync();
                }
                Effect::Reconnect { reason } => {
                    crate::diag::report!("sync reconnects: {reason}");
                    if let Some(ws) = self.ws.take() {
                        ws.close().await;
                    }
                    self.drop_socket();
                }
                Effect::Stop => {
                    if self.client.phase() == Phase::Revoked {
                        self.revoked();
                    } else {
                        self.problem = Some(Problem::Outdated);
                        self.stopped = true;
                    }
                    if let Some(ws) = self.ws.take() {
                        ws.close().await;
                    }
                    self.client.on_close();
                }
            }
        }
    }

    fn revoked(&mut self) {
        self.problem = Some(Problem::SignedOutRemotely);
        self.sign_out_locally();
    }

    /// Pushes the next outbox batch if the socket can take it.
    async fn pump(&mut self) {
        if self.ws.is_none() {
            return;
        }
        let outbox = match self.store.with(|s| s.outbox(MAX_OPS_PER_PUSH)) {
            Ok(o) => o,
            Err(e) => {
                crate::diag::report!("reading outbox: {e}");
                return;
            }
        };
        if let Some(msg) = self.client.next_push(&outbox) {
            self.send(&msg).await;
        }
    }

    async fn send(&mut self, msg: &kabar_protocol::sync::ClientMsg) {
        let Some(ws) = &mut self.ws else { return };
        let text = serde_json::to_string(msg).unwrap_or_default();
        if let Err(e) = ws.send(text).await {
            crate::diag::report!("sync send: {e}");
            self.drop_socket();
        }
    }

    fn touch_last_sync(&mut self) {
        let _ = self.store.with(|s| s.set_meta("last_sync_ms", Some(&now_ms().to_string())));
    }

    fn emit_status(&mut self) {
        let sync = match (&self.auth, self.client.phase(), &self.ws) {
            (None, _, _) => SyncState::SignedOut,
            (Some(_), _, _) if self.stopped => SyncState::Stopped,
            (Some(_), Phase::Live, Some(_)) => SyncState::Live,
            (Some(_), Phase::CatchingUp, Some(_)) => SyncState::CatchingUp,
            (Some(_), _, None) if Instant::now() < self.retry_at => SyncState::Offline,
            (Some(_), _, _) => SyncState::Connecting,
        };
        let status = CloudStatus {
            configured: true,
            email: self.auth.as_ref().and_then(|a| a.session().email.clone()),
            device_id: self.device.device_id.clone(),
            sync,
            pending: self.store.with(|s| s.outbox_len()).unwrap_or(0),
            last_sync_ms: self
                .store
                .with(|s| s.meta("last_sync_ms"))
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok()),
            problem: self.problem,
            temporary: self.auth.as_ref().is_some_and(|a| !a.remembered()),
        };
        if self.last_status.as_ref() != Some(&status) {
            self.last_status = Some(status.clone());
            self.events.send(Event::Status(status)).ok();
        }
    }
}

/// On the web, only the tab that owns the database syncs and refreshes the
/// session; an older tab shows "open in another tab" (`platform::web`).
fn leads() -> bool {
    #[cfg(target_arch = "wasm32")]
    return crate::platform::web::owns_tab();
    #[cfg(not(target_arch = "wasm32"))]
    true
}

async fn opened(opening: &mut Option<Opening>) -> Result<Socket, ConnectError> {
    match opening {
        Some(f) => f.await,
        None => std::future::pending().await,
    }
}

async fn next_frame(ws: &mut Option<Socket>) -> Frame {
    match ws {
        Some(ws) => ws.next().await,
        None => std::future::pending().await,
    }
}

/// Stretches the password (`password::password_key`) off the UI thread
/// where there is one. The browser has one thread: a short nap first lets
/// the page show that it's signing in before the work starts.
async fn stretch(username: String, password: String) -> Result<String, String> {
    #[cfg(not(target_arch = "wasm32"))]
    return tokio::task::spawn_blocking(move || password::password_key(&username, &password))
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(target_arch = "wasm32")]
    {
        crate::platform::time::sleep(Duration::from_millis(50)).await;
        password::password_key(&username, &password)
    }
}

/// What a password answer means for the learner, when it's about the password.
fn password_problem(e: &CloudError) -> Option<Problem> {
    match e {
        CloudError::Api { code: ErrorCode::WrongPassword, .. } => Some(Problem::WrongPassword),
        CloudError::Api { code: ErrorCode::UsernameTaken, .. } => Some(Problem::UsernameTaken),
        CloudError::Api { code: ErrorCode::TooManyTries, retry_after_ms, .. } => {
            Some(Problem::Locked(retry_after_ms.unwrap_or(60_000).div_ceil(60_000).max(1)))
        }
        _ => None,
    }
}

pub fn now_ms() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod password_tests {
    use super::*;

    #[test]
    fn password_answers_become_their_own_problems() {
        let api = |status, body: &str| super::super::api::api_error(status, body);
        assert_eq!(
            password_problem(&api(401, r#"{"code":"wrong_password","message":""}"#)),
            Some(Problem::WrongPassword)
        );
        assert_eq!(
            password_problem(&api(409, r#"{"code":"username_taken","message":""}"#)),
            Some(Problem::UsernameTaken)
        );
        // Minutes, rounded up: 61 seconds is "2 minutes", never "0".
        let locked = |ms: u64| {
            password_problem(&api(
                429,
                &format!(r#"{{"code":"too_many_tries","message":"","retry_after_ms":{ms}}}"#),
            ))
        };
        assert_eq!(locked(61_000), Some(Problem::Locked(2)));
        assert_eq!(locked(1), Some(Problem::Locked(1)));
        assert_eq!(password_problem(&api(401, r#"{"code":"unauthorized","message":""}"#)), None);
        assert_eq!(password_problem(&CloudError::Network("down".into())), None);
    }

    #[test]
    fn a_waiting_sign_in_retries_quickly_then_every_fifteen_seconds() {
        let mut p = PendingSignIn::new(SignInStep::IdToken("t".into()), ID_TOKEN_LIFE);
        assert!(p.retry_at <= Instant::now(), "the first try is immediate");
        let delays: Vec<u64> = (0..7)
            .map(|_| {
                p.schedule_retry();
                // Rounded: a few microseconds pass between the two clock reads.
                (p.retry_at - Instant::now()).as_secs_f64().round() as u64
            })
            .collect();
        assert_eq!(delays, [1, 2, 4, 8, 15, 15, 15]);
        assert!(p.until - Instant::now() > CODE_LIFE, "an ID token outlives a code");
    }
}
