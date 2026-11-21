//! Sign-in and sign-out edges from the accounts review (#715): a token the
//! Cloud calls expired, an account that left the allowlist, and a sign-in on
//! someone else's device that must leave nothing behind.

use std::sync::atomic::Ordering;

use kabar_protocol::cloud::Platform;
use kabar_protocol::reader::WordStatus;
use kabar_protocol::sync::OpKind;

use super::e2e::Device;
use super::fake::FakeCloud;
use super::{Command, Event, Problem, Secret, SyncState};

const EMAIL: &str = "learner@example.com";

fn signed_in(dir: &std::path::Path, cloud: &FakeCloud, name: &str) -> Device {
    let mut d = Device::start(dir, name, Platform::Android, &cloud.url);
    d.send(Command::SignInWithIdToken(FakeCloud::id_token(EMAIL)));
    d.wait_live();
    d
}

/// Waits until `f` holds (state the engine changes on its own thread).
fn eventually(what: &str, mut f: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !f() {
        assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Still signed in: the devices list has this device (signing out empties it).
fn still_signed_in(d: &mut Device) {
    d.send(Command::LoadDevices);
    assert!(devices(d).unwrap().iter().any(|x| x.current), "still signed in");
}

fn devices(d: &mut Device) -> Result<Vec<kabar_protocol::cloud::DeviceSummary>, Problem> {
    d.wait("devices", |e| match e {
        Event::Devices(list) => Some(list.clone()),
        _ => None,
    })
}

#[test]
fn a_token_the_cloud_calls_expired_refreshes_instead_of_signing_out() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    let mut tablet = signed_in(dir.path(), &cloud, "tablet");
    let tablet_id = tablet.store.device_id().clone();
    let before = cloud.requested("POST /v1/auth/refresh");

    // The phone's clock runs behind: every token it holds is already
    // expired as far as Kabar Cloud is concerned, its refresh token isn't.
    cloud.forget_access_tokens();
    phone.send(Command::LoadDevices);
    assert!(devices(&mut phone).unwrap().iter().any(|d| d.current));

    cloud.forget_access_tokens();
    phone.send(Command::LoadUsage);
    let usage = phone.wait("usage", |e| match e {
        Event::Usage(u) => Some(u.clone()),
        _ => None,
    });
    assert!(usage.is_ok());

    cloud.forget_access_tokens();
    phone.send(Command::PushToken("fcm".into()));
    let pushed = phone.wait("push", |e| match e {
        Event::Push { pushed, .. } => Some(*pushed),
        _ => None,
    });
    assert_eq!(pushed, Some(true));

    cloud.forget_access_tokens();
    phone.send(Command::RevokeDevice(tablet_id));
    let list = devices(&mut phone).unwrap();
    assert!(list.iter().any(|d| d.revoked), "the tablet was signed out");
    tablet.wait_status("tablet signed out", |s| s.sync == SyncState::SignedOut);

    assert!(cloud.requested("POST /v1/auth/refresh") >= before + 4, "each call refreshed once");
    still_signed_in(&mut phone);
}

#[test]
fn an_account_off_the_allowlist_is_told_so_when_it_signs_out() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    cloud.state.knobs.disallowed.store(true, Ordering::SeqCst);
    cloud.forget_access_tokens();
    phone.send(Command::LoadDevices);
    let s = phone.wait_status("signed out", |s| s.sync == SyncState::SignedOut);
    assert_eq!(s.problem, Some(Problem::NotAllowed), "not “signed out from another device”");
}

#[test]
fn signing_out_of_someone_elses_device_wipes_what_the_account_brought() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    phone.send(Command::SetLogin {
        username: "mo.kabar".into(),
        password: Secret("correct horse battery".into()),
    });
    phone.wait("login saved", |e| matches!(e, Event::Login(l) if l.saved).then_some(()));
    let payload = serde_json::json!({ "lexeme": "rumah", "lang": "id", "status": "known" });
    phone.store.write_synced("word_status", "id:rumah", OpKind::Upsert, Some(payload)).unwrap();
    phone.send(Command::Kick);
    phone.wait_status("phone pushed", |s| s.pending == 0 && s.sync == SyncState::Live);

    // A library computer: "This isn't my device".
    let mut library = Device::start(dir.path(), "library", Platform::Web, &cloud.url);
    let device = library.store.device_id().clone();
    library.send(Command::SignInPassword {
        username: "mo.kabar".into(),
        password: Secret("correct horse battery".into()),
        remember: false,
    });
    let s = library.wait_status("library live", |s| s.sync == SyncState::Live);
    assert!(s.temporary);
    let known = |d: &Device| d.store.word_statuses(["id:rumah"]).unwrap()["id:rumah"] == WordStatus::Known;
    eventually("the word arrived", || known(&library));
    assert!(library.store.is_temporary().unwrap());

    library.send(Command::SignOut);
    library.wait("wiped", |e| matches!(e, Event::Changed(t) if t.contains("word_status")).then_some(()));
    library.wait_status("signed out", |s| s.sync == SyncState::SignedOut);
    assert_eq!(library.store.word_statuses(["id:rumah"]).unwrap()["id:rumah"], WordStatus::default());
    assert_eq!((library.store.last_seq().unwrap(), library.store.outbox_len().unwrap()), (0, 0));
    assert!(!library.store.is_temporary().unwrap());
    assert_eq!(library.store.device_id(), &device, "still the same install");
    assert_eq!(cloud.requested("POST /v1/auth/signout"), 1);

    // The learner's own phone keeps everything.
    assert_eq!(phone.store.word_statuses(["id:rumah"]).unwrap()["id:rumah"], WordStatus::Known);
}

#[test]
fn cancelling_with_no_sign_in_waiting_changes_nothing() {
    let cloud = FakeCloud::start();
    let dir = tempfile::tempdir().unwrap();
    let mut phone = signed_in(dir.path(), &cloud, "phone");
    phone.send(Command::CancelSignIn);
    still_signed_in(&mut phone);
}
