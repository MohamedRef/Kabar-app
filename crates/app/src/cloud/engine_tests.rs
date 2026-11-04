//! The sync engine against the local hub ([`FakeCloud`]): offline queues,
//! last-writer-wins between two live devices, reconnects, remote sign-out,
//! an outdated app, and the Devices, usage, push and proxy calls.

use std::sync::atomic::Ordering;
use std::time::Duration;

use kabar_protocol::cloud::{Platform, Session};
use kabar_protocol::reader::WordStatus;
use kabar_protocol::sync::OpKind;
use tokio::sync::oneshot;

use super::e2e::Device;
use super::fake::FakeCloud;
use super::link::{CloudLink, Unreachable};
use super::session::{MemoryVault, SessionVault};
use super::{Command, Event, Problem, SyncState};

const EMAIL: &str = "learner@example.com";

fn signed_in(dir: &std::path::Path, cloud: &FakeCloud, name: &str) -> Device {
    let mut d = Device::start(dir, name, Platform::Android, &cloud.url);
    d.send(Command::SignInWithIdToken(FakeCloud::id_token(EMAIL)));
    d.wait_live();
    d
}

fn word(d: &mut Device, key: &str, status: &str) {
    let (lang, lexeme) = key.split_once(':').unwrap();
    let payload = serde_json::json!({ "lexeme": lexeme, "lang": lang, "status": status });
    d.store.write_synced("word_status", key, OpKind::Upsert, Some(payload)).unwrap();
    d.send(Command::Kick);
}

fn status_of(d: &Device, key: &str) -> WordStatus {
    d.store.word_statuses([key]).unwrap()[key]
}

/// Waits until `f` holds, polling (for state the engine changes on its own
/// thread without an event this side can wait on).
fn eventually(what: &str, mut f: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !f() {
        assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn access(d: &Device, force: bool) -> Option<String> {
    let (reply, answer) = oneshot::channel();
    d.send(Command::Access { force, reply });
    answer.blocking_recv().unwrap().map(|(_, token)| token)
}

#[test]
fn a_backgrounded_device_queues_writes_and_catches_up_when_back() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let mut tablet = signed_in(dir.path(), &cloud, "tablet");
    assert_eq!(cloud.sockets(), 2);

    // The tablet goes into a drawer: its socket closes.
    tablet.send(Command::Background(true));
    eventually("tablet socket closed", || cloud.sockets() == 1);

    // The phone keeps learning; the tablet writes offline too.
    word(&mut phone, "id:rumah", "learning");
    word(&mut phone, "id:makan", "known");
    phone.wait_status("phone pushed", |s| s.pending == 0 && s.sync == SyncState::Live);
    eventually("hub has the phone's words", || cloud.row("word_status", "id:makan").is_some());
    word(&mut tablet, "jv:omah", "learning");
    let queued = tablet.wait_status("tablet queued", |s| s.pending == 1);
    assert_ne!(queued.sync, SyncState::Live);
    assert_eq!(cloud.row("word_status", "jv:omah"), None, "nothing leaves a backgrounded device");

    // Back in hand: the tablet catches up and its queue flushes.
    tablet.send(Command::Background(false));
    tablet.wait_live();
    tablet.wait_status("tablet flushed", |s| s.pending == 0);
    assert_eq!(status_of(&tablet, "id:rumah"), WordStatus::Learning);
    assert_eq!(status_of(&tablet, "id:makan"), WordStatus::Known);
    assert_eq!(cloud.row("word_status", "jv:omah").unwrap()["status"], "learning");
    phone
        .wait("tablet's word", |e| matches!(e, Event::Changed(t) if t.contains("word_status")).then_some(()));
    assert_eq!(status_of(&phone, "jv:omah"), WordStatus::Learning);
    assert!(tablet.store.meta("last_sync_ms").unwrap().is_some());
}

#[test]
fn the_newer_write_wins_on_both_devices() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let mut tablet = signed_in(dir.path(), &cloud, "tablet");
    // Both edit the same word while the tablet is away; the tablet's edit
    // comes later, so it wins everywhere, though it reaches the hub last.
    tablet.send(Command::Background(true));
    eventually("tablet socket closed", || cloud.sockets() == 1);
    word(&mut phone, "id:rumah", "learning");
    phone.wait_status("phone pushed", |s| s.pending == 0);
    std::thread::sleep(Duration::from_millis(5));
    word(&mut tablet, "id:rumah", "ignored");
    tablet.send(Command::Background(false));
    tablet.wait_status("tablet flushed", |s| s.pending == 0 && s.sync == SyncState::Live);
    phone
        .wait("tablet's edit", |e| matches!(e, Event::Changed(t) if t.contains("word_status")).then_some(()));
    assert_eq!(status_of(&tablet, "id:rumah"), WordStatus::Ignored);
    assert_eq!(status_of(&phone, "id:rumah"), WordStatus::Ignored);
    assert_eq!(cloud.row("word_status", "id:rumah").unwrap()["status"], "ignored");
}

