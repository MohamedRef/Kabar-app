//! Teach Kabar end to end against recorded replies in `fixtures/ai/` and the
//! §24 test texts. No live calls.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use kabar_protocol::ai::{EngineCall, EngineError, EngineId, EngineReply};
use kabar_protocol::content::{Pos, Register};
use kabar_protocol::sync::{SyncKind, table_kind};

use super::*;
use crate::ai::{AiError, Engine as AiEngine, EngineFuture, MemoryCache, PromptSet, RouteTable, Router};

macro_rules! fixture {
    ($name:literal) => {
        serde_json::from_str::<EngineReply>(include_str!(concat!("../../../../fixtures/ai/", $name))).unwrap()
    };
}

fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    match f.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("test engines never pend"),
    }
}

#[derive(Clone)]
struct Scripted {
    id: EngineId,
    replies: Rc<RefCell<VecDeque<Result<EngineReply, EngineError>>>>,
    calls: Rc<RefCell<Vec<EngineCall>>>,
}

impl Scripted {
    fn new(id: EngineId, replies: Vec<Result<EngineReply, EngineError>>) -> Self {
        Self { id, replies: Rc::new(RefCell::new(replies.into())), calls: Rc::default() }
    }
}

impl AiEngine for Scripted {
    fn id(&self) -> EngineId {
        self.id
    }
    fn is_free(&self) -> bool {
        true
    }
    fn call<'a>(&'a self, call: &'a EngineCall) -> EngineFuture<'a> {
        self.calls.borrow_mut().push(call.clone());
        let next = self.replies.borrow_mut().pop_front();
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

fn engine() -> Engine {
    Engine::default()
}

fn dicul_request() -> TeachRequest {
    TeachRequest {
        lang: Lang::Jv,
        word: " Dicul ".into(),
        // docs/SPEC.md §24
        context: Some("arp dicul piro".into()),
        sources: vec![SourceInput::Pasted {
            text: "dicul (bahasa Jawa) = diculke, dijual. Contoh: motore arep dicul. Hubungi 0812-3456-7890"
                .into(),
        }],
    }
}

#[test]
fn dicul_from_a_pasted_google_snippet() {
    let prepared = prepare(&dicul_request(), &engine()).unwrap();
    assert_eq!(prepared.word, "dicul");
    assert!(!prepared.request.personal);
    assert!(prepared.sources[0].excerpt.contains("[phone]"));
    assert!(!prepared.sources[0].excerpt.contains("0812"));
    assert_eq!(prepared.request.input["context"], "arp dicul piro");
    assert_eq!(prepared.hints.root.as_deref(), Some("cul"), "{:?}", prepared.hints);

    let wai = Scripted::new(EngineId::WorkersAi, vec![Ok(fixture!("learn_entry.dicul.workers_ai.json"))]);
    let r = router(&[&wai]);
    let answer = block_on(r.run::<LearnEntry>(&prepared.request)).unwrap();
    assert!(answer.needs_review);
    let call = &wai.calls.borrow()[0];
    assert!(call.user.contains("arp dicul piro") && call.system.contains("Egyptian colloquial"));
    assert!(!call.personal);

    let t = draft(&prepared, &answer);
    assert_eq!(t.row_id(), "jv:dicul");
    assert!(t.is_draft());
    assert_eq!(t.entry.provenance.generator, "learner");
    assert_eq!(t.entry.provenance.prompt, "learn_entry@v1");
    assert_eq!(t.entry.rank, 0);
    assert_eq!(t.entry.affixes, ["di-", "cul", "-ke"]);
    assert_eq!(t.entry.senses[0].register, Register::Colloquial);
    assert_eq!(t.levels.as_ref().unwrap().level, "ngoko");
    assert_eq!(t.engine, "workers_ai");
    assert_eq!(t.doubts.len(), 1, "the model's doubt only; it agrees on the root");
    assert_eq!(t.context.as_deref(), Some("arp dicul piro"));

    // The sync payload stays small and round-trips.
    let json = serde_json::to_string(&t).unwrap();
    assert!(json.len() < kabar_protocol::sync::MAX_PAYLOAD_BYTES);
    assert_eq!(serde_json::from_str::<TaughtEntry>(&json).unwrap(), t);
}

#[test]
fn invalid_reply_moves_on_to_the_next_engine() {
    let req = TeachRequest {
        lang: Lang::Id,
        word: "mantul".into(),
        context: Some("Makanannya mantul!".into()),
        sources: vec![SourceInput::Link {
            url: "https://example.com/arti-mantul".into(),
            html: Some(
                "<title>Arti mantul</title><p>Mantul artinya mantap betul, alias keren banget.</p>".into(),
            ),
        }],
    };
    let prepared = prepare(&req, &engine()).unwrap();
    assert_eq!(prepared.sources[0].title.as_deref(), Some("Arti mantul"));
    assert_eq!(prepared.sources[0].kind, SourceKind::Link);

    // Workers AI fails twice (an example without the word, Latin "arz"),
    // Gemini answers inside a fence.
    let bad = fixture!("learn_entry.mantul.invalid.workers_ai.json");
    let wai = Scripted::new(EngineId::WorkersAi, vec![Ok(bad.clone()), Ok(bad)]);
    let gem = Scripted::new(EngineId::GeminiFree, vec![Ok(fixture!("learn_entry.mantul.gemini_free.json"))]);
    let r = router(&[&wai, &gem]);
    let answer = block_on(r.run::<LearnEntry>(&prepared.request)).unwrap();
    assert_eq!(answer.engine, EngineId::GeminiFree);
    let repair = &wai.calls.borrow()[1];
    assert!(repair.is_repair && repair.user.contains("Arabic script"), "{}", repair.user);

    let t = draft(&prepared, &answer);
    assert_eq!(t.entry.senses[0].register, Register::Slang);
    assert!(t.levels.is_none());
    assert!(t.doubts.is_empty());
}

#[test]
fn screenshots_are_personal_and_never_reach_gemini() {
    let req = TeachRequest {
        lang: Lang::Jv,
        word: "piye".into(),
        context: None,
        sources: vec![SourceInput::Screenshot { ocr_text: "Pakdhe: piye dhe?\nAku: apik".into() }],
    };
    let prepared = prepare(&req, &engine()).unwrap();
    assert!(prepared.request.personal);

    let gem = Scripted::new(EngineId::GeminiFree, vec![]);
    let r = router(&[&gem]);
    assert!(matches!(r.plan(Task::LearnEntry, prepared.request.personal), Err(AiError::NoEngine { .. })));
    assert!(r.plan(Task::LearnEntry, false).is_ok());

    let wai =
        Scripted::new(EngineId::WorkersAi, vec![Err(EngineError::Unavailable { reason: "offline".into() })]);
    let r = router(&[&gem, &wai]);
    assert!(block_on(r.run::<LearnEntry>(&prepared.request)).is_err());
    assert!(wai.calls.borrow()[0].personal);
    assert!(gem.calls.borrow().is_empty());
}

#[test]
fn bad_requests_say_why() {
    let e = engine();
    let mut req = dicul_request();
    req.word = "  ".into();
    assert_eq!(prepare(&req, &e).unwrap_err(), TeachError::EmptyWord);
    req.word = "x".repeat(61);
    assert_eq!(prepare(&req, &e).unwrap_err(), TeachError::WordTooLong);

    req.word = "dicul".into();
    req.sources = vec![SourceInput::Pasted { text: "   ".into() }];
    assert_eq!(prepare(&req, &e).unwrap_err(), TeachError::NothingToLearnFrom);
    req.sources = vec![SourceInput::Link { url: "https://x.id".into(), html: None }];
    assert_eq!(prepare(&req, &e).unwrap_err(), TeachError::LinkUnreadable { url: "https://x.id".into() });

    let wiki: Value =
        serde_json::from_str(include_str!("../../../../fixtures/learn/wiktionary.kabar.json")).unwrap();
    req.sources = vec![SourceInput::Wiktionary { json: wiki.clone() }];
    assert_eq!(prepare(&req, &e).unwrap_err(), TeachError::NotInWiktionary);

    let ok = prepare(
        &TeachRequest {
            lang: Lang::Id,
            word: "kabar".into(),
            context: None,
            sources: vec![SourceInput::Wiktionary { json: wiki }],
        },
        &e,
    )
    .unwrap();
    assert_eq!(ok.sources[0].kind, SourceKind::Wiktionary);
    assert!(ok.sources[0].excerpt.contains("news"));
    assert_eq!(ok.sources[0].url.as_deref(), Some("https://en.wiktionary.org/wiki/kabar"));
}

fn kalimat() -> (Prepared, Answer<LearnEntry>) {
    let req = TeachRequest {
        lang: Lang::Id,
        word: "kalimat".into(),
        context: None,
        sources: vec![SourceInput::Pasted { text: "kalimat: satuan bahasa, sentence".into() }],
    };
    let prepared = prepare(&req, &engine()).unwrap();
    let local = Scripted::new(EngineId::Local, vec![Ok(fixture!("learn_entry.kalimat.local.json"))]);
    let answer = block_on(router(&[&local]).run::<LearnEntry>(&prepared.request)).unwrap();
    (prepared, answer)
}

#[test]
fn false_friend_keeps_its_bridge_card() {
    let (prepared, answer) = kalimat();
    let t = draft(&prepared, &answer);
    assert_eq!(t.entry.arabic_origin.as_deref(), Some("كَلِمَة"));
    assert!(t.entry.false_friend.is_some());
    let b = t.bridge.as_ref().unwrap();
    assert_eq!(b.shift, kabar_protocol::content::MeaningShift::FalseFriend);
    assert_eq!(b.root.as_deref(), Some("ك ل م"));
    assert_eq!(t.engine, "local");
}

#[test]
fn validation_catches_what_serde_does_not() {
    let (_, answer) = kalimat();
    let good = answer.value;
    assert!(good.validate().is_ok());

    let mut v = good.clone();
    v.false_friend = Some(v.false_friend.clone().unwrap());
    v.arabic_origin = None;
    v.bridge = None;
    assert!(v.validate().unwrap_err().contains("false_friend needs arabic_origin"));

    let mut v = good.clone();
    v.bridge.as_mut().unwrap().arabic = "kalima".into();
    assert!(v.validate().unwrap_err().contains("bridge.arabic"));

    let mut v = good.clone();
    v.examples[1].sense = 3;
    assert!(v.validate().unwrap_err().contains("sense"));

    let mut v = good.clone();
    v.root = Some("kalim".into());
    v.affixes = vec!["ka-".into(), "lim".into()];
    assert!(v.validate().unwrap_err().contains("affixes must include the root"));

    let mut v = good;
    v.senses.clear();
    assert!(v.validate().unwrap_err().contains("1-4 senses"));
}

#[test]
fn root_disagreement_and_low_confidence_become_doubts() {
    let prepared = prepare(&dicul_request(), &engine()).unwrap();
    let wai = Scripted::new(EngineId::WorkersAi, vec![Ok(fixture!("learn_entry.dicul.workers_ai.json"))]);
    let mut answer = block_on(router(&[&wai]).run::<LearnEntry>(&prepared.request)).unwrap();
    answer.value.root = Some("ucul".into());
    answer.value.doubts.clear();
    answer.value.confident = false;
    let t = draft(&prepared, &answer);
    assert_eq!(t.doubts.len(), 2);
    assert!(t.doubts[0].en.contains("“cul”") && t.doubts[0].en.contains("“ucul”"));
    assert!(t.doubts[1].arz.contains("مش متأكد"));
}

#[test]
fn review_edit_approve_and_popover_order() {
    let (prepared, answer) = kalimat();
    let t = draft(&prepared, &answer);

    // A hand edit keeps the word and language and must stay valid.
    let edited = edit(&t, |e| {
        e.entry.senses[0].gloss.en = "sentence (grammar)".into();
        e.entry.word = "other".into();
    })
    .unwrap();
    assert!(edited.edited);
    assert_eq!(edited.entry.word, "kalimat");
    assert!(edit(&t, |e| e.entry.examples.clear()).is_err());
    assert!(!edit(&t, |_| {}).unwrap().edited);

    let prebuilt = GlossEntry { rank: 812, ..t.entry.clone() };
    assert_eq!(
        popover_layers(Some(&edited), Some(&prebuilt)),
        [GlossLayer::Prebuilt(&prebuilt), GlossLayer::Draft(&edited)]
    );

    let approved = approve(&edited);
    assert!(approved.is_approved());
    assert_eq!(
        popover_layers(Some(&approved), Some(&prebuilt)),
        [GlossLayer::Taught(&approved), GlossLayer::Prebuilt(&prebuilt)]
    );
    assert_eq!(popover_layers(None, None), []);

    let mut dict = PersonalDictionary::new([approved.clone()]);
    assert_eq!(dict.lookup_forms(Lang::Id, &["Kalimatnya", "kalimat"]), Some(&approved));
    assert!(dict.lookup_forms(Lang::Jv, &["kalimat"]).is_none());
    assert_eq!(dict.approved().count(), 1);
    assert_eq!(dict.to_review().count(), 0);
    dict.insert(t.clone());
    assert_eq!(dict.len(), 1, "same row id replaces");
    assert_eq!(dict.to_review().count(), 1);
    assert!(dict.remove(Lang::Id, "KALIMAT").is_some());
    assert!(dict.is_empty());
}

#[test]
fn lemma_lookup_finds_inflected_forms() {
    let prepared = prepare(&dicul_request(), &engine()).unwrap();
    let wai = Scripted::new(EngineId::WorkersAi, vec![Ok(fixture!("learn_entry.dicul.workers_ai.json"))]);
    let answer = block_on(router(&[&wai]).run::<LearnEntry>(&prepared.request)).unwrap();
    let t = approve(&draft(&prepared, &answer));
    let dict = PersonalDictionary::new([t]);
    // "diculke" is the lemma of the taught "dicul".
    assert_eq!(dict.lookup_forms(Lang::Jv, &["diculke"]).unwrap().entry.word, "dicul");
    assert_eq!(dict.get(Lang::Jv, "dicul").unwrap().entry.senses[0].pos, Pos::Verb);
}

#[test]
fn taught_entries_sync_last_writer_wins() {
    assert_eq!(table_kind(kabar_protocol::learn::TAUGHT_TABLE), Some(SyncKind::Lww));
}
