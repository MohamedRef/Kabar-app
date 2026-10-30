//! Existing behavior regressions for the feature facade.

use kabar_core::coach::history;
use kabar_core::settings::Theme;
use kabar_protocol::coach::{LeagueResult, PlanItem, PlanItemKind, PlanSize};
use kabar_protocol::learn::TaughtEntry;

use super::*;

fn day(y: i32, m: u32, d: u32) -> LocalDate {
    LocalDate::from_ymd(y, m, d).unwrap()
}

#[test]
fn device_id_and_settings_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kabar.db");

    let mut store = Store::open(&path).unwrap();
    let device = store.device_id().clone();
    let old = store.settings().unwrap();
    let new = Settings { theme: Theme::Dark, welcomed: true, ..old.clone() };
    store.save_settings(&old, &new).unwrap();
    drop(store);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.device_id(), &device);
    assert_eq!(store.settings().unwrap(), new);
}

#[test]
fn pen_lab_runs_newest_first() {
    let mut store = Store::open_in_memory().unwrap();
    let a = store.save_pen_lab_run(&PenLabReport { device_model: Some("a".into()), ..Default::default() });
    let b = store.save_pen_lab_run(&PenLabReport { device_model: Some("b".into()), ..Default::default() });
    assert!(b.unwrap().hlc > a.unwrap().hlc);
    let runs = store.pen_lab_runs().unwrap();
    assert_eq!(runs[0].report.device_model.as_deref(), Some("b"));
    assert_eq!(runs.len(), 2);
}

#[test]
fn coach_profile_round_trips_and_overwrites() {
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(store.coach_profile().unwrap(), None);
    let mut p = CoachProfile { name: "Mohamed".into(), ..Default::default() };
    store.save_coach_profile(&p).unwrap();
    p.utc_offset_min = 420;
    store.save_coach_profile(&p).unwrap();
    assert_eq!(store.coach_profile().unwrap(), Some(p));
}

#[test]
fn plans_are_keyed_by_date() {
    let mut store = Store::open_in_memory().unwrap();
    let today = day(2026, 10, 26);
    let plan = DailyPlan {
        date: today,
        size: PlanSize::Quick,
        comeback: false,
        items: vec![PlanItem { id: "reading".into(), kind: PlanItemKind::Reading, minutes: 5 }],
    };
    store.save_plan(&plan).unwrap();
    assert_eq!(store.plan_for(today).unwrap(), Some(plan));
    assert_eq!(store.plan_for(today.add_days(1)).unwrap(), None);
}

#[test]
fn activity_is_appended_filtered_and_retractable() {
    let mut store = Store::open_in_memory().unwrap();
    let today = day(2026, 10, 26);
    let a =
        store.add_activity(ActivityKind::Reviews { cards: 10, correct: 8 }, today.add_days(-2), 300).unwrap();
    let words = ActivityKind::NewWords { words: vec!["membaca".into(), "buku".into()] };
    let b = store.add_activity(words, today, 120).unwrap();
    let c = store.add_activity(ActivityKind::PlanItemDone { item_id: "review".into() }, today, 300).unwrap();
    assert!(c.hlc > b.hlc && b.hlc > a.hlc);
    assert_eq!(store.activity(None).unwrap().len(), 3);
    let recent = store.activity(Some(today)).unwrap();
    assert_eq!(recent, vec![b.clone(), c.clone()]);
    assert_eq!(history::day(&recent, today).minutes(), 7);

    store.retract_activity(&c.id).unwrap();
    assert_eq!(store.activity(Some(today)).unwrap(), vec![b]);
}

#[test]
fn reminders_and_reports() {
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(store.reminder_state().unwrap(), ReminderState::default());
    let state = ReminderState { snoozed_until_ms: Some(42), ..Default::default() };
    store.save_reminder_state(&state).unwrap();
    assert_eq!(store.reminder_state().unwrap(), state);

    assert_eq!(store.latest_report_week().unwrap(), None);
    for (week, minutes) in [(day(2026, 10, 12), 90), (day(2026, 10, 19), 120)] {
        store
            .save_report(&WeeklyReport {
                week_start: week,
                minutes,
                days_studied: 4,
                new_words: 12,
                retention: Some(0.85),
                streak: 4,
                strongest: None,
                weakest: None,
                league: LeagueResult { xp: 300, rank: 1, of: 2 },
                next_focus: vec![],
            })
            .unwrap();
    }
    assert_eq!(store.latest_report_week().unwrap(), Some(day(2026, 10, 19)));
    assert_eq!(store.report_for(day(2026, 10, 12)).unwrap().unwrap().minutes, 90);
}

