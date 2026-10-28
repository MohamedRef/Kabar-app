//! Router tests against recorded replies in `fixtures/ai/`. No live calls.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use serde_json::json;

use super::tasks::{
    AnalyzeText, DailyPlan, GradeAnswer, GuessSketch, LevelCheck, LevelMove, PlanKind, Register,
    RoleplayTurn, TokenLang, UNCLEAR_SKETCH, Verdict,
};
use super::*;

macro_rules! fixture {
    ($name:literal) => {
        serde_json::from_str::<EngineReply>(include_str!(concat!("../../../../fixtures/ai/", $name))).unwrap()
    };
}

/// Engines here resolve immediately, so one poll is enough.
fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    match f.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("test engines never pend"),
    }
}

#[derive(Default)]
struct Script {
    replies: RefCell<VecDeque<Result<EngineReply, EngineError>>>,
    calls: RefCell<Vec<EngineCall>>,
    forgotten: RefCell<Vec<CacheKey>>,
}

#[derive(Clone)]
struct Scripted {
    id: EngineId,
    free: bool,
    script: Rc<Script>,
}

impl Scripted {
    fn new(id: EngineId, replies: Vec<Result<EngineReply, EngineError>>) -> Self {
        let script = Script { replies: RefCell::new(replies.into()), ..Script::default() };
        Self { id, free: true, script: Rc::new(script) }
    }
    fn calls(&self) -> Vec<EngineCall> {
        self.script.calls.borrow().clone()
    }
}

impl Engine for Scripted {
    fn id(&self) -> EngineId {
        self.id
    }
    fn is_free(&self) -> bool {
        self.free
    }
    fn call<'a>(&'a self, call: &'a EngineCall) -> EngineFuture<'a> {
        self.script.calls.borrow_mut().push(call.clone());
        let next = self.script.replies.borrow_mut().pop_front();
        Box::pin(async move { next.unwrap_or_else(|| panic!("{:?} called more than scripted", self.id)) })
    }
    fn forget<'a>(&'a self, key: &'a CacheKey) -> ForgetFuture<'a> {
        self.script.forgotten.borrow_mut().push(key.clone());
        Box::pin(std::future::ready(()))
    }
}

fn router(engines: &[&Scripted]) -> Router {
    let mut r = Router::new(RouteTable::builtin(), PromptSet::builtin(), Box::new(MemoryCache::default()));
    for e in engines {
        r = r.with_engine(Box::new((*e).clone()));
    }
    r
}

fn sek_request() -> AiRequest {
    AiRequest::new(json!({"text": "Sek tak lapor pusat", "words": ["Sek", "tak", "lapor", "pusat"]}))
}

#[test]
fn parses_fenced_reply_and_caches_it() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![Ok(fixture!("analyze_text.sek_tak_lapor_pusat.workers_ai.json"))],
    );
    let r = router(&[&wai]);

    let a = block_on(r.run::<AnalyzeText>(&sek_request())).unwrap();
    assert_eq!(a.engine, EngineId::WorkersAi);
    assert!(!a.from_cache && a.provisional);
    assert_eq!(a.value.translation_en, "Hold on, I'll report to HQ.");
    assert_eq!(a.value.tokens[1].lang, TokenLang::Jv);
    assert!(a.needs_review, "Javanese tokens stay unverified");

    let call = &wai.calls()[0];
    assert!(call.user.contains("Sek tak lapor pusat") && call.system.contains("Egyptian Arabic"));
    assert_eq!(call.cache_key, a.cache_key);
    assert!(call.json);
    assert_eq!(call.max_tokens, Some(4096));

    // Same input, keys in another order: served from cache, no new call.
    let again =
        AiRequest::new(json!({"words": ["Sek", "tak", "lapor", "pusat"], "text": "Sek tak lapor pusat"}));
    let b = block_on(r.run::<AnalyzeText>(&again)).unwrap();
    assert!(b.from_cache);
    assert_eq!(b.value, a.value);
    assert_eq!(wai.calls().len(), 1);
}

