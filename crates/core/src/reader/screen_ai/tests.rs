//! `structure_screenshot` end to end against recorded replies in
//! `fixtures/ai/`, the OCR fixture and the golden set. No live calls.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use kabar_protocol::ai::{EngineCall, EngineError, EngineId, EngineReply};
use serde::Deserialize;

use super::*;
use crate::ai::{AiError, Engine, EngineFuture, MemoryCache, PromptSet, RouteTable, Router, Skip};

macro_rules! fixture {
    ($name:literal) => {
        serde_json::from_str::<EngineReply>(include_str!(concat!("../../../../../fixtures/ai/", $name)))
            .unwrap()
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
    fn calls(&self) -> Vec<EngineCall> {
        self.calls.borrow().clone()
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

/// WhatsApp, 1:1 with Pakdhe Joko, the §24 test texts.
fn whatsapp() -> OcrResult {
    serde_json::from_str(include_str!("../../../../../fixtures/ocr/whatsapp_jv_chat.mlkit.json")).unwrap()
}

#[derive(Deserialize)]
struct Golden {
    items: Vec<GoldenItem>,
}

#[derive(Deserialize)]
struct GoldenItem {
    id: String,
    input: Value,
    expect: Value,
}

fn golden() -> Vec<GoldenItem> {
    let text = include_str!("../../../../../content/golden/structure_screenshot.yaml");
    serde_yaml_ng::from_str::<Golden>(text).unwrap().items
}

fn golden_item(id: &str) -> GoldenItem {
    golden().into_iter().find(|i| i.id == id).unwrap()
}

/// The bake-off sends a golden `input` as is; the app never sends `image`
/// yet (no blob upload).
fn golden_request(id: &str) -> AiRequest {
    let mut input = golden_item(id).input;
    input.as_object_mut().unwrap().remove("image");
    AiRequest { input, personal: true, image_sha256: None }
}

fn bubbles(doc: &ScreenDoc) -> Vec<(Side, &str, Option<&str>, Option<&str>)> {
    doc.items
        .iter()
        .map(|i| {
            let side = match i.kind {
                ItemKind::Bubble { side } => side,
                _ => Side::Center,
            };
            (side, i.text.as_str(), i.time.as_deref(), i.speaker.as_deref())
        })
        .collect()
}

#[test]
fn input_uses_the_golden_shape_in_reading_order() {
    let req = request(&whatsapp());
    assert!(req.personal, "screenshots are personal");
    assert!(req.image_sha256.is_none());
    let blocks = req.input["ocr_blocks"].as_array().unwrap();
    assert_eq!(req.input.as_object().unwrap().len(), 1, "only ocr_blocks: no image until the blob store");
    assert_eq!(blocks[0], json!({"text": "12:45", "box": [40, 24, 110, 40]}));
    assert_eq!(blocks[2], json!({"text": "Pakdhe Joko online", "box": [150, 110, 330, 96]}));
    let texts: Vec<&str> = blocks.iter().map(|b| b["text"].as_str().unwrap()).collect();
    assert!(texts.contains(&"piye dhe 12:43"), "lines of a block are joined: {texts:?}");
    assert_eq!(texts.last(), Some(&"Ketik pesan"));
    // Every box is four whole pixels.
    assert!(blocks.iter().all(|b| b["box"].as_array().unwrap().iter().all(|v| v.is_i64())));
}

#[test]
fn whatsapp_chat_through_the_router_keeps_line_boxes() {
    let home = Scripted::new(
        EngineId::Local,
        vec![Ok(fixture!("structure_screenshot.whatsapp_jv_chat.local.json"))],
    );
    let r = router(&[&home]);
    let ocr = whatsapp();
    let a = block_on(r.run::<StructureScreenshot>(&request(&ocr))).unwrap();
    assert_eq!(a.engine, EngineId::Local);
    assert!(a.provisional && !a.needs_review);

    let call = &home.calls()[0];
    assert!(call.personal, "the engine call carries the personal flag");
    assert!(call.user.contains("Sek tak lapor pusat 12:41"));
    assert!(call.system.contains("right side: always \"me\""));
    assert_eq!(call.max_tokens, Some(4096));

    let doc = to_screen_doc(&a.value, &ocr);
    assert!(doc.looks_like_chat);
    assert_eq!(
        bubbles(&doc),
        [
            (Side::Left, "Sek tak lapor pusat", Some("12:41"), Some("Pakdhe Joko")),
            (Side::Right, "ng prabowo?", Some("12:42"), None),
            (Side::Left, "piye dhe", Some("12:43"), Some("Pakdhe Joko")),
            (Side::Right, "arp dicul piro", Some("12.44"), None),
        ]
    );
    // Same reading text as the deterministic pass, and the same boxes.
    let det = crate::reader::screenshot::structure(&ocr);
    assert_eq!(doc.reading_text(), det.reading_text());
    assert_eq!(doc.items[0].bbox, Rect { x: 60.0, y: 560.0, w: 520.0, h: 52.0 });
    // "piye dhe" is one line of a two-line block; the "12:43" line isn't its text.
    assert_eq!(doc.items[2].bbox.y, 760.0);
    assert!(doc.items[2].bbox.h < 96.0);
}

#[test]
fn group_chat_carries_speakers_over() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![Ok(fixture!("structure_screenshot.golden_001.workers_ai.json"))],
    );
    let r = router(&[&wai]);
    let a = block_on(r.run::<StructureScreenshot>(&golden_request("structure_screenshot-001"))).unwrap();
    assert_eq!(a.value.kind, ScreenKind::Chat);
    let speakers: Vec<Option<&str>> = a.value.messages.iter().map(|m| m.speaker.as_deref()).collect();
    assert_eq!(speakers, [Some("Pakdhe Harjo"), Some("me"), Some("Mas Wahyu"), Some("Mas Wahyu")]);
    // The answer is exactly the golden expectation.
    let expect: StructureScreenshot =
        serde_json::from_value(golden_item("structure_screenshot-001").expect).unwrap();
    assert_eq!(a.value, expect);
}

#[test]
fn listing_is_center_text_with_the_seller() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![Ok(fixture!("structure_screenshot.golden_017.workers_ai.json"))],
    );
    let r = router(&[&wai]);
    let a = block_on(r.run::<StructureScreenshot>(&golden_request("structure_screenshot-017"))).unwrap();
    assert_eq!(a.value.kind, ScreenKind::Listing);
    let doc = to_screen_doc(
        &a.value,
        &OcrResult {
            engine: kabar_protocol::reader::OcrEngine::MlKit,
            width: 450.0,
            height: 760.0,
            blocks: Vec::new(),
        },
    );
    assert!(!doc.looks_like_chat);
    assert!(doc.items.iter().all(|i| i.kind == ItemKind::Text));
    assert_eq!(doc.items[1].text, "Rp26.000.000");
    assert_eq!(doc.items[1].speaker.as_deref(), Some("Gadget Bekas Raka"));
}