#[test]
fn clock_resumes_past_coach_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kabar.db");
    let mut store = Store::open(&path).unwrap();
    let e = store.add_activity(ActivityKind::Reviews { cards: 1, correct: 1 }, day(2026, 10, 26), 0).unwrap();
    // A row stamped far ahead of wall time, as after a skewed clock.
    let ahead = e.hlc.wall_ms + 3_600_000;
    store.conn.execute("UPDATE xp_events SET hlc_wall = ?1", [ahead as i64]).unwrap();
    drop(store);

    let mut store = Store::open(&path).unwrap();
    let next = store.add_activity(ActivityKind::Reviews { cards: 1, correct: 1 }, day(2026, 10, 26), 0);
    assert!(next.unwrap().hlc.wall_ms >= ahead);
}

#[test]
fn local_values_overwrite() {
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(store.local_value("batik").unwrap(), None);
    store.set_local_value("batik", "kawung").unwrap();
    store.set_local_value("batik", "parang").unwrap();
    assert_eq!(store.local_value("batik").unwrap().as_deref(), Some("parang"));
}

#[test]
fn completed_lessons_skip_other_kinds_and_tombstones() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.completed_lessons().unwrap().is_empty());
    let rows = [
        ("e1", "bipa1.u01.l01", "completed", 0),
        ("e2", "bipa1.u01.l01", "completed", 0),
        ("e3", "bipa1.u01.l02", "started", 0),
        ("e4", "bipa1.u01.l03", "completed", 1),
        ("e5", "bipa1.test", "completed", 0),
    ];
    for (id, lesson, kind, deleted) in rows {
        store
            .conn
            .execute(
                "INSERT INTO lesson_events (id, lesson_id, kind, payload, device_id, hlc_wall, hlc_counter, deleted)
                 VALUES (?1, ?2, ?3, '{}', 'd', 1, 0, ?4)",
                params![id, lesson, kind, deleted],
            )
            .unwrap();
    }
    let done = store.completed_lessons().unwrap();
    assert_eq!(done.into_iter().collect::<Vec<_>>(), ["bipa1.test", "bipa1.u01.l01"]);
}

#[test]
fn passing_a_lesson_approves_it() {
    use kabar_protocol::lesson::{ExerciseResult, LessonCompleted, Outcome};
    let mut store = Store::open_in_memory().unwrap();
    let done = |score, missed: usize| LessonCompleted {
        score,
        xp: 0,
        duration_ms: 0,
        mistakes: vec![],
        results: (0..5)
            .map(|i| ExerciseResult {
                id: format!("e{i}"),
                first: if i < missed { Outcome::Missed } else { Outcome::Correct },
                retry: None,
                mistakes: vec![],
            })
            .collect(),
        pass_verifies: true,
    };
    store.record_lesson_event("j1.u01.l01", "completed", &done(100, 0)).unwrap();
    store.record_lesson_event("j1.u01.l02", "completed", &done(40, 3)).unwrap();
    assert_eq!(store.approved_lessons().unwrap().into_iter().collect::<Vec<_>>(), ["j1.u01.l01"]);
    // A later, better try approves it too.
    store.record_lesson_event("j1.u01.l02", "completed", &done(80, 1)).unwrap();
    assert_eq!(store.approved_lessons().unwrap().len(), 2);
}