#[test]
fn a_dropped_socket_reconnects_and_a_refused_one_refreshes_the_token() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let id = phone.store.device_id().clone();

    // The edge drops the socket: the engine is back on its own.
    cloud.state.knobs.refuse_sockets.store(1, Ordering::SeqCst);
    cloud.close_sockets(&id, 1011);
    phone.wait_status("offline", |s| s.sync != SyncState::Live);
    phone.wait_live();
    // The refused handshake made it refresh before the next try.
    assert!(cloud.requested("POST /v1/auth/refresh") >= 1);
    assert_eq!(cloud.sockets(), 1);
    word(&mut phone, "id:rumah", "known");
    phone.wait_status("pushed after reconnect", |s| s.pending == 0);
    eventually("hub has it", || cloud.row("word_status", "id:rumah").is_some());
}

#[test]
fn signing_out_elsewhere_signs_this_device_out() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let id = phone.store.device_id().clone();
    cloud.close_sockets(&id, 4001);
    let s = phone.wait_status("signed out", |s| s.sync == SyncState::SignedOut);
    assert_eq!(s.problem, Some(Problem::SignedOutRemotely));
    assert_eq!(s.email, None);
    assert_eq!(access(&phone, false), None);
}

#[test]
fn an_outdated_app_stops_syncing_until_updated() {
    let cloud = FakeCloud::start();
    cloud.state.knobs.outdated.store(true, Ordering::SeqCst);
    let dir = tempfile::tempdir().unwrap();
    let mut phone = Device::start(dir.path(), "phone", Platform::Android, &cloud.url);
    phone.send(Command::SignInWithIdToken(FakeCloud::id_token(EMAIL)));
    let s = phone.wait_status("stopped", |s| s.sync == SyncState::Stopped);
    assert_eq!(s.problem, Some(Problem::Outdated));
    assert_eq!(s.email.as_deref(), Some(EMAIL));
    // It doesn't keep knocking.
    eventually("socket closed", || cloud.sockets() == 0);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(cloud.sockets(), 0);
}

#[test]
fn sign_in_problems_are_named() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = Device::start(dir.path(), "phone", Platform::Android, &cloud.url);
    phone.wait_status("signed out at start", |s| s.sync == SyncState::SignedOut && s.configured);
    phone.send(Command::SignInWithIdToken("someone-else".into()));
    let s = phone.wait_status("not allowed", |s| s.problem.is_some());
    assert_eq!((s.sync, s.problem), (SyncState::SignedOut, Some(Problem::NotAllowed)));
    // No Google client in this build.
    phone.send(Command::SignInGoogle);
    let s = phone.wait_status("not configured", |s| s.problem == Some(Problem::NotConfigured));
    assert_eq!(s.sync, SyncState::SignedOut);

    // Nothing listens on the port: a network problem, not a refusal.
    let gone = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", gone.local_addr().unwrap());
    drop(gone);
    let mut offline = Device::start(dir.path(), "offline", Platform::Macos, &url);
    offline.send(Command::SignInWithIdToken(FakeCloud::id_token(EMAIL)));
    let s = offline.wait_status("network", |s| s.problem.is_some());
    assert_eq!((s.sync, s.problem), (SyncState::SignedOut, Some(Problem::Network)));
}

