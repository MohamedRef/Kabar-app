//! A demo database for previews and screenshots of Stats and the level
//! check (never shipped; test-only):
//!
//! ```sh
//! KABAR_DEMO_DB=/tmp/kabar-demo/kabar.db cargo test -p kabar-app seed_stats_demo -- --ignored
//! ```
//!
//! Then point the app's data dir at it (on Linux: `HOME` such that
//! `$HOME/.local/share/kabar/kabar.db` is that file) and open
//! `KABAR_OPEN_SCREEN=stats`, `level_check` or `journey`.

use kabar_core::review::seed_bridge_deck;
use kabar_core::settings::Settings;
use kabar_protocol::coach::{ActivityKind, CoachProfile, LocalDate, Skill, Target};
use kabar_protocol::level_check::{
    LEVEL_CHECK_KIND, LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_VERSION, LevelCheckResult, LevelEta, LevelMove,
    RadarPoint,
};
use kabar_protocol::srs::{Rating, ReviewMode};

use crate::store::{Store, now_ms};

const DAY_MS: u64 = 86_400_000;

#[test]
#[ignore = "writes a demo database; run by hand for previews"]
fn seed_stats_demo() {
    let path = std::env::var("KABAR_DEMO_DB").expect("set KABAR_DEMO_DB");
    let _ = std::fs::remove_file(&path);
    let mut store = Store::open(std::path::Path::new(&path)).unwrap();
    let old = store.settings().unwrap();
    let lang = std::env::var("KABAR_DEMO_LANG").unwrap_or_default();
    let new = Settings {
        welcomed: true,
        ui_language: match lang.as_str() {
            "en" => kabar_core::locale::UiLanguage::En,
            "ar" => kabar_core::locale::UiLanguage::Ar,
            _ => kabar_core::locale::UiLanguage::ArEg,
        },
        eastern_digits: std::env::var("KABAR_DEMO_EASTERN").is_ok(),
        ..old.clone()
    };
    store.save_settings(&old, &new).unwrap();
    let now = now_ms();
    let today = LocalDate::from_unix_ms(now, 120);
    let profile = CoachProfile {
        name: "Hamo".into(),
        target: Some(Target { level: "bipa2".into(), by: today.add_days(200) }),
        ..CoachProfile::default()
    };
    store.save_coach_profile(&profile).unwrap();
    seed_chat(&mut store, now);

    // ~16 weeks of study with rest days, a pattern that grows over time.
    for back in 0..110i32 {
        if back % 7 == 3 || back % 11 == 5 {
            continue;
        }
        let minutes = 5 + ((110 - back) as u32 * 7 + back as u32 * 3) % 28;
        let date = today.add_days(-back);
        store.add_activity(ActivityKind::Reviews { cards: 12, correct: 10 }, date, minutes * 60).unwrap();
        if back % 3 == 0 {
            let skill = Skill::ALL[(back as usize / 3) % 5];
            let kind = ActivityKind::Practice { skill, topic: "demo".into(), correct: 4, total: 6 };
            store.add_activity(kind, date, 0).unwrap();
        }
    }

    // Cards and a review history that makes some words known.
    let cards = seed_bridge_deck();
    store.upsert_cards(&cards).unwrap();
    // The app's scheduler for the demo profile's time zone.
    let sched = crate::schedule::scheduler_at(profile.utc_offset_min, &store.settings().unwrap());
    for (i, c) in cards.iter().enumerate() {
        let mut t = now - (100 + i as u64 % 20) * DAY_MS;
        let mut s = kabar_core::srs::CardState::new(c.id.as_str());
        let reps = i % 7;
        for r in 0..reps {
            if t >= now {
                break;
            }
            let rating = if (i + r) % 9 == 0 { Rating::Again } else { Rating::Good };
            store.record_review(&c.id, rating, t, 4000, ReviewMode::Classic).unwrap();
            s = sched.apply(&s, rating, t);
            t = s.due_ms.unwrap_or(t + DAY_MS);
        }
    }

    // KABAR_DEMO_PLACED=bipa3: placed above the levels that have test items,
    // so the check goes straight to its outcome (history only).
    if let Ok(level) = std::env::var("KABAR_DEMO_PLACED") {
        let placed = kabar_protocol::test::PlacementResult {
            start_level: level,
            start_unit: None,
            sections: Vec::new(),
        };
        store.record_lesson_event("placement", "placed", &placed).unwrap();
    }

    // Last month's check.
    let prev = LevelCheckResult {
        v: LEVEL_CHECK_VERSION,
        date: today.add_days(-31),
        language: "id".into(),
        from_level: "bipa1".into(),
        level: "bipa1".into(),
        moved: LevelMove::Stay,
        score: Some(0.62),
        radar: vec![
            RadarPoint { skill: Skill::Reading, value: 0.62 },
            RadarPoint { skill: Skill::Listening, value: 0.48 },
            RadarPoint { skill: Skill::Writing, value: 0.40 },
            RadarPoint { skill: Skill::Speaking, value: 0.35 },
            RadarPoint { skill: Skill::Vocabulary, value: 0.70 },
        ],
        eta: Some(LevelEta { level: "bipa2".into(), date: today.add_days(170) }),
        missed: Vec::new(),
        duration_ms: 560_000,
    };
    // KABAR_DEMO_FIRST_CHECK: no check yet, for the first check's welcome.
    if std::env::var("KABAR_DEMO_FIRST_CHECK").is_ok() {
        return;
    }
    store.record_lesson_event(LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_KIND, &prev).unwrap();
    if std::env::var("KABAR_DEMO_TWO_CHECKS").is_ok() {
        let now_check = LevelCheckResult {
            date: today,
            score: Some(0.71),
            radar: vec![
                RadarPoint { skill: Skill::Reading, value: 0.74 },
                RadarPoint { skill: Skill::Listening, value: 0.55 },
                RadarPoint { skill: Skill::Writing, value: 0.46 },
                RadarPoint { skill: Skill::Vocabulary, value: 0.78 },
            ],
            eta: Some(LevelEta { level: "bipa2".into(), date: today.add_days(140) }),
            ..prev
        };
        store.record_lesson_event(LEVEL_CHECK_LESSON_ID, LEVEL_CHECK_KIND, &now_check).unwrap();
    }
}

