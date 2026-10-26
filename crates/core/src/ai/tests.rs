//! Router tests against recorded replies in `fixtures/ai/`. No live calls.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use serde_json::json;

use super::tasks::{AnalyzeText, GradeAnswer, TokenLang, Verdict};
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
    // Aksara Jawa stroke checking lands in M13, with its prompt.
    #[derive(Debug, serde::Deserialize)]
    struct Strokes;
    impl AiOutput for Strokes {
        const TASK: Task = Task::CheckStrokes;
    }
    let wai = Scripted::new(EngineId::WorkersAi, vec![]);
    let r = router(&[&wai]);
    let err = block_on(r.run::<Strokes>(&AiRequest::default())).unwrap_err();
    assert_eq!(err, AiError::NoPrompt(Task::CheckStrokes));
}