/// Since 2026-10-29 a pass is progress, not a content check: only the
/// learner's tap checks a lesson, and taking it back restores the flag
/// on the lesson and its cards. Old passes keep counting.
#[test]
fn checking_content_is_separate_from_passing() {
    use kabar_protocol::lesson::{ExerciseResult, LessonCompleted, Outcome};
    let lesson =
        kabar_core::lesson::parse_lesson(include_str!("../../../../fixtures/lessons/bipa1.u01.l01.yaml"))
            .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let pass = |pass_verifies| LessonCompleted {
        score: 100,
        xp: 0,
        duration_ms: 0,
        mistakes: vec![],
        results: vec![ExerciseResult {
            id: "e01".into(),
            first: Outcome::Correct,
            retry: None,
            mistakes: vec![],
        }],
        pass_verifies,
    };
    store.record_lesson_event(&lesson.id, "completed", &pass(false)).unwrap();
    store.add_lesson_cards(&lesson, false).unwrap();
    assert!(store.verified_lessons().unwrap().is_empty(), "a new pass checks nothing");

    store.verify_lesson(&lesson.id).unwrap();
    store.verify_lesson(&lesson.id).unwrap();
    assert_eq!(store.verified_lessons().unwrap().get(&lesson.id), Some(&Verification::Checked));
    assert_eq!(store.lesson_events(&lesson.id).unwrap().iter().filter(|e| e.kind == "verified").count(), 1);
    let mine = |s: &Store| -> Vec<bool> {
        s.cards()
            .unwrap()
            .into_iter()
            .filter(|c| c.source_ref.as_deref() == Some(lesson.id.as_str()))
            .map(|c| c.verified)
            .collect()
    };
    assert!(!mine(&store).is_empty() && mine(&store).iter().all(|v| *v));

    store.unverify_lesson(&lesson.id).unwrap();
    assert!(store.verified_lessons().unwrap().is_empty());
    assert!(mine(&store).iter().all(|v| !*v));
    // A correction, not a deletion: the log keeps both events.
    let kinds: Vec<String> = store.lesson_events(&lesson.id).unwrap().into_iter().map(|e| e.kind).collect();
    assert_eq!(kinds, ["completed", "verified", "unverified"]);
    store.unverify_lesson(&lesson.id).unwrap();
    store.verify_lesson(&lesson.id).unwrap();
    assert_eq!(store.verified_lessons().unwrap().get(&lesson.id), Some(&Verification::Checked));
    assert_eq!(store.lesson_events(&lesson.id).unwrap().len(), 4, "no duplicate events");
    store.unverify_lesson(&lesson.id).unwrap();

    // A pass from before the split still counts, and says so.
    store
        .record_lesson_event("j1.u01.l01", "completed", &LessonCompleted { score: 85, ..pass(true) })
        .unwrap();
    assert_eq!(
        store.verified_lessons().unwrap().get("j1.u01.l01"),
        Some(&Verification::PassedEarlier { score: 85 })
    );
}

#[test]
fn lesson_events_are_appended_in_order() {
    use kabar_protocol::lesson::LessonCompleted;
    let mut store = Store::open_in_memory().unwrap();
    let done = LessonCompleted {
        score: 81,
        xp: 19,
        duration_ms: 420_000,
        mistakes: vec!["sore".into()],
        results: vec![],
        pass_verifies: false,
    };
    let a = store.record_lesson_event("bipa1.u01.l01", "completed", &done).unwrap();
    let b = store.record_lesson_event("bipa1.u01.l01", "completed", &done).unwrap();
    store.record_lesson_event("bipa1.u01.l02", "completed", &done).unwrap();
    assert!(b.hlc > a.hlc);
    let events = store.lesson_events("bipa1.u01.l01").unwrap();
    assert_eq!(events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), [a.id.as_str(), b.id.as_str()]);
    let back: LessonCompleted = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(back, done);
}

#[test]
fn the_newest_placement_wins() {
    use kabar_protocol::test::PlacementResult;
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(store.placement().unwrap(), None);
    let placed =
        |level: &str| PlacementResult { start_level: level.into(), start_unit: None, sections: vec![] };
    store.record_lesson_event("placement", "placed", &placed("bipa2")).unwrap();
    store.record_lesson_event("placement", "placed", &placed("bipa3")).unwrap();
    assert_eq!(store.placement().unwrap(), Some(placed("bipa3")));
    // Placement never marks lessons done.
    assert!(store.completed_lessons().unwrap().is_empty());
}

#[test]
fn position_is_last_writer_wins_and_survives_reopen() {
    use kabar_protocol::lesson::LessonStep;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kabar.db");
    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.position().unwrap(), None);
    let at = |id: &str| LessonPosition {
        lesson_id: "bipa1.u01.l02".into(),
        step: LessonStep::Exercise { id: id.into(), retry: false },
        progress: None,
        response: None,
    };
    store.save_position(&at("e03")).unwrap();
    store.save_position(&at("e04")).unwrap();
    drop(store);

    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.position().unwrap(), Some(at("e04")));
    let rows: i64 = store.conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 1, "one row per device");
    store.clear_position().unwrap();
    assert_eq!(store.position().unwrap(), None);
}