/// A vault that can't keep anything (a locked Keystore).
struct BrokenVault;

impl SessionVault for BrokenVault {
    fn load(&self) -> Option<Session> {
        None
    }
    fn save(&self, _: &Session) -> bool {
        false
    }
    fn clear(&self) {}
}

#[test]
fn a_session_that_cannot_be_kept_is_a_failed_sign_in() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone =
        Device::start_with(dir.path(), "phone", Platform::Android, &cloud.url, Box::new(BrokenVault));
    phone.send(Command::SignInWithIdToken(FakeCloud::id_token(EMAIL)));
    let s = phone.wait_status("failed", |s| s.problem.is_some());
    assert_eq!((s.sync, s.problem), (SyncState::SignedOut, Some(Problem::SignInFailed)));
}

#[test]
fn a_kept_session_signs_in_at_launch_and_refreshes_when_due() {
    let cloud = FakeCloud::start();
    // Every token the cloud hands out is already due for a refresh.
    cloud.state.knobs.short_tokens.store(true, Ordering::SeqCst);
    let dir = tempfile::tempdir().unwrap();
    let vault = std::sync::Arc::new(MemoryVault::default());
    struct Shared(std::sync::Arc<MemoryVault>);
    impl SessionVault for Shared {
        fn load(&self) -> Option<Session> {
            self.0.load()
        }
        fn save(&self, s: &Session) -> bool {
            self.0.save(s)
        }
        fn clear(&self) {
            self.0.clear()
        }
    }
    let mut first =
        Device::start_with(dir.path(), "mac", Platform::Macos, &cloud.url, Box::new(Shared(vault.clone())));
    first.send(Command::SignInWithIdToken(FakeCloud::id_token(EMAIL)));
    first.wait_live();
    let kept = vault.load().unwrap();
    first.send(Command::Shutdown);
    eventually("first engine closed its socket", || cloud.sockets() == 0);

    // The next launch finds the session and goes live without a sign-in.
    let mut again =
        Device::start_with(dir.path(), "mac", Platform::Macos, &cloud.url, Box::new(Shared(vault.clone())));
    again.wait_live();
    assert_eq!(cloud.requested("POST /v1/auth/google"), 1);
    // The due token was refreshed, and the rotated refresh token kept.
    assert_ne!(vault.load().unwrap().refresh_token, kept.refresh_token);
    assert!(access(&again, false).is_some());

    // Signing out tells the cloud and forgets the session here.
    again.send(Command::SignOut);
    again.wait_status("signed out", |s| s.sync == SyncState::SignedOut);
    assert_eq!(cloud.requested("POST /v1/auth/signout"), 1);
    assert_eq!(vault.load(), None);
}

#[test]
fn devices_usage_and_push_answers_reach_the_ui() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let id = phone.store.device_id().clone();

    phone.send(Command::LoadUsage);
    let usage = phone.wait("usage", |e| match e {
        Event::Usage(u) => Some(u.clone()),
        _ => None,
    });
    let usage = usage.unwrap();
    assert!(usage.hard_stops);
    assert_eq!(usage.spend_usd_cents, 0);

    phone.send(Command::PushToken("fcm-token-1".into()));
    let pushed = phone.wait("push", |e| match e {
        Event::Push { token, pushed } => Some((token.clone(), *pushed)),
        _ => None,
    });
    assert_eq!(pushed, ("fcm-token-1".into(), Some(true)));
    assert_eq!(cloud.push_token(&id).as_deref(), Some("fcm-token-1"));
    // An empty token turns pushes off here: local reminders take over.
    phone.send(Command::PushToken(String::new()));
    let pushed = phone.wait("push off", |e| match e {
        Event::Push { pushed, .. } => Some(*pushed),
        _ => None,
    });
    assert_eq!(pushed, Some(false));
    assert_eq!(cloud.push_token(&id), None);

    // A broken edge (a 502 page): the screens hear "server", not "offline".
    cloud.state.knobs.http_status.store(502, Ordering::SeqCst);
    phone.send(Command::LoadDevices);
    let devices = phone.wait("devices", |e| match e {
        Event::Devices(d) => Some(d.clone()),
        _ => None,
    });
    assert_eq!(devices, Err(Problem::Server));
    phone.send(Command::LoadUsage);
    let usage = phone.wait("usage error", |e| match e {
        Event::Usage(u) => Some(u.clone()),
        _ => None,
    });
    assert_eq!(usage, Err(Problem::Server));
    phone.send(Command::PushToken("fcm-token-2".into()));
    let pushed = phone.wait("push unknown", |e| match e {
        Event::Push { pushed, .. } => Some(*pushed),
        _ => None,
    });
    assert_eq!(pushed, None, "couldn't be asked");
}

