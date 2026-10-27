//! The app's sync engine against a local Kabar Cloud (workerd + fake Google).
//! Ignored by `cargo test`; `crates/server/tests/e2e.mjs` runs it with
//! `KABAR_E2E_URL` and `KABAR_E2E_TOKEN_URL` set. Nothing leaves the machine.

use std::time::{Duration, Instant};

use kabar_core::settings::{Settings, Theme};
use kabar_protocol::cloud::{DeviceInfo, Platform};
use kabar_protocol::sync::OpKind;
use tokio::sync::mpsc::UnboundedReceiver;

use super::session::MemoryVault;
use super::{CloudConfig, CloudHandle, Command, Event, Problem, SyncState};
use crate::store::Store;

struct Device {
    store: Store,
    handle: CloudHandle,
}

impl Device {
    fn start(dir: &std::path::Path, name: &str, platform: Platform, url: &str) -> Self {
        let path = dir.join(format!("{name}.db"));
        let store = Store::open(&path).unwrap();
        let info =
            DeviceInfo { device_id: store.device_id().clone(), name: name.into(), model: None, platform };
        let config = CloudConfig { base_url: url.into(), google: None };
        let handle = super::start(config, path, info, Box::new(MemoryVault::default())).unwrap();
        Self { store, handle }
    }

    fn send(&self, c: Command) {
        self.handle.commands.send(c).unwrap();
    }

    /// Waits for the first event `f` accepts.
    fn wait<T>(&mut self, what: &str, f: impl FnMut(&Event) -> Option<T>) -> T {
        wait(&mut self.handle.events, what, f)
    }

    fn wait_live(&mut self) {
        self.wait("live", |e| matches!(e, Event::Status(s) if s.sync == SyncState::Live).then_some(()));
    }
}

fn wait<T>(rx: &mut UnboundedReceiver<Event>, what: &str, mut f: impl FnMut(&Event) -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "timed out waiting for {what}");
        match rx.try_recv() {
            Ok(event) => {
                if let Some(v) = f(&event) {
                    return v;
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) => panic!("engine stopped: {e}"),
        }
    }
}

fn id_token(token_url: &str) -> String {
    let mut res = ureq::get(token_url).call().unwrap();
    res.body_mut().read_to_string().unwrap()
}

#[test]
#[ignore = "needs a local Kabar Cloud: run crates/server/tests/e2e.mjs"]
fn phone_and_tablet_sync_through_kabar_cloud() {
    let (Ok(url), Ok(token_url)) = (std::env::var("KABAR_E2E_URL"), std::env::var("KABAR_E2E_TOKEN_URL"))
    else {
        panic!("set KABAR_E2E_URL and KABAR_E2E_TOKEN_URL");
    };
    let dir = tempfile::tempdir().unwrap();
    let mut phone = Device::start(dir.path(), "Galaxy S24 Ultra", Platform::Android, &url);
    let mut tablet = Device::start(dir.path(), "Galaxy Tab S11", Platform::Android, &url);

    // A setting changed before sign-in is queued and goes up later.
    let old = phone.store.settings().unwrap();
    phone.store.save_settings(&old, &Settings { theme: Theme::Dark, ..old.clone() }).unwrap();

    phone.send(Command::SignInWithIdToken(id_token(&token_url)));
    phone.wait_live();
    tablet.send(Command::SignInWithIdToken(id_token(&token_url)));
    // Catch-up is applied before the hub says caught_up.
    tablet.wait_live();
    assert_eq!(tablet.store.settings().unwrap().theme, Theme::Dark);

    // M2's acceptance check: a word saved on the phone reaches the tablet.
    let started = Instant::now();
    let payload = serde_json::json!({ "lexeme": "rumah", "lang": "id", "status": "learning" });
    phone.store.write_synced("word_status", "id:rumah", OpKind::Upsert, Some(payload)).unwrap();
    phone.send(Command::Kick);
    tablet.wait("word", |e| match e {
        Event::Changed(t) if t.contains("word_status") => Some(()),
        _ => None,
    });
    let took = started.elapsed();
    let rows = tablet.store.synced_rows("word_status").unwrap();
    assert_eq!(rows[0].1["lexeme"], "rumah");
    assert!(took < Duration::from_secs(5), "took {took:?}");
    eprintln!("app e2e: phone → tablet in {} ms", took.as_millis());
    phone.wait("outbox empty", |e| matches!(e, Event::Status(s) if s.pending == 0).then_some(()));

    // Devices screen, then remote sign-out of the tablet.
    phone.send(Command::LoadDevices);
    let tablet_id = tablet.store.device_id().clone();
    let devices = phone.wait("devices", |e| match e {
        Event::Devices(Ok(d)) => Some(d.clone()),
        _ => None,
    });
    assert!(devices.iter().any(|d| d.current && d.name == "Galaxy S24 Ultra"));
    assert!(devices.iter().any(|d| d.device_id == tablet_id && d.last_sync_ms.is_some()));

    // The AI and TTS proxies answer the app with its session. Neither has
    // a provider key in the local Worker, so both say so, from the server.
    let link = super::link::CloudLink::default();
    link.connect(phone.handle.commands.clone());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let call = kabar_protocol::ai::EngineCall {
        cache_key: kabar_protocol::ai::CacheKey {
            task: kabar_protocol::ai::Task::RoleplayTurn,
            input_hash: "e2e".into(),
            engine: kabar_protocol::ai::EngineId::WorkersAi,
            prompt_version: 1,
            profile_hash: String::new(),
        },
        system: "You are Mbak Gadis.".into(),
        user: "Halo, Mbak!".into(),
        image_sha256: None,
        is_repair: false,
        json: true,
        max_tokens: None,
        personal: false,
    };
    let reply = runtime.block_on(link.ai(call));
    match &reply {
        Err(kabar_protocol::ai::EngineError::Unavailable { reason }) => {
            assert!(!reason.contains("not signed in") && !reason.contains("isn't set up"), "{reason}")
        }
        other => panic!("expected the proxy's own answer, got {other:?}"),
    }
    let audio =
        runtime.block_on(link.tts(crate::audio::line(kabar_protocol::cloud::Voice::IdGadis, "Halo!", 0, 0)));
    assert!(
        matches!(&audio, Err(super::link::Unreachable::Server(m)) if m.contains("NotConfigured")),
        "{audio:?}"
    );

    // Vision: an image goes up once, named by its hash.
    let png = b"\x89PNG\r\n\x1a\nkabar-e2e".to_vec();
    let sha = runtime.block_on(link.put_blob(png)).unwrap();
    assert_eq!(sha.len(), 64);

    phone.send(Command::RevokeDevice(tablet_id));
    tablet.wait("signed out", |e| match e {
        Event::Status(s) if s.sync == SyncState::SignedOut => {
            assert_eq!(s.problem, Some(Problem::SignedOutRemotely));
            Some(())
        }
        _ => None,
    });

    phone.send(Command::SignOut);
    phone.wait("phone signed out", |e| {
        matches!(e, Event::Status(s) if s.sync == SyncState::SignedOut).then_some(())
    });
    phone.send(Command::Shutdown);
    tablet.send(Command::Shutdown);
}