#[test]
fn a_hallucinated_message_is_rejected_and_repaired() {
    let wai = Scripted::new(
        EngineId::WorkersAi,
        vec![
            Ok(fixture!("structure_screenshot.golden_001.hallucinated.workers_ai.json")),
            Ok(fixture!("structure_screenshot.golden_001.workers_ai.json")),
        ],
    );
    let r = router(&[&wai]);
    let a = block_on(r.run::<StructureScreenshot>(&golden_request("structure_screenshot-001"))).unwrap();
    assert_eq!(a.value.messages.len(), 4);
    let calls = wai.calls();
    assert!(calls[1].is_repair);
    assert!(
        calls[1].user.contains("message 5 (\"Oke, tak enteni kabare\") is not in the OCR text"),
        "{}",
        calls[1].user
    );
}

#[test]
fn hallucinating_twice_fails_and_the_deterministic_pass_stays() {
    let bad = || Ok(fixture!("structure_screenshot.golden_001.hallucinated.workers_ai.json"));
    let wai = Scripted::new(EngineId::WorkersAi, vec![bad(), bad()]);
    let r = router(&[&wai]);
    let err =
        block_on(r.run::<StructureScreenshot>(&golden_request("structure_screenshot-001"))).unwrap_err();
    assert!(matches!(err, AiError::AllFailed { .. }), "{err:?}");
}

#[test]
fn personal_screenshots_never_plan_gemini_free() {
    let gem = Scripted::new(EngineId::GeminiFree, vec![]);
    let wai = Scripted::new(EngineId::WorkersAi, vec![]);
    let home = Scripted::new(EngineId::Local, vec![]);
    let r = router(&[&gem, &wai, &home]);

    let req = request(&whatsapp());
    assert!(req.personal);
    let plan = r.plan(Task::StructureScreenshot, req.personal).unwrap();
    assert_eq!(plan, [EngineId::Local, EngineId::WorkersAi]);
    // Personal at task level too, so even an unflagged request can't reach it.
    assert!(Task::StructureScreenshot.is_personal());
    assert!(!r.plan(Task::StructureScreenshot, false).unwrap().contains(&EngineId::GeminiFree));

    // With only gemini connected there is nothing to run, and it is never called.
    let only_gem = router(&[&gem]);
    let err = block_on(only_gem.run::<StructureScreenshot>(&req)).unwrap_err();
    let AiError::NoEngine { skipped, .. } = &err else { panic!("{err:?}") };
    assert!(skipped.iter().all(|(_, s)| *s == Skip::NotConnected));
    assert!(gem.calls().is_empty());
}

