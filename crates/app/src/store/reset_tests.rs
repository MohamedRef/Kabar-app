//! «ابدأ من الأول» over real stores: the marker syncs, an offline device's
//! older answers stay out, undo is exact, and nothing that is not progress
//! changes.

use kabar_core::reset::Scope;
use kabar_core::srs::Scheduler;
use kabar_protocol::lesson::{LessonPosition, LessonStep};
use kabar_protocol::srs::{Rating, ReviewMode};
use kabar_protocol::sync::SeqOp;

use super::*;

const BIPA1: &str = "lesson.bipa1.u01.l01.pagi.recognition";
const BIPA1_B: &str = "lesson.bipa1.u01.l02.rumah.recognition";
const BIPA2: &str = "lesson.bipa2.u01.l01.kantor.recognition";
const SEED: &str = "seed.bridge.kabar.meaning";

fn relay(from: &mut Store, to: &mut Store, seq: u64) -> u64 {
    let ops = from.outbox(500).unwrap();
    from.remove_from_outbox(ops.iter().map(|o| o.id.as_str())).unwrap();
    let n = ops.len() as u64;
    let ops: Vec<SeqOp> =
        ops.into_iter().enumerate().map(|(i, op)| SeqOp { seq: seq + i as u64, op }).collect();
    to.apply_remote(&ops, seq + n).unwrap();
    seq + n
}

fn answer(s: &mut Store, card: &str) {
    s.record_review(card, Rating::Good, now_ms(), 2000, ReviewMode::Classic).unwrap();
}

fn state(s: &Store) -> std::collections::BTreeMap<String, kabar_core::srs::CardState> {
    Scheduler::default().replay_all(&s.progress_log().unwrap())
}

fn level(l: &str) -> Scope {
    Scope::Level(l.into())
}

fn later() {
    // Order is by HLC: what follows lands at least a millisecond later.
    std::thread::sleep(std::time::Duration::from_millis(3));
}

#[test]
fn an_offline_tablets_older_answers_stay_out_and_its_newer_ones_count() {
    let (mut phone, mut tablet) = (Store::open_in_memory().unwrap(), Store::open_in_memory().unwrap());
    answer(&mut phone, BIPA1);
    answer(&mut phone, BIPA2);
    let seq = relay(&mut phone, &mut tablet, 1);
    answer(&mut tablet, BIPA1_B);
    later();
    phone.put_reset(&level("bipa1")).unwrap();
    let seq = relay(&mut phone, &mut tablet, seq);
    relay(&mut tablet, &mut phone, seq);
    for s in [&phone, &tablet] {
        let st = state(s);
        assert!(!st.contains_key(BIPA1) && !st.contains_key(BIPA1_B), "BIPA 1 is new on both");
        assert!(st.contains_key(BIPA2), "BIPA 2 is untouched");
        assert_eq!(s.review_log().unwrap().len(), 3, "nothing was deleted");
    }
    answer(&mut tablet, BIPA1_B);
    assert!(state(&tablet).contains_key(BIPA1_B), "an answer after the reset counts");
}

#[test]
fn a_reset_outranks_answers_from_a_device_whose_clock_runs_ahead() {
    // #687: the phone's UI connection writes the marker, but only its sync
    // engine's connection had seen the tablet's stamps. The tablet's clock
    // runs four minutes ahead (the hub takes up to five), so its earlier
    // answer sorted after the cutoff and still counted.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kabar.db");
    let (mut engine, mut ui) = (Store::open(&path).unwrap(), Store::open(&path).unwrap());
    let mut tablet = Store::open_in_memory().unwrap();
    answer(&mut tablet, BIPA1);
    let ops: Vec<SeqOp> = tablet
        .outbox(500)
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, mut op)| {
            op.hlc.wall_ms += 4 * 60_000;
            SeqOp { seq: 1 + i as u64, op }
        })
        .collect();
    engine.apply_remote(&ops, ops.len() as u64).unwrap();
    assert!(state(&ui).contains_key(BIPA1));
    later();
    let cutoff = ui.put_reset(&level("bipa1")).unwrap();
    assert!(cutoff.wall_ms >= ops[0].op.hlc.wall_ms, "{cutoff:?}");
    assert!(!state(&ui).contains_key(BIPA1), "the tablet's earlier answer is reset too");
    assert!(!state(&engine).contains_key(BIPA1));
    // An answer after the reset still counts.
    answer(&mut ui, BIPA1);
    assert!(state(&ui).contains_key(BIPA1));
}