/// What the hub would send: the ops in `from`'s outbox, numbered.
fn relay(from: &Store, first_seq: u64) -> Vec<SeqOp> {
    from.outbox(100)
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, op)| SeqOp { seq: first_seq + i as u64, op })
        .collect()
}

fn word(status: &str) -> Option<Value> {
    Some(serde_json::json!({ "lexeme": "rumah", "lang": "id", "status": status }))
}

#[test]
fn a_word_saved_on_the_phone_reaches_the_tablet() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut tablet = Store::open_in_memory().unwrap();
    phone.write_synced("word_status", "id:rumah", OpKind::Upsert, word("learning")).unwrap();
    let ops = relay(&phone, 1);
    assert_eq!(ops.len(), 1);

    let changed = tablet.apply_remote(&ops, 1).unwrap();
    assert!(changed.contains("word_status"));
    assert_eq!(tablet.synced_rows("word_status").unwrap()[0].1["status"], "learning");
    assert_eq!(tablet.last_seq().unwrap(), 1);
    // Remote ops never go back out.
    assert_eq!(tablet.outbox_len().unwrap(), 0);
    // Replays change nothing.
    assert!(tablet.apply_remote(&ops, 1).unwrap().is_empty());
}

#[test]
fn a_local_edit_beats_the_remote_row_it_replaces() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut tablet = Store::open_in_memory().unwrap();
    // The phone's clock runs two minutes ahead.
    let mut op = phone.write_synced("word_status", "id:rumah", OpKind::Upsert, word("learning")).unwrap();
    op.hlc.wall_ms += 120_000;
    tablet.apply_remote(&[SeqOp { seq: 1, op: op.clone() }], 1).unwrap();
    let edit = tablet.write_synced("word_status", "id:rumah", OpKind::Upsert, word("known")).unwrap();
    assert!(edit.hlc > op.hlc);
    // And the phone, receiving the older op later, keeps the newer one.
    let mut late = op.clone();
    late.hlc.wall_ms -= 1;
    late.id = "late".into();
    assert!(tablet.apply_remote(&[SeqOp { seq: 2, op: late }], 2).unwrap().is_empty());
    assert_eq!(tablet.synced_rows("word_status").unwrap()[0].1["status"], "known");
}

#[test]
fn remote_settings_update_the_settings_table() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut tablet = Store::open_in_memory().unwrap();
    let old = phone.settings().unwrap();
    let new = Settings { theme: Theme::Dark, ..old.clone() };
    phone.save_settings(&old, &new).unwrap();
    tablet.apply_remote(&relay(&phone, 1), 1).unwrap();
    assert_eq!(tablet.settings().unwrap().theme, Theme::Dark);
}

#[test]
fn acked_ops_leave_the_outbox_in_order() {
    let mut store = Store::open_in_memory().unwrap();
    let a = store.write_synced("word_status", "id:a", OpKind::Upsert, word("new")).unwrap();
    let b = store.write_synced("word_status", "id:b", OpKind::Upsert, word("new")).unwrap();
    let ids: Vec<String> = store.outbox(10).unwrap().into_iter().map(|o| o.id).collect();
    assert_eq!(ids, [a.id.clone(), b.id.clone()]);
    store.remove_from_outbox([a.id.as_str()]).unwrap();
    assert_eq!(store.outbox(10).unwrap()[0].id, b.id);
}

#[test]
fn invalid_writes_are_refused() {
    let mut store = Store::open_in_memory().unwrap();
    let err = store.write_synced("nope", "x", OpKind::Upsert, word("new")).unwrap_err();
    assert!(matches!(err, StoreError::Invalid(RejectReason::UnknownTable)));
    assert_eq!(store.outbox_len().unwrap(), 0);
}

#[test]
fn unknown_tables_from_newer_builds_are_kept() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut tablet = Store::open_in_memory().unwrap();
    let payload = Some(serde_json::json!({ "title": "Sek tak lapor pusat" }));
    phone.write_synced("texts", "t1", OpKind::Upsert, payload).unwrap();
    tablet.apply_remote(&relay(&phone, 1), 1).unwrap();
    assert_eq!(tablet.synced_rows("texts").unwrap()[0].0, "t1");
}

