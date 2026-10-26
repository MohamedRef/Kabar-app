//! The sync engine: one task that owns the session, the WebSocket and its own
//! database connection. It drives [`SyncClient`] and reports to the UI.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use kabar_core::sync_client::{Backoff, Effect, Phase, SyncClient};
use kabar_protocol::cloud::{DeviceInfo, DeviceSummary, ErrorCode, GoogleSignIn};
use kabar_protocol::sync::{MAX_OPS_PER_PUSH, ServerMsg};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::api::{Api, CloudError};
use super::oauth::{self, GoogleClient, OAuthError};
use super::session::{Auth, SessionVault};
use super::{CloudStatus, Command, Event, Problem, SyncState};
use crate::store::Store;

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Keeps NAT mappings and the Cloudflare edge from dropping an idle socket.
/// Protocol pings are answered by the runtime, so they don't wake the hub.
const PING_EVERY: Duration = Duration::from_secs(30);
/// Writes from screens that don't send [`Command::Kick`] still go up this soon.
const OUTBOX_EVERY: Duration = Duration::from_secs(2);

pub struct Engine {
    store: Store,
    api: Api,
    google: Option<GoogleClient>,
    device: DeviceInfo,
    auth: Option<Auth>,
    /// Kept while signed out, for the next sign-in.
    vault: Option<Box<dyn SessionVault>>,
    client: SyncClient,
    ws: Option<Ws>,
    backoff: Backoff,
    retry_at: Instant,
    /// Set after `Revoked`, `Outdated` or sign-out: no reconnects until sign-in.
    stopped: bool,
    force_refresh: bool,
    problem: Option<Problem>,
    events: UnboundedSender<Event>,
}