#[test]
fn revoking_this_device_from_its_own_devices_screen_signs_out() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let id = phone.store.device_id().clone();
    phone.send(Command::RevokeDevice(id));
    let s = phone.wait_status("signed out", |s| s.sync == SyncState::SignedOut);
    assert_eq!(s.problem, None, "the learner asked for it");
    assert_eq!(cloud.requested("POST /v1/auth/signout"), 1);
    // Signed out, the devices list is empty and calls go nowhere.
    phone.send(Command::LoadUsage);
    let usage = phone.wait("usage", |e| match e {
        Event::Usage(u) => Some(u.clone()),
        _ => None,
    });
    assert_eq!(usage, Err(Problem::Network));
}

#[test]
fn the_proxies_answer_with_the_session_and_retry_a_refused_token() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let phone = signed_in(dir.path(), &cloud, "phone");
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    let link = CloudLink::default();
    // Not linked to the engine yet: this build has no cloud.
    let png = b"\x89PNG\r\n\x1a\nkabar".to_vec();
    assert_eq!(runtime.block_on(link.put_blob(png.clone())), Err(Unreachable::NotConfigured));
    link.connect(phone.handle.commands.clone());

    let sha = runtime.block_on(link.put_blob(png.clone())).unwrap();
    assert_eq!(cloud.state.blobs.lock().unwrap().get(&sha), Some(&png));
    let key = kabar_protocol::ai::CacheKey {
        task: kabar_protocol::ai::Task::RoleplayTurn,
        input_hash: "h".into(),
        engine: kabar_protocol::ai::EngineId::WorkersAi,
        prompt_version: 1,
        profile_hash: String::new(),
    };
    assert_eq!(runtime.block_on(link.ai_reject(key.clone())), Ok(()));

    // The cloud forgets the access token (a restart of its key): the first
    // call is refused, the link refreshes once and the call goes through.
    let refreshes = cloud.requested("POST /v1/auth/refresh");
    cloud.forget_access_tokens();
    assert_eq!(runtime.block_on(link.ai_reject(key)), Ok(()));
    assert_eq!(cloud.requested("POST /v1/auth/refresh"), refreshes + 1);
    assert_eq!(cloud.requested("POST /v1/ai/reject"), 3);
}

/// Saves until told not to (a Keystore that locks mid-session).
struct Flaky(std::sync::Arc<std::sync::atomic::AtomicBool>, MemoryVault);

impl SessionVault for Flaky {
    fn load(&self) -> Option<Session> {
        self.1.load()
    }
    fn save(&self, s: &Session) -> bool {
        self.0.load(Ordering::SeqCst) && self.1.save(s)
    }
    fn clear(&self) {
        self.1.clear()
    }
}

#[test]
fn a_refresh_the_vault_cannot_keep_still_answers_and_keeps_the_old_refresh_token() {
    use super::api::Api;
    use super::session::Auth;
    let cloud = FakeCloud::start();
    cloud.state.knobs.short_tokens.store(true, Ordering::SeqCst);
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let api = Api::new(&cloud.url);
    let device = kabar_protocol::cloud::DeviceInfo {
        device_id: kabar_protocol::DeviceId("s24u".into()),
        name: "Galaxy S24 Ultra".into(),
        model: None,
        platform: Platform::Android,
    };
    let body = kabar_protocol::cloud::GoogleSignIn { id_token: FakeCloud::id_token(EMAIL), device };
    let session = rt.block_on(api.sign_in_google(&body)).unwrap();
    let saves = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut auth =
        Auth::new(session.clone(), Box::new(Flaky(saves.clone(), MemoryVault::default()))).ok().unwrap();

    saves.store(false, Ordering::SeqCst);
    let token = rt.block_on(auth.access_token(&api, super::engine::now_ms(), false)).unwrap();
    assert_ne!(token, session.access_token, "a fresh access token");
    assert_eq!(auth.session().refresh_token, session.refresh_token, "the one the vault still holds");
    assert_eq!(auth.session().refresh_expires_at_ms, session.refresh_expires_at_ms);

    // A kept session whose refresh token ran out doesn't sign in.
    let stale = Session { refresh_expires_at_ms: 5, ..session };
    let vault = MemoryVault::default();
    vault.save(&stale);
    assert!(Auth::restore(Box::new(vault), 10).is_err());
}

