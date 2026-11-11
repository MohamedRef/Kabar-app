//! The whole life of an answer across the durable boundaries (engineering
//! audit F23): first launch on a file, grade, restart, restore from a
//! backup on a new phone (the database, not the install marker), and two
//! devices syncing both ways. Each step must land on the same review
//! state. Native-only steps (process death, TalkBack…) are in
//! docs/NATIVE_QA.md.

use kabar_core::srs::Scheduler;
use kabar_protocol::srs::{Rating, ReviewMode};
use kabar_protocol::sync::SeqOp;

use super::*;

const T0: u64 = 1_793_610_000_000;
const DAY: u64 = 86_400_000;

/// What the hub would pass on: `from`'s outbox, numbered from `seq`.
fn relay(from: &mut Store, seq: u64) -> Vec<SeqOp> {
    let ops = from.outbox(500).unwrap();
    from.remove_from_outbox(ops.iter().map(|o| o.id.as_str())).unwrap();
    ops.into_iter().enumerate().map(|(i, op)| SeqOp { seq: seq + i as u64, op }).collect()
}

fn state(s: &Store) -> std::collections::BTreeMap<String, kabar_core::srs::CardState> {
    Scheduler::default().replay_all(&s.review_log().unwrap())
}

#[test]
fn first_launch_grade_restart_restore_and_two_device_sync_agree() {
    let phone_dir = tempfile::tempdir().unwrap();
    let (db, marker) = (phone_dir.path().join("files/kabar.db"), phone_dir.path().join("no_backup/install"));
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();

    // First launch; three answers over three days.
    let graded = {
        let mut phone = Store::open_marked(&db, Some(&marker)).unwrap();
        for (i, (card, rating)) in [("rumah", Rating::Good), ("makan", Rating::Again), ("pagi", Rating::Easy)]
            .into_iter()
            .enumerate()
        {
            phone.record_review(card, rating, T0 + i as u64 * DAY, 4000, ReviewMode::Classic).unwrap();
        }
        let s = state(&phone);
        assert_eq!(s.len(), 3);
        s
    };

    // Restart: the same file, opened again.
    let mut phone = Store::open_marked(&db, Some(&marker)).unwrap();
    assert_eq!(state(&phone), graded, "a restart reads back the same state");

    // Restore: a backup of the database on a new phone takes its own id.
    let new_dir = tempfile::tempdir().unwrap();
    let copy = new_dir.path().join("files/kabar.db");
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::copy(&db, &copy).unwrap();
    let restored = Store::open_marked(&copy, Some(&new_dir.path().join("no_backup/install"))).unwrap();
    assert_ne!(restored.device_id(), phone.device_id());
    assert_eq!(state(&restored), graded, "a restore lands on the same state");

    // Two devices: the tablet gets the phone's answers through the hub…
    let mut tablet = Store::open_in_memory().unwrap();
    let ops = relay(&mut phone, 1);
    let last = ops.len() as u64;
    tablet.apply_remote(&ops, last).unwrap();
    assert_eq!(state(&tablet), graded);
    // …answers once itself, and the phone gets that back.
    tablet.record_review("makan", Rating::Good, T0 + 5 * DAY, 3000, ReviewMode::Classic).unwrap();
    let back = relay(&mut tablet, last + 1);
    phone.apply_remote(&back, last + back.len() as u64).unwrap();
    assert_eq!(state(&phone), state(&tablet), "both devices agree after syncing both ways");
    assert_ne!(state(&phone), graded, "the tablet's answer moved the card");
}