#[test]
fn retries_once_with_the_error_then_succeeds() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![
            Ok(fixture!("analyze_text.sek_tak_lapor_pusat.invalid.workers_ai.json")),
            Ok(fixture!("analyze_text.sek_tak_lapor_pusat.workers_ai.json")),
        ],
    );
    let r = router(&[&wai]);
    let a = block_on(r.run::<AnalyzeText>(&sek_request())).unwrap();
    assert_eq!(a.engine, EngineId::WorkersAi);

    let calls = wai.calls();
    assert_eq!(calls.len(), 2);
    assert!(!calls[0].is_repair && calls[1].is_repair);
    assert!(calls[1].user.contains("missing field `gloss_ar`"), "{}", calls[1].user);
}

#[test]
fn two_bad_replies_move_to_the_next_engine() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![
            Ok(fixture!("analyze_text.sek_tak_lapor_pusat.invalid.workers_ai.json")),
            Ok(fixture!("analyze_text.sek_tak_lapor_pusat.invalid.workers_ai.json")),
        ],
    );
    let gem = Scripted::new(
        EngineId::GeminiFree,
        vec![
            Ok(fixture!("analyze_text.sek_tak_lapor_pusat.latin_ar.gemini_free.json")),
            Ok(fixture!("analyze_text.sek_tak_lapor_pusat.gemini_free.json")),
        ],
    );
    let r = router(&[&wai, &gem]);
    let a = block_on(r.run::<AnalyzeText>(&sek_request())).unwrap();
    assert_eq!(a.engine, EngineId::GeminiFree);
    assert!(gem.calls()[1].user.contains("Arabic script"), "validation error is fed back");
}

#[test]
fn every_failure_is_reported() {
    let wai =
        Scripted::new(EngineId::WorkersAi, vec![Err(EngineError::QuotaExhausted { retry_after_s: None })]);
    let gem =
        Scripted::new(EngineId::GeminiFree, vec![Err(EngineError::RateLimited { retry_after_s: Some(30) })]);
    let r = router(&[&wai, &gem]);
    let err = block_on(r.run::<AnalyzeText>(&sek_request())).unwrap_err();
    let AiError::AllFailed { attempts, .. } = &err else { panic!("{err:?}") };
    assert_eq!(
        attempts.iter().map(|a| a.engine).collect::<Vec<_>>(),
        [EngineId::WorkersAi, EngineId::GeminiFree]
    );
    assert_eq!(err.friendly(), FriendlyReason::ResetsLater);

    let home =
        Scripted::new(EngineId::Local, vec![Err(EngineError::Unavailable { reason: "asleep".into() })]);
    let r = router(&[&home]);
    assert_eq!(
        block_on(r.run::<AnalyzeText>(&sek_request())).unwrap_err().friendly(),
        FriendlyReason::Unreachable
    );
}

#[test]
fn signed_out_is_not_called_offline() {
    let cloud = Scripted::new(
        EngineId::WorkersAi,
        vec![Err(EngineError::NotConnected { reason: "not signed in to Kabar Cloud".into() })],
    );
    let home =
        Scripted::new(EngineId::Local, vec![Err(EngineError::Unavailable { reason: "asleep".into() })]);
    let r = router(&[&cloud, &home]);
    assert_eq!(
        block_on(r.run::<AnalyzeText>(&sek_request())).unwrap_err().friendly(),
        FriendlyReason::NotAvailable
    );
}