#[test]
fn the_desktop_credential_store_keeps_and_forgets_the_session() {
    let vault = super::session::KeyringVault::new();
    let s = Session {
        account_id: "acc".into(),
        device_id: kabar_protocol::DeviceId("linux".into()),
        email: Some(EMAIL.into()),
        access_token: "a".into(),
        access_expires_at_ms: 1,
        refresh_token: "r".into(),
        refresh_expires_at_ms: 2,
    };
    assert!(vault.save(&s));
    assert_eq!(vault.load(), Some(s));
    vault.clear();
    assert_eq!(vault.load(), None);
}

#[test]
fn ops_the_hub_refuses_for_good_leave_the_outbox() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    cloud.state.knobs.reject_pushes.store(true, Ordering::SeqCst);
    word(&mut phone, "id:rumah", "known");
    // Not sent again and again: the outbox empties, the word stays here.
    phone.wait_status("outbox empty", |s| s.pending == 0);
    assert_eq!(cloud.row("word_status", "id:rumah"), None);
    assert_eq!(status_of(&phone, "id:rumah"), WordStatus::Known);
}

#[test]
fn a_hub_error_or_garbage_reconnects_and_catches_up() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let mut tablet = signed_in(dir.path(), &cloud, "tablet");
    let id = tablet.store.device_id().clone();
    // A frame nobody can read is skipped; the socket stays up.
    cloud.send_text(&id, "{not json");
    word(&mut phone, "id:makan", "learning");
    tablet.wait("word", |e| matches!(e, Event::Changed(t) if t.contains("word_status")).then_some(()));
    // An error from the hub: the tablet reconnects, then hears the next word.
    cloud.send_text(&id, r#"{"type":"error","code":"internal","message":"hub restarted"}"#);
    tablet.wait_status("reconnecting", |s| s.sync != SyncState::Live);
    tablet.wait_live();
    word(&mut phone, "id:rumah", "known");
    tablet.wait("next word", |e| matches!(e, Event::Changed(t) if t.contains("word_status")).then_some(()));
    eventually("both words", || status_of(&tablet, "id:rumah") == WordStatus::Known);
    assert_eq!(status_of(&tablet, "id:makan"), WordStatus::Learning);
}

#[test]
fn a_socket_that_will_not_open_is_a_network_problem_until_it_does() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    cloud.state.knobs.socket_status.store(503, Ordering::SeqCst);
    cloud.state.knobs.refuse_sockets.store(1, Ordering::SeqCst);
    let mut phone = Device::start(dir.path(), "phone", Platform::Android, &cloud.url);
    phone.send(Command::SignInWithIdToken(FakeCloud::id_token(EMAIL)));
    let s = phone.wait_status("offline", |s| s.problem == Some(Problem::Network));
    assert_eq!(s.email.as_deref(), Some(EMAIL), "still signed in");
    // The next try gets through, and the problem clears.
    let s = phone.wait_status("live", |s| s.sync == SyncState::Live);
    assert_eq!(s.problem, None);
}

#[test]
fn a_session_the_cloud_dropped_signs_out_on_the_next_call() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let id = phone.store.device_id().clone();
    cloud.revoke(&id);
    phone.send(Command::PushToken("fcm".into()));
    let pushed = phone.wait("push", |e| match e {
        Event::Push { pushed, .. } => Some(*pushed),
        _ => None,
    });
    assert_eq!(pushed, None);
    let s = phone.wait_status("signed out", |s| s.sync == SyncState::SignedOut);
    assert_eq!(s.problem, Some(Problem::SignedOutRemotely));
}