#[test]
fn settings_from_before_sync_are_queued() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kabar.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute_batch(MIGRATIONS[0]).unwrap();
        tx.execute(
            "INSERT INTO settings (key, value, device_id, hlc_wall, hlc_counter) VALUES ('theme', '\"dark\"', 'mac', 5, 0)",
            [],
        )
        .unwrap();
        tx.pragma_update(None, "user_version", 1).unwrap();
        tx.commit().unwrap();
    }
    let store = Store::open(&path).unwrap();
    let ops = store.outbox(10).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].payload, Some(serde_json::json!({ "value": "dark" })));
    assert_eq!(ops[0].hlc, Hlc { wall_ms: 5, counter: 0, device: DeviceId("mac".into()) });
    assert_eq!(store.settings().unwrap().theme, Theme::Dark);
}

#[test]
fn coach_rows_sync_and_retractions_follow() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut tablet = Store::open_in_memory().unwrap();
    let profile = CoachProfile { name: "Mohamed".into(), utc_offset_min: 120, ..Default::default() };
    phone.save_coach_profile(&profile).unwrap();
    let today = day(2026, 10, 26);
    let e = phone.add_activity(ActivityKind::PlanItemDone { item_id: "review".into() }, today, 300).unwrap();
    tablet.apply_remote(&relay(&phone, 1), 2).unwrap();
    assert_eq!(tablet.coach_profile().unwrap(), Some(profile));
    assert_eq!(tablet.activity(Some(today)).unwrap(), vec![e.clone()]);

    // Unticked on the tablet: gone on the phone too.
    let before = tablet.outbox_len().unwrap();
    tablet.retract_activity(&e.id).unwrap();
    assert_eq!(tablet.outbox_len().unwrap(), before + 1);
    let undo = tablet.outbox(10).unwrap().pop().unwrap();
    phone.apply_remote(&[SeqOp { seq: 3, op: undo }], 3).unwrap();
    assert!(phone.activity(Some(today)).unwrap().is_empty());
}

#[test]
fn coach_rows_from_before_sync_are_queued() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kabar.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        for (i, sql) in MIGRATIONS[..SYNC_SCHEMA as usize - 1].iter().enumerate() {
            let tx = conn.transaction().unwrap();
            tx.execute_batch(sql).unwrap();
            tx.pragma_update(None, "user_version", i as u32 + 1).unwrap();
            tx.commit().unwrap();
        }
        let event = ActivityEvent {
            id: "e1".into(),
            hlc: Hlc { wall_ms: 7, counter: 0, device: DeviceId("mac".into()) },
            date: day(2026, 10, 25),
            at_ms: 7,
            kind: ActivityKind::Reviews { cards: 3, correct: 2 },
            seconds: 60,
        };
        conn.execute(
            "INSERT INTO xp_events (id, date, event, device_id, hlc_wall, hlc_counter, deleted)
             VALUES ('e1', '2026-10-25', ?1, 'mac', 9, 0, 1)",
            [serde_json::to_string(&event).unwrap()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO goals (id, value, device_id, hlc_wall, hlc_counter) VALUES ('profile', '{\"name\":\"M\"}', 'mac', 8, 0)",
            [],
        )
        .unwrap();
    }
    let store = Store::open(&path).unwrap();
    let kinds: Vec<(String, OpKind)> =
        store.outbox(10).unwrap().into_iter().map(|o| (o.table, o.kind)).collect();
    assert_eq!(
        kinds,
        [
            ("goals".to_string(), OpKind::Upsert),
            ("xp_events".to_string(), OpKind::Append),
            ("xp_events".to_string(), OpKind::Delete)
        ]
    );
}