impl Engine {
    pub fn new(
        store: Store,
        api: Api,
        google: Option<GoogleClient>,
        device: DeviceInfo,
        vault: Box<dyn SessionVault>,
        events: UnboundedSender<Event>,
    ) -> Self {
        let client = SyncClient::new(store.last_seq().unwrap_or(0));
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
            backoff: Backoff::default(),
            retry_at: Instant::now(),
            stopped: false,
            force_refresh: false,
            problem: None,
            events,
        }
    }

    pub async fn run(mut self, mut commands: UnboundedReceiver<Command>) {
        let mut ping = tokio::time::interval(PING_EVERY);
        let mut outbox = tokio::time::interval(OUTBOX_EVERY);
        self.emit_status();
        loop {
            if self.ws.is_none() && self.wants_socket() && Instant::now() >= self.retry_at {
                self.connect().await;
                self.emit_status();
            }
            let wait_until = if self.ws.is_none() && self.wants_socket() {
                self.retry_at
            } else {
                Instant::now() + Duration::from_secs(3600)
            };
            tokio::select! {
                cmd = commands.recv() => match cmd {
                    None | Some(Command::Shutdown) => break,
                    Some(cmd) => self.command(cmd).await,
                },
                frame = next_frame(&mut self.ws) => self.frame(frame).await,
                _ = tokio::time::sleep_until(wait_until) => {}
                _ = outbox.tick() => self.pump().await,
                _ = ping.tick() => {
                    if let Some(ws) = &mut self.ws
                        && ws.send(Message::Ping(Vec::new().into())).await.is_err()
                    {
                        self.drop_socket();
                    }
                }
            }
            self.emit_status();
        }
        if let Some(mut ws) = self.ws.take() {
            let _ = ws.close(None).await;
        }
    }

    fn wants_socket(&self) -> bool {
        self.auth.is_some() && !self.stopped
    }

    // --- commands --------------------------------------------------------

    async fn command(&mut self, cmd: Command) {
        match cmd {
            Command::SignInGoogle => {
                let Some(google) = self.google.clone() else {
                    self.problem = Some(Problem::NotConfigured);
                    return;
                };
                self.problem = None;
                self.events.send(Event::SigningIn(true)).ok();
                let token = oauth::google_id_token(&google).await;
                self.events.send(Event::SigningIn(false)).ok();
                match token {
                    Ok(token) => self.sign_in(token).await,
                    Err(OAuthError::Cancelled) => self.problem = Some(Problem::SignInCancelled),
                    Err(e) => {
                        eprintln!("kabar: Google sign-in: {e}");
                        self.problem = Some(Problem::SignInFailed);
                    }
                }
            }
            Command::SignInWithIdToken(token) => self.sign_in(token).await,
            Command::SignOut => {
                if let Some(access) = self.access(false).await {
                    let api = self.api.clone();
                    let _ = blocking(move || api.sign_out(&access)).await;
                }
                self.sign_out_locally();
            }
            Command::LoadDevices => self.load_devices().await,
            Command::LoadUsage => self.load_usage().await,
            Command::RevokeDevice(device) => {
                if device == self.device.device_id {
                    return Box::pin(self.command(Command::SignOut)).await;
                }
                if let Some(access) = self.access(false).await {
                    let api = self.api.clone();
                    if let Err(e) = blocking(move || api.revoke(&access, &device)).await {
                        self.cloud_error(&e);
                    }
                }
                self.load_devices().await;
            }
            Command::Kick => self.pump().await,
            Command::Access { force, reply } => {
                let access = self.access(force).await;
                reply.send(access.map(|token| (self.api.clone(), token))).ok();
            }
            Command::Shutdown => {}
        }
    }

    async fn sign_in(&mut self, id_token: String) {
        let body = GoogleSignIn { id_token, device: self.device.clone() };
        let api = self.api.clone();
        match blocking(move || api.sign_in_google(&body)).await {
            Ok(session) => {
                let vault =
                    self.vault.take().unwrap_or_else(|| Box::new(super::session::MemoryVault::default()));
                self.auth = Some(Auth::new(session, vault));
                self.problem = None;
                self.stopped = false;
                self.client = SyncClient::new(self.store.last_seq().unwrap_or(0));
                self.backoff.reset();
                self.retry_at = Instant::now();
            }
            Err(e) => {
                eprintln!("kabar: Kabar Cloud sign-in: {e}");
                self.problem = Some(match &e {
                    CloudError::Api { code: ErrorCode::NotAllowed, .. } => Problem::NotAllowed,
                    CloudError::Network(_) => Problem::Network,
                    _ => Problem::SignInFailed,
                });
            }
        }
    }

    fn sign_out_locally(&mut self) {
        self.drop_socket();
        if let Some(auth) = self.auth.take() {
            self.vault = Some(auth.forget());
        }
        self.stopped = false;
        self.events.send(Event::Devices(Ok(Vec::new()))).ok();
    }

    async fn load_devices(&mut self) {
        let Some(access) = self.access(false).await else { return };
        let api = self.api.clone();
        let result = blocking(move || api.devices(&access)).await;
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
        let api = self.api.clone();
        let usage = match blocking(move || api.usage(&access)).await {
            Ok(report) => Ok(report),
            Err(e) => {
                self.cloud_error(&e);
                Err(self.problem.unwrap_or(Problem::Network))
            }
        };
        self.events.send(Event::Usage(usage)).ok();
    }

    /// A fresh access token, or `None` (with the reason recorded).
    async fn access(&mut self, force: bool) -> Option<String> {
        let mut auth = self.auth.take()?;
        let api = self.api.clone();
        let (auth, result) = blocking(move || {
            let r = auth.access_token(&api, now_ms(), force);
            (auth, r)
        })
        .await;
        self.auth = Some(auth);
        match result {
            Ok(token) => Some(token),
            Err(e) => {
                self.cloud_error(&e);
                None
            }
        }
    }

    fn cloud_error(&mut self, e: &CloudError) {
        eprintln!("kabar: Kabar Cloud: {e}");
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
        let mut request = match self.api.sync_url().into_client_request() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("kabar: bad sync url: {e}");
                self.stopped = true;
                return;
            }
        };
        if let Ok(v) = format!("Bearer {access}").parse() {
            request.headers_mut().insert("authorization", v);
        }
        match tokio_tungstenite::connect_async(request).await {
            Ok((ws, _)) => {
                self.ws = Some(ws);
                self.problem = None;
                let effects = self.client.on_open();
                self.run_effects(effects).await;
            }
            Err(e) => {
                use tokio_tungstenite::tungstenite::Error;
                if let Error::Http(res) = &e
                    && res.status().as_u16() == 401
                {
                    // Expired or revoked: refreshing tells which.
                    self.force_refresh = true;
                } else {
                    self.problem = Some(Problem::Network);
                }
                eprintln!("kabar: sync connect: {e}");
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
        self.client.on_close();
        if self.wants_socket() {
            self.schedule_retry();
        }
    }

    async fn frame(&mut self, frame: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>) {
        let text = match frame {
            Some(Ok(Message::Text(text))) => text,
            Some(Ok(Message::Close(close))) => {
                // 4001: signed out from another device (after `revoked`).
                if close.as_ref().is_some_and(|c| u16::from(c.code) == 4001) {
                    self.revoked();
                }
                self.drop_socket();
                return;
            }
            Some(Ok(_)) => return,
            Some(Err(e)) => {
                eprintln!("kabar: sync socket: {e}");
                self.drop_socket();
                return;
            }
            None => {
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
            Err(e) => eprintln!("kabar: unreadable sync message: {e}"),
        }
    }

    async fn run_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Send(msg) => self.send(&msg).await,
                Effect::Apply { ops, last_seq } => match self.store.apply_remote(&ops, last_seq) {
                    Ok(changed) => {
                        self.touch_last_sync();
                        if !changed.is_empty() {
                            self.events.send(Event::Changed(changed)).ok();
                        }
                    }
                    Err(e) => {
                        // Not saved, so the next hello brings them again.
                        eprintln!("kabar: applying remote ops: {e}");
                        self.drop_socket();
                        return;
                    }
                },
                Effect::SaveLastSeq(seq) => {
                    let _ = self.store.set_last_seq(seq);
                }
                Effect::Acked { done, rejected } => {
                    for r in &rejected {
                        eprintln!("kabar: op {} rejected: {:?}", r.op_id, r.reason);
                    }
                    let ids =
                        done.iter().map(String::as_str).chain(rejected.iter().map(|r| r.op_id.as_str()));
                    if let Err(e) = self.store.remove_from_outbox(ids) {
                        eprintln!("kabar: outbox: {e}");
                    }
                    self.touch_last_sync();
                }
                Effect::Reconnect { reason } => {
                    eprintln!("kabar: sync reconnects: {reason}");
                    if let Some(mut ws) = self.ws.take() {
                        let _ = ws.close(None).await;
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
                    if let Some(mut ws) = self.ws.take() {
                        let _ = ws.close(None).await;
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
        let outbox = match self.store.outbox(MAX_OPS_PER_PUSH) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("kabar: reading outbox: {e}");
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
        if let Err(e) = ws.send(Message::Text(text.into())).await {
            eprintln!("kabar: sync send: {e}");
            self.drop_socket();
        }
    }

    fn touch_last_sync(&mut self) {
        let _ = self.store.set_meta("last_sync_ms", Some(&now_ms().to_string()));
    }

    fn emit_status(&self) {
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
            pending: self.store.outbox_len().unwrap_or(0),
            last_sync_ms: self.store.meta("last_sync_ms").ok().flatten().and_then(|v| v.parse().ok()),
            problem: self.problem,
        };
        self.events.send(Event::Status(status)).ok();
    }
}

async fn next_frame(ws: &mut Option<Ws>) -> Option<Result<Message, tokio_tungstenite::tungstenite::Error>> {
    match ws {
        Some(ws) => ws.next().await,
        None => std::future::pending().await,
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.expect("blocking task panicked")
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