#[test]
fn personal_data_never_reaches_gemini() {
    let gem = Scripted::new(EngineId::GeminiFree, vec![]);
    let wai = Scripted::new(EngineId::WorkersAi, vec![]);
    let r = router(&[&gem, &wai]);

    // A personal task: gemini is not even in its route.
    assert_eq!(r.plan(Task::CoachMessage, false).unwrap(), [EngineId::WorkersAi]);

    // A learning task whose input is personal (journal text): gemini is skipped.
    assert_eq!(r.plan(Task::AnalyzeText, false).unwrap(), [EngineId::WorkersAi, EngineId::GeminiFree]);
    assert_eq!(r.plan(Task::AnalyzeText, true).unwrap(), [EngineId::WorkersAi]);

    let only_gem = router(&[&gem]);
    let err = only_gem.plan(Task::AnalyzeText, true).unwrap_err();
    assert!(
        matches!(&err, AiError::NoEngine { skipped, .. } if skipped.contains(&(EngineId::GeminiFree, Skip::Privacy)))
    );
    assert!(gem.calls().is_empty());
}

#[test]
fn claude_is_off_and_paid_engines_need_consent() {
    let mut claude = Scripted::new(EngineId::Claude, vec![]);
    claude.free = false;
    let mut r = router(&[&claude]);
    let skipped = |r: &Router| match r.plan(Task::GradeAnswer, false).unwrap_err() {
        AiError::NoEngine { skipped, .. } => skipped,
        e => panic!("{e:?}"),
    };
    assert!(skipped(&r).contains(&(EngineId::Claude, Skip::SwitchedOff)));

    r.settings.disabled.clear();
    assert!(skipped(&r).contains(&(EngineId::Claude, Skip::Paid)), "$0 hard stop");

    r.settings.allow_paid = true;
    assert_eq!(r.plan(Task::GradeAnswer, false).unwrap(), [EngineId::Claude]);
}

#[test]
fn thumbs_down_replaces_the_cached_answer_with_the_next_engine() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![Ok(fixture!("analyze_text.sek_tak_lapor_pusat.workers_ai.json"))],
    );
    let gem = Scripted::new(
        EngineId::GeminiFree,
        vec![Ok(fixture!("analyze_text.sek_tak_lapor_pusat.gemini_free.json"))],
    );
    let r = router(&[&wai, &gem]);
    let req = sek_request();

    let first = block_on(r.run::<AnalyzeText>(&req)).unwrap();
    let second = block_on(r.regenerate(&req, &first)).unwrap();
    assert_eq!(second.engine, EngineId::GeminiFree);
    // Kabar Cloud is told too, so its shared cache stops serving it.
    assert_eq!(*wai.script.forgotten.borrow(), std::slice::from_ref(&first.cache_key));
    assert!(gem.script.forgotten.borrow().is_empty());
    assert_eq!(second.value.tokens[0].gloss_en, "wait");

    // From now on the replacement is what's served.
    let third = block_on(r.run::<AnalyzeText>(&req)).unwrap();
    assert!(third.from_cache);
    assert_eq!(third.engine, EngineId::GeminiFree);
    assert_eq!(wai.calls().len(), 1);

    // Nothing after the last engine: say so, don't loop.
    assert!(matches!(block_on(r.regenerate(&req, &third)), Err(AiError::AllFailed { .. })));
}

#[test]
fn prompts_carry_the_profile_and_a_new_level_misses_the_old_answer() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![
            Ok(fixture!("analyze_text.sek_tak_lapor_pusat.workers_ai.json")),
            Ok(fixture!("analyze_text.sek_tak_lapor_pusat.workers_ai.json")),
        ],
    );
    let r = router(&[&wai]);
    let req = sek_request();
    r.set_profile(LearnerProfile {
        indonesian_level: "BIPA 3".into(),
        javanese_level: "J1".into(),
        known_words: 640,
        ..LearnerProfile::default()
    });
    let a = block_on(r.run::<AnalyzeText>(&req)).unwrap();
    let sys = &wai.calls()[0].system;
    assert!(sys.contains("Indonesian level: BIPA 3. Javanese level: J1."), "{sys}");
    assert!(sys.contains("Known words: about 500."), "{sys}");

    // A few more words: same band, same key, served from the cache.
    r.set_profile(LearnerProfile { known_words: 700, ..r.profile() });
    assert!(block_on(r.run::<AnalyzeText>(&req)).unwrap().from_cache);

    // A new level: a new key, so the answer is written again for it.
    r.set_profile(LearnerProfile { indonesian_level: "BIPA 4".into(), ..r.profile() });
    let b = block_on(r.run::<AnalyzeText>(&req)).unwrap();
    assert!(!b.from_cache);
    assert_ne!(a.cache_key, b.cache_key);
    assert_ne!(a.cache_key.id(), b.cache_key.id());
    assert_eq!(wai.calls().len(), 2);
}