#[test]
fn the_whole_course_restarts_every_card_and_undo_is_exact() {
    let mut s = Store::open_in_memory().unwrap();
    for card in [BIPA1, BIPA2, SEED, BIPA1] {
        answer(&mut s, card);
    }
    let before = state(&s);
    later();
    s.put_reset(&level("bipa2")).unwrap();
    assert_eq!(state(&s).len(), 2, "a level leaves the seed card alone");
    s.put_reset(&Scope::All).unwrap();
    assert!(state(&s).is_empty());
    s.remove_reset(&Scope::All).unwrap();
    s.remove_reset(&level("bipa2")).unwrap();
    assert_eq!(state(&s), before);
    assert!(s.resets().unwrap().is_empty());
}

#[test]
fn finished_lessons_and_lanjut_follow_the_marker() {
    let mut s = Store::open_in_memory().unwrap();
    s.record_lesson_event("bipa1.u01.l01", "completed", &serde_json::json!({})).unwrap();
    s.record_lesson_event("bipa2.u01.l01", "completed", &serde_json::json!({})).unwrap();
    let pos = LessonPosition {
        lesson_id: "bipa1.u01.l02".into(),
        step: LessonStep::Exercise { id: "e01".into(), retry: false },
        progress: None,
        response: None,
    };
    s.save_position(&pos).unwrap();
    later();
    s.put_reset(&level("bipa1")).unwrap();
    assert_eq!(s.completed_lessons().unwrap(), BTreeSet::from(["bipa2.u01.l01".to_string()]));
    assert_eq!(s.position().unwrap(), None);
    // Finishing it again counts.
    s.record_lesson_event("bipa1.u01.l01", "completed", &serde_json::json!({})).unwrap();
    assert!(s.completed_lessons().unwrap().contains("bipa1.u01.l01"));
    s.remove_reset(&level("bipa1")).unwrap();
    assert_eq!(s.position().unwrap(), Some(pos));
}

#[test]
fn checks_saved_words_and_the_whole_log_are_untouched() {
    let mut s = Store::open_in_memory().unwrap();
    answer(&mut s, BIPA1);
    s.verify_lesson("bipa1.u01.l01").unwrap();
    s.save_word("pagi", "pagi", "Selamat pagi!", None).unwrap();
    let dump = |s: &Store| {
        let mut out = Vec::new();
        for t in ["lesson_events", "saved_words", "review_log", "xp_events", "word_status", "highlights"] {
            let mut stmt = s.conn.prepare(&format!("SELECT * FROM {t} ORDER BY 1")).unwrap();
            let n = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..n).map(|i| r.get::<_, rusqlite::types::Value>(i).map(|v| format!("{v:?}"))).collect()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<Vec<String>>>>()
                .unwrap();
            out.push(rows);
        }
        out
    };
    let before = dump(&s);
    let verified = s.verified_lessons().unwrap();
    s.put_reset(&Scope::All).unwrap();
    assert_eq!(dump(&s), before);
    assert_eq!(s.verified_lessons().unwrap(), verified);
}

#[test]
fn an_export_carries_the_marker() {
    let mut s = Store::open_in_memory().unwrap();
    s.put_reset(&level("j3")).unwrap();
    let doc = s.export("test", now_ms()).unwrap().document;
    assert!(doc.to_string().contains("reset.j3"));
}

#[test]
fn finishing_on_another_device_drops_older_lanjut_for_that_lesson_only() {
    let (mut phone, mut tablet) = (Store::open_in_memory().unwrap(), Store::open_in_memory().unwrap());
    let mid = LessonPosition {
        lesson_id: "bipa1.u01.l01".into(),
        step: LessonStep::Exercise { id: "e07".into(), retry: false },
        progress: None,
        response: None,
    };
    phone.save_position(&mid).unwrap();
    let seq = relay(&mut phone, &mut tablet, 1);
    assert_eq!(tablet.position().unwrap(), Some(mid.clone()));

    later();
    tablet.save_position(&mid).unwrap();
    tablet.record_lesson_event("bipa1.u01.l01", "completed", &serde_json::json!({})).unwrap();
    tablet.clear_position().unwrap();
    relay(&mut tablet, &mut phone, seq);

    assert_eq!(tablet.position().unwrap(), None, "finished on tablet: phone's older row no longer resumes");
    assert_eq!(phone.position().unwrap(), None, "synced back to phone: nothing left to resume");

    // Starting a new replay later saves a fresher stamp and resumes normally.
    later();
    let replay = LessonPosition { step: LessonStep::Exercise { id: "e02".into(), retry: false }, ..mid };
    phone.save_position(&replay).unwrap();
    assert_eq!(phone.position().unwrap(), Some(replay));
}