#[test]
fn revoking_a_device_the_cloud_does_not_know_is_a_server_problem() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    phone.send(Command::RevokeDevice(kabar_protocol::DeviceId("ghost".into())));
    let devices = phone.wait("devices", |e| match e {
        Event::Devices(d) => Some(d.clone()),
        _ => None,
    });
    // The list still loads, with this device in it.
    assert!(devices.unwrap().iter().any(|d| d.current));
    let s = phone.wait_status("problem", |s| s.problem.is_some());
    assert_eq!(s.problem, Some(Problem::Server));
    assert_eq!(s.sync, SyncState::Live, "still syncing");
    // A sign-in the server fails on its side is named too.
    let mut other = Device::start(dir.path(), "other", Platform::Web, &cloud.url);
    other.send(Command::SignInWithIdToken("boom".into()));
    let s = other.wait_status("failed", |s| s.problem.is_some());
    assert_eq!(s.problem, Some(Problem::SignInFailed));
}

fn login_event(
    d: &mut Device,
    what: &str,
    mut f: impl FnMut(&super::LoginStatus) -> bool,
) -> super::LoginStatus {
    d.wait(what, |e| match e {
        Event::Login(l) if f(l) => Some(l.clone()),
        _ => None,
    })
}

#[test]
fn a_username_and_password_can_be_set_used_locked_and_removed() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let secret = |s: &str| super::Secret(s.into());

    phone.send(Command::LoadLogin);
    let l = login_event(&mut phone, "no login yet", |l| l.username.is_some());
    assert_eq!(l.username, Some(None));
    // A name outside a-z 0-9 . _ - never leaves the device.
    phone.send(Command::SetLogin { username: "مو".into(), password: secret("correct horse battery") });
    let l = login_event(&mut phone, "bad name", |l| l.problem.is_some());
    assert_eq!(l.problem, Some(Problem::BadUsername));
    assert_eq!(cloud.requested("POST /v1/login"), 0);
    phone.send(Command::SetLogin { username: "Mo.Kabar".into(), password: secret("correct horse battery") });
    let l = login_event(&mut phone, "saved", |l| l.saved);
    assert_eq!(l.username, Some(Some("mo.kabar".into())));

    // A device that remembers the session is not temporary.
    let mut mac = Device::start(dir.path(), "mac", Platform::Macos, &cloud.url);
    mac.send(Command::SignInPassword {
        username: "mo.kabar".into(),
        password: secret("correct horse battery"),
        remember: true,
    });
    let s = mac.wait_status("mac live", |s| s.sync == SyncState::Live);
    assert!(!s.temporary);

    // Five wrong passwords lock the name for a minute: the learner hears when to try again.
    let mut guesser = Device::start(dir.path(), "guesser", Platform::Web, &cloud.url);
    for _ in 0..kabar_server::login::LOCK_AFTER {
        guesser.send(Command::SignInPassword {
            username: "mo.kabar".into(),
            password: secret("wrong horse battery"),
            remember: false,
        });
        guesser.wait("checked", |e| matches!(e, Event::SigningIn(false)).then_some(()));
    }
    let s = guesser.wait_status("locked", |s| matches!(s.problem, Some(Problem::Locked(_))));
    assert_eq!(s.problem, Some(Problem::Locked(1)));
    assert_eq!(s.sync, SyncState::SignedOut);

    // Removing the login: the password no longer signs in.
    phone.send(Command::RemoveLogin);
    let l = login_event(&mut phone, "removed", |l| l.saved && l.username == Some(None));
    assert_eq!(l.problem, None);
    let mut late = Device::start(dir.path(), "late", Platform::Web, &cloud.url);
    late.send(Command::SignInPassword {
        username: "mo.kabar".into(),
        password: secret("correct horse battery"),
        remember: false,
    });
    let s = late.wait_status("refused", |s| s.problem.is_some());
    assert_eq!((s.sync, s.problem), (SyncState::SignedOut, Some(Problem::WrongPassword)));
}