#[test]
fn grade_answer_checks_score_against_verdict() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![
            Ok(fixture!("grade_answer.saya_sedang_membaca_buku.inconsistent.workers_ai.json")),
            Ok(fixture!("grade_answer.saya_sedang_membaca_buku.workers_ai.json")),
        ],
    );
    let r = router(&[&wai]);
    let req = AiRequest::new(json!({
        "exercise": "Translate: I am reading a book.",
        "references": ["Saya sedang membaca buku.", "Aku lagi baca buku."],
        "answer": "Saya sedang baca buku."
    }));
    let a = block_on(r.run::<GradeAnswer>(&req)).unwrap();
    assert_eq!((a.value.verdict, a.value.score), (Verdict::Almost, 75));
    assert!(!a.needs_review);
    assert!(wai.calls()[1].user.contains("does not match verdict"));
}

#[test]
fn tasks_without_a_prompt_are_reported_not_hidden() {
    #[derive(Debug, serde::Deserialize)]
    struct Map;
    impl AiOutput for Map {
        const TASK: Task = Task::MindMap;
    }
    let wai = Scripted::new(EngineId::WorkersAi, vec![]);
    // Every task ships a prompt today, so build a router without any.
    let r = Router::new(RouteTable::builtin(), PromptSet::default(), Box::new(MemoryCache::default()))
        .with_engine(Box::new(wai.clone()));
    let err = block_on(r.run::<Map>(&AiRequest::default())).unwrap_err();
    assert_eq!(err, AiError::NoPrompt(Task::MindMap));
    assert!(block_on(r.run::<AnalyzeText>(&sek_request())).is_err());
}

#[test]
fn a_plan_over_the_goal_is_sent_back_to_the_model() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![
            Ok(fixture!("daily_plan.evening_backlog.over_goal.workers_ai.json")),
            Ok(fixture!("daily_plan.evening_backlog.workers_ai.json")),
        ],
    );
    let r = router(&[&wai]);
    let req = AiRequest::new(json!({
        "date": "2026-10-27",
        "goal_minutes": 5,
        "due_reviews": 30,
        "weak_points": ["tidak vs bukan"],
        "next_lesson": {"id": "bipa1.u08.l02", "title": "Tidak atau bukan?", "minutes": 12},
        "streak_days": 12,
        "time_of_day": "evening"
    }));
    let a = block_on(r.run::<DailyPlan>(&req)).unwrap();
    assert_eq!(a.value.items.len(), 1);
    assert_eq!(a.value.items[0].kind, PlanKind::Review);

    let calls = wai.calls();
    assert!(calls[1].user.contains("add up to goal_minutes (5)"), "{}", calls[1].user);
    // A personal task: gemini is not in the route at all.
    assert_eq!(r.plan(Task::DailyPlan, false).unwrap(), [EngineId::WorkersAi]);
}

#[test]
fn the_level_rule_decides_not_the_model() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![
            Ok(fixture!("level_check.bipa1_no_test.wrong_move.workers_ai.json")),
            Ok(fixture!("level_check.bipa1_no_test.workers_ai.json")),
        ],
    );
    let r = router(&[&wai]);
    // No level test and healthy stats: nobody moves (content/golden/README.md).
    let req = AiRequest::new(json!({
        "lang": "id",
        "current_level": "bipa1",
        "stats": {
            "period_days": 30, "active_days": 18, "minutes": 300,
            "units_done": 6, "units_total": 10, "level_test": null,
            "review_accuracy": 0.82, "retention": 0.80, "known_words": 260
        },
        "samples": []
    }));
    let a = block_on(r.run::<LevelCheck>(&req)).unwrap();
    assert_eq!((a.value.level.as_str(), a.value.moved), ("bipa1", LevelMove::Stay));
    assert!(wai.calls()[1].user.contains("moved Stay"), "{}", wai.calls()[1].user);
}

