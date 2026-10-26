//! The Studio's AI tasks through the router, against recorded replies in
//! `fixtures/ai/`. No live calls.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use kabar_protocol::ai::{EngineCall, EngineError, EngineId, EngineReply, Task};
use kabar_protocol::content::Lang;

use super::gambar::{SketchGuess, Verdict, judge};
use super::mindmap::{self, MindMap};
use super::recognize::{self, Handwriting};
use crate::ai::{AiError, Engine, EngineFuture, MemoryCache, PromptSet, RouteTable, Router};

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

impl Engine for Scripted {
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

#[test]
fn mind_map_retries_a_thin_map_then_lays_it_out() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![
            Ok(fixture!("mind_map.di_pasar.too_few.workers_ai.json")),
            Ok(fixture!("mind_map.di_pasar.workers_ai.json")),
        ],
    );
    let req =
        mindmap::request("di pasar", "bipa1", &["pasar".into(), "beli".into(), "murah".into()], Lang::Id);
    let answer = block_on(router(&[&wai]).run::<MindMap>(&req)).unwrap();
    let calls = wai.calls.borrow();
    assert!(calls[1].is_repair && calls[1].user.contains("3-6 branches"));
    assert!(calls[0].user.contains("\"known_words\""));
    assert_eq!(answer.value.branches.len(), 4);
    assert!(!answer.needs_review);
    let mut n = 0;
    let items = mindmap::layout(&answer.value, "map", 400.0, 300.0, || {
        n += 1;
        n.to_string()
    });
    assert_eq!(items.len(), (1 + 4 + 10) * 2 - 1);
}

#[test]
fn sketches_are_personal_and_skip_gemini() {
    let gem = Scripted::new(EngineId::GeminiFree, vec![]);
    let wai = Scripted::new(EngineId::WorkersAi, vec![Ok(fixture!("guess_sketch.kucing.workers_ai.json"))]);
    let r = router(&[&gem, &wai]);
    let req = recognize::sketch_request("ab12");
    let answer = block_on(r.run::<SketchGuess>(&req)).unwrap();
    assert_eq!(judge("kucing", &answer.value), Verdict::Got);
    let call = &wai.calls.borrow()[0];
    assert!(call.personal && call.image_sha256.as_deref() == Some("ab12"));
    assert!(!call.user.contains("kucing"), "the target word is never sent");
    assert!(gem.calls.borrow().is_empty());
    assert!(matches!(router(&[&gem]).plan(Task::GuessSketch, true), Err(AiError::NoEngine { .. })));
}

#[test]
fn handwriting_keeps_the_learners_spelling() {
    let local = Scripted::new(EngineId::Local, vec![Ok(fixture!("read_handwriting.berkerja.local.json"))]);
    let req = recognize::handwriting_request(Lang::Id, Some("Saya bekerja di Jakarta."), "cd34");
    let answer = block_on(router(&[&local]).run::<Handwriting>(&req)).unwrap();
    assert_eq!(answer.value.text, "Saya berkerja di Jakarta.");
    let marks = super::answer::letter_diff("Saya bekerja di Jakarta.", &answer.value.text);
    assert_eq!(super::answer::mistakes(&marks), 1);
}