/// Mas Ardi's hotel check-in (scene d01) replayed from its example dialogue,
/// with the recast the scene teaches as a correction, so Chat previews show
/// bubbles, a fix and "spot on".
fn seed_chat(store: &mut Store, now: u64) {
    use kabar_protocol::chat::{ChatCorrection, ChatMessage, ChatRole, ChatRow, Conversation};
    let yaml = include_str!("../../../../content/personas/mas-ardi.yaml");
    let ardi = kabar_core::story::parse_persona(yaml).unwrap();
    let scene = ardi.example_dialogues.iter().find(|d| d.id == "d01").unwrap();
    let doc = "demo-chat-ardi".to_string();
    let conversation = Conversation {
        id: doc.clone(),
        doc: doc.clone(),
        persona: ardi.id.clone(),
        scenario: Some(scene.id.clone()),
        level: scene.level.clone(),
        register: "standard".into(),
        created_ms: now - 600_000,
        updated_ms: now - 60_000,
    };
    store.save_chat_row(&ChatRow::Conversation(conversation)).unwrap();
    for (i, line) in scene.lines.iter().take(5).enumerate() {
        let learner = line.speaker == kabar_core::chat::LEARNER;
        // The persona line after a learner line may recast its mistake.
        let fix = scene.lines.get(i + 1).and_then(|n| n.correction.as_ref()).filter(|_| learner);
        let message = ChatMessage {
            id: format!("{doc}-{i}"),
            doc: doc.clone(),
            role: if learner { ChatRole::Learner } else { ChatRole::Persona },
            text: line.text.clone(),
            corrections: fix
                .map(|c| ChatCorrection {
                    text: c.wrong.clone(),
                    fix: c.right.clone(),
                    why_en: c.why.en.clone(),
                    why_ar: c.why.ar_eg.clone(),
                })
                .into_iter()
                .collect(),
            register: None,
            unverified: false,
            created_ms: now - 600_000 + i as u64 * 60_000,
        };
        store.save_chat_row(&ChatRow::Message(message)).unwrap();
    }
}