#[test]
fn every_golden_expectation_validates_against_its_input() {
    let items = golden();
    assert!(items.len() >= 30);
    for item in items {
        let answer: StructureScreenshot =
            serde_json::from_value(item.expect.clone()).unwrap_or_else(|e| panic!("{}: {e}", item.id));
        answer.validate().unwrap_or_else(|e| panic!("{}: {e}", item.id));
        answer.validate_against(&item.input).unwrap_or_else(|e| panic!("{}: {e}", item.id));
    }
}

fn answer(kind: ScreenKind, messages: Vec<ScreenMessage>) -> StructureScreenshot {
    StructureScreenshot { kind, messages, confident: true }
}

fn msg(side: Side, speaker: Option<&str>, text: &str, time: Option<&str>) -> ScreenMessage {
    ScreenMessage { speaker: speaker.map(Into::into), side, text: text.into(), time: time.map(Into::into) }
}

#[test]
fn validation_rules() {
    let ok = |a: &StructureScreenshot| a.validate();
    assert!(ok(&answer(ScreenKind::Chat, vec![])).unwrap_err().contains("at least one"));
    assert!(
        ok(&answer(ScreenKind::Chat, vec![msg(Side::Left, None, "  ", None)])).unwrap_err().contains("empty")
    );
    assert!(
        ok(&answer(ScreenKind::Listing, vec![msg(Side::Left, Some("Raka"), "Rp26.000.000", None)]))
            .unwrap_err()
            .contains("center")
    );
    assert!(
        ok(&answer(ScreenKind::Chat, vec![msg(Side::Right, Some("Mas Wahyu"), "piye dhe", None)]))
            .unwrap_err()
            .contains("\"me\"")
    );
    assert!(
        ok(&answer(ScreenKind::Chat, vec![msg(Side::Left, None, "piye dhe", Some("Rp26.000"))]))
            .unwrap_err()
            .contains("not a time")
    );
    assert!(ok(&answer(ScreenKind::Chat, vec![msg(Side::Right, None, "piye dhe", Some("10.34"))])).is_ok());

    let unsure = StructureScreenshot { confident: false, ..answer(ScreenKind::Post, vec![]) };
    assert!(unsure.needs_review());
    // Missing "confident" means confident (the golden expect shape).
    let a: StructureScreenshot = serde_json::from_value(
        json!({"kind": "ui", "messages": [{"speaker": "app", "side": "center", "text": "Selesai"}]}),
    )
    .unwrap();
    assert!(a.confident && !a.needs_review());
    // Unknown kinds and fields are refused by serde.
    assert!(serde_json::from_value::<StructureScreenshot>(json!({"kind": "email", "messages": []})).is_err());
}

#[test]
fn tracing_tolerates_ocr_fixes_but_not_inventions() {
    let input = json!({"ocr_blocks": [
        {"text": "Sek tak lapor pusst 10.33", "box": [0, 0, 1, 1]},
        {"text": "arp dicul piroSek", "box": [0, 0, 1, 1]},
        {"text": "Ke", "box": [0, 0, 1, 1]},
        {"text": "Siti Aminah", "box": [0, 0, 1, 1]},
    ]});
    let check = |text: &str, time: Option<&str>| {
        answer(ScreenKind::Chat, vec![msg(Side::Left, None, text, time)]).validate_against(&input)
    };
    assert!(check("Sek tak lapor pusat", Some("10.33")).is_ok(), "one-letter OCR slip fixed");
    assert!(check("arp dicul piro 👍", None).is_ok(), "glued words split, emoji ignored");
    assert!(check("Ke: Siti Aminah", None).is_ok(), "label and value joined");
    assert!(check("Hold on, I'll report to HQ", None).unwrap_err().contains("not in the OCR text"));
    assert!(check("arp dicul piro", Some("10.35")).unwrap_err().contains("time"));
    assert!(check("Sek tak lapor bos", None).is_err(), "one invented word in a short message");
}

#[test]
fn times() {
    for t in [
        "10.42",
        "08:05 PM",
        "2 jam",
        "3j",
        "5m ago",
        "1 hari yang lalu",
        "1-6",
        "12/10",
        "12-10-2026",
        "12 Okt",
        "12 Okt 2026",
        "12 Okt 2026, 14.22",
        "Kemarin 21.40",
        "Kemarin",
        "Senin",
        "baru saja",
    ] {
        assert!(looks_like_time(t), "{t}");
    }
    for t in ["", "Rp26.000", "26.000", "piye dhe", "512gb", "2 rumah", "1.204 suka"] {
        assert!(!looks_like_time(t), "{t}");
    }
    assert!(
        within_one_edit("pusat", "pusst")
            && within_one_edit("pusat", "pusa")
            && within_one_edit("pusat", "pusat")
    );
    assert!(!within_one_edit("pusat", "pasar"));
}