#[test]
fn learning_rows_sync_and_undo_follows() {
    use kabar_core::review::seed_bridge_deck;
    use kabar_protocol::srs::{Rating, ReviewMode};
    let mut phone = Store::open_in_memory().unwrap();
    let mut tablet = Store::open_in_memory().unwrap();
    let deck = seed_bridge_deck();
    phone.upsert_cards(&deck).unwrap();
    let kept = phone.record_review("kabar", Rating::Good, now_ms(), 900, ReviewMode::Classic).unwrap();
    let undone = phone.record_review("kabar", Rating::Again, now_ms(), 900, ReviewMode::Classic).unwrap();
    assert!(phone.undo_review(&undone.id).unwrap());
    assert!(!phone.undo_review(&undone.id).unwrap(), "undoing twice queues nothing");
    let entry: TaughtEntry =
        serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
    phone.save_taught(&entry).unwrap();
    phone.record_lesson_event("bipa1.u01.l01", "completed", &serde_json::json!({ "score": 81 })).unwrap();

    let ops = relay(&phone, 1);
    let changed = tablet.apply_remote(&ops, ops.len() as u64).unwrap();
    for table in ["cards", "review_log", "taught_entries", "lesson_events"] {
        assert!(changed.contains(table), "{table}");
    }
    assert_eq!(tablet.cards().unwrap().len(), deck.len());
    assert_eq!(tablet.review_log().unwrap(), [kept]);
    assert_eq!(tablet.taught_entries().unwrap(), std::slice::from_ref(&entry));
    assert_eq!(tablet.completed_lessons().unwrap().len(), 1);
    assert_eq!(tablet.outbox_len().unwrap(), 0);

    // Forgotten on the tablet: gone on the phone too.
    tablet.delete_taught(&entry.row_id()).unwrap();
    let forget = tablet.outbox(10).unwrap().pop().unwrap();
    phone.apply_remote(&[SeqOp { seq: 99, op: forget }], 99).unwrap();
    assert!(phone.taught_entries().unwrap().is_empty());
}

#[test]
fn another_devices_review_is_stored_but_never_pushed() {
    use kabar_protocol::srs::{Rating, ReviewEvent, ReviewMode};
    let mut store = Store::open_in_memory().unwrap();
    let remote = ReviewEvent {
        id: "remote-1".into(),
        card_id: "kabar".into(),
        hlc: Hlc { wall_ms: 500, counter: 0, device: DeviceId("tab-s11".into()) },
        at_ms: 500,
        rating: Rating::Hard,
        duration_ms: 3000,
        mode: ReviewMode::CardWall,
        evidence: Default::default(),
    };
    store.append_review(&remote).unwrap();
    assert_eq!(store.review_log().unwrap(), [remote]);
    assert_eq!(store.outbox_len().unwrap(), 0);
    assert!(!store.undo_review("remote-1").unwrap());
}

#[test]
fn learning_rows_from_before_sync_are_queued() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kabar.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        for (i, sql) in MIGRATIONS[..SYNC_SCHEMA as usize - 1].iter().enumerate() {
            let tx = conn.transaction().unwrap();
            tx.execute_batch(sql).unwrap();
            tx.pragma_update(None, "user_version", i as u32 + 1).unwrap();
            tx.commit().unwrap();
        }
        conn.execute_batch(
            "INSERT INTO cards (id, payload, device_id, hlc_wall, hlc_counter) VALUES ('kabar', '{\"id\":\"kabar\"}', 'mac', 3, 0);
             INSERT INTO review_log (id, card_id, payload, device_id, hlc_wall, hlc_counter)
                VALUES ('r1', 'kabar', '{\"id\":\"r1\"}', 'mac', 4, 0);
             INSERT INTO lesson_events (id, lesson_id, kind, payload, device_id, hlc_wall, hlc_counter)
                VALUES ('l1', 'bipa1.u01.l01', 'completed', '{}', 'mac', 5, 0);",
        )
        .unwrap();
    }
    let store = Store::open(&path).unwrap();
    let tables: Vec<String> = store.outbox(10).unwrap().into_iter().map(|o| o.table).collect();
    for table in ["cards", "review_log", "lesson_events"] {
        assert!(tables.iter().any(|t| t == table), "{table} in {tables:?}");
    }
}

#[test]
fn ai_answers_are_cached_locally() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.ai_cache_get("roleplay_turn:workers_ai:v1:abc").unwrap(), None);
    store.ai_cache_put("roleplay_turn:workers_ai:v1:abc", r#"{"reply":"Halo"}"#).unwrap();
    store.ai_cache_put("roleplay_turn:workers_ai:v1:abc", r#"{"reply":"Halo!"}"#).unwrap();
    assert_eq!(
        store.ai_cache_get("roleplay_turn:workers_ai:v1:abc").unwrap().as_deref(),
        Some(r#"{"reply":"Halo!"}"#)
    );
    store.ai_cache_remove("roleplay_turn:workers_ai:v1:abc").unwrap();
    assert_eq!(store.ai_cache_get("roleplay_turn:workers_ai:v1:abc").unwrap(), None);
    assert_eq!(store.outbox_len().unwrap(), 0, "never synced");
}

#[test]
fn refuses_a_newer_schema() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    assert!(matches!(migrate(&mut conn), Err(StoreError::TooNew { found: 99, .. })));
}