#[test]
fn javanese_roleplay_and_maps_stay_unverified() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![Ok(fixture!("roleplay_turn.angkringan_dimas.workers_ai.json"))],
    );
    let r = router(&[&wai]);
    let req = AiRequest::new(json!({
        "persona": "mas_dimas",
        "level": "j1",
        "scenario": "Meeting your friend Dimas at an angkringan after class in Yogyakarta.",
        "history": [{"role": "persona", "text": "Halo! Piye kabare? Wis mangan durung?"}],
        "learner": "Aku belum makan, lapar banget."
    }));
    let a = block_on(r.run::<RoleplayTurn>(&req)).unwrap();
    assert_eq!(a.value.register, Register::Ngoko);
    assert_eq!(a.value.corrections.len(), 2);
    assert!(a.needs_review, "Javanese personas are unverified until the learner approves");
}

#[test]
fn a_persona_chat_turn_uses_the_style_guide_and_repairs_a_bad_quote() {
    let dimas =
        crate::story::parse_persona(include_str!("../../../../content/personas/mas-dimas.yaml")).unwrap();
    let scene = crate::chat::scenarios(&dimas).into_iter().find(|s| s.id.as_deref() == Some("d01")).unwrap();
    let opener = kabar_protocol::chat::ChatMessage {
        id: "m1".into(),
        doc: "c1".into(),
        role: kabar_protocol::chat::ChatRole::Persona,
        text: scene.opener.clone(),
        corrections: vec![],
        register: None,
        unverified: true,
        created_ms: 0,
    };
    let req =
        crate::chat::request(&dimas, "j1", &scene, "ngoko", &[opener], "Aku belum makan, lapar banget.");
    // The first answer quotes words the learner never wrote: repaired once.
    let bad = EngineReply {
        engine: EngineId::WorkersAi,
        text: r#"{"reply": "Ayo mangan!", "corrections": [{"text": "aku ngelak", "fix": "aku ngelih", "why_en": "x", "why_ar": "س"}], "register": "ngoko"}"#.into(),
        model: String::new(),
    };
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![Ok(bad), Ok(fixture!("roleplay_turn.angkringan_dimas.workers_ai.json"))],
    );
    let r = router(&[&wai]);
    let a = block_on(r.run::<RoleplayTurn>(&req)).unwrap();
    assert_eq!(a.value.corrections.len(), 2);
    assert!(a.needs_review);
    assert_eq!(a.prompt_version, 2);
    let calls = wai.calls();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].is_repair);
    assert!(calls[0].system.contains("character sheet"), "{}", calls[0].system);
    assert!(calls[0].user.contains("level_guide") && calls[0].user.contains("register_hint"));
}

#[test]
fn a_blank_sketch_is_unclear_and_lemmas_stay_indonesian() {
    let gem = Scripted::new(
        EngineId::GeminiFree,
        vec![
            Ok(fixture!("guess_sketch.blank_canvas.english.gemini_free.json")),
            Ok(fixture!("guess_sketch.blank_canvas.gemini_free.json")),
        ],
    );
    let r = router(&[&gem]);
    let a = block_on(r.run::<GuessSketch>(&AiRequest::new(json!({"target_word": "rumah"})))).unwrap();
    assert_eq!(a.value.guess_id, UNCLEAR_SKETCH);
    assert!(a.value.alternatives.is_empty());
    assert!(gem.calls()[1].user.contains("snake_case"), "{}", gem.calls()[1].user);
}
