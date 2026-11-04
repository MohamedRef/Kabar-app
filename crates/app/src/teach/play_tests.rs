//! Teach Kabar driven through its buttons, with recorded AI replies from
//! `fixtures/ai/` (no live calls): teach, review, edit, approve, discard,
//! another version, and every way it can fail.

use std::cell::RefCell;
use std::collections::VecDeque;

use kabar_core::ai::{Engine as AiEngine, EngineFuture, MemoryCache, PromptSet, RouteTable};
use kabar_core::layout::WidthClass;
use kabar_protocol::ai::{EngineCall, EngineError, EngineId, EngineReply};
use kabar_protocol::hlc::DeviceId;

use super::*;
use crate::cloud::CloudStatus;
use crate::lesson::sim::{self, Node, Sim};

macro_rules! fixture {
    ($name:literal) => {
        serde_json::from_str::<EngineReply>(include_str!(concat!("../../../../fixtures/ai/", $name))).unwrap()
    };
}

#[derive(Clone, Default)]
struct Scripted {
    /// Workers AI unless set.
    id: Option<EngineId>,
    replies: Rc<RefCell<VecDeque<Result<EngineReply, EngineError>>>>,
    calls: Rc<RefCell<Vec<EngineCall>>>,
}

impl AiEngine for Scripted {
    fn id(&self) -> EngineId {
        self.id.unwrap_or(EngineId::WorkersAi)
    }
    fn is_free(&self) -> bool {
        true
    }
    fn call<'a>(&'a self, call: &'a EngineCall) -> EngineFuture<'a> {
        self.calls.borrow_mut().push(call.clone());
        let next = self.replies.borrow_mut().pop_front();
        Box::pin(async move { next.unwrap_or_else(|| panic!("called more than scripted")) })
    }
}

thread_local! {
    /// The engine the next test app's router uses (None: no engines at all).
    static ENGINE: RefCell<Option<Scripted>> = const { RefCell::new(None) };
    static SECOND: RefCell<Option<Scripted>> = const { RefCell::new(None) };
    static CONFIGURED: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static SEED: RefCell<Option<TeachSeed>> = const { RefCell::new(None) };
}

fn app() -> Element {
    let (mut ui, _db) = sim::provide(WidthClass::Compact);
    use_hook(move || ui.settings.write().ui_language = UiLanguage::En);
    let seed = SEED.with_borrow_mut(Option::take);
    use_hook(move || consume_context::<TeachInbox>().0.clone().set(seed));
    use_context_provider(|| {
        let mut r =
            Router::new(RouteTable::builtin(), PromptSet::builtin(), Box::new(MemoryCache::default()));
        for e in [ENGINE.with_borrow(Clone::clone), SECOND.with_borrow(Clone::clone)].into_iter().flatten() {
            r = r.with_engine(Box::new(e));
        }
        Ai(Rc::new(r))
    });
    use_context_provider(|| {
        let mut status = CloudStatus::not_configured(DeviceId("test-device".into()));
        status.configured = CONFIGURED.get();
        Cloud {
            status: Signal::new(status),
            devices: Signal::new(None),
            usage: Signal::new(None),
            signing_in: Signal::new(false),
            revision: Signal::new(0),
            commands: Signal::new(None),
            push: Signal::new(false),
            push_sent: Signal::new(None),
            login: Signal::new(Default::default()),
        }
    });
    rsx! { TeachScreen {} }
}

fn open(replies: Option<Vec<Result<EngineReply, EngineError>>>) -> (Sim, Scripted) {
    let engine = Scripted::default();
    if let Some(r) = &replies {
        *engine.replies.borrow_mut() = r.iter().cloned().collect();
    }
    ENGINE.set(replies.map(|_| engine.clone()));
    (Sim::new(app), engine)
}

fn en(m: T) -> String {
    tr(UiLanguage::En, m).replace('&', "&amp;").replace('\'', "&#39;")
}

fn button(text: T) -> impl Fn(&Node) -> bool {
    let label = tr(UiLanguage::En, text).to_string();
    move |n| n.tag == "button" && n.text.contains(&label)
}

fn db(sim: &mut Sim) -> Db {
    sim.with(consume_context::<Db>)
}

/// Fills the form for the §24 example: Javanese "dicul" from a pasted note.
fn fill_dicul(sim: &mut Sim) {
    sim.input(|n| n.attr("id") == Some("teach-word"), "dicul");
    sim.click(|n| n.attr("role") == Some("radio") && n.text.contains("Basa Jawa"));
    sim.input(|n| n.attr("id") == Some("teach-context"), "arp dicul piro");
    sim.input(
        |n| n.tag == "textarea",
        "dicul (bahasa Jawa) = diculke, dijual. Contoh: motore arep dicul. Hubungi 0812-3456-7890",
    );
}

#[test]
fn teaching_a_word_makes_a_draft_that_can_be_approved() {
    let (mut sim, engine) = open(Some(vec![Ok(fixture!("learn_entry.dicul.workers_ai.json"))]));
    let html = sim.html();
    assert!(html.contains(&en(T::Title)), "{html}");
    assert!(sim.find(button(T::Teach)).unwrap().disabled(), "no word yet");
    fill_dicul(&mut sim);
    assert!(sim.find(|n| n.attr("aria-checked") == Some("true")).unwrap().text.contains("Basa Jawa"));
    sim.click(button(T::Teach));
    let call = &engine.calls.borrow()[0];
    assert!(call.user.contains("arp dicul piro"));
    assert!(!call.user.contains("0812"), "phone numbers never leave the device");
    let html = sim.html();
    assert!(html.contains("teach-review") && html.contains(&en(T::Draft)), "{html}");
    assert!(html.contains(&en(T::Unverified)), "Javanese stays unverified");
    assert!(html.contains(&en(T::Doubts)), "the model's doubt shows");
    assert!(html.contains("dicul → "), "the breakdown: {html}");
    // Saved as a draft right away, listed under "to review".
    let saved = db(&mut sim).0.borrow().taught_entries().unwrap();
    assert_eq!(saved.len(), 1);
    assert!(saved[0].is_draft());
    assert!(html.contains(&en(T::MyDictionary)) && html.contains(&en(T::ToReview)));
    sim.click(button(T::Approve));
    let saved = db(&mut sim).0.borrow().taught_entries().unwrap();
    assert!(saved[0].is_approved());
    let html = sim.html();
    assert!(!html.contains("teach-review"), "the review closes");
    assert!(html.contains(&en(T::TaughtByYou)));
    // Opening it from the dictionary shows it again, without "another version".
    sim.click(|n| n.inside("taught-list") && n.text.contains("dicul"));
    let html = sim.html();
    assert!(html.contains("teach-review") && html.contains("pill taught"), "{html}");
    assert!(sim.find(button(T::Another)).is_none());
}

#[test]
fn a_draft_can_be_edited_regenerated_and_discarded() {
    let (mut sim, engine) = open(Some(vec![Ok(fixture!("learn_entry.dicul.workers_ai.json"))]));
    fill_dicul(&mut sim);
    sim.click(button(T::Teach));
    sim.click(button(T::Edit));
    let html = sim.html();
    assert!(html.contains("sense-edit") && html.contains(&en(T::Lemma)), "{html}");
    sim.input(|n| n.tag == "input" && n.inside("teach-review") && !n.inside("sense-edit"), "dicul");
    sim.input(|n| n.inside("sense-edit") && n.attr("dir") == Some("auto"), "يتباع");
    sim.click(button(T::SaveEdits));
    let saved = db(&mut sim).0.borrow().taught_entries().unwrap();
    assert_eq!(saved[0].entry.senses[0].gloss.arz, "يتباع");
    assert_eq!(saved[0].entry.lemma, "dicul");
    let html = sim.html();
    assert!(!html.contains("sense-edit"), "back to reading");
    assert!(!html.contains("dicul → "), "the lemma is the word now: {html}");
    // Another version goes to the next engine; with only one there is
    // none, which is said kindly and keeps the draft.
    sim.click(button(T::Another));
    assert_eq!(engine.calls.borrow().len(), 1);
    let html = sim.html();
    assert!(html.contains("class=\"no\" role=\"status\""), "{html}");
    assert!(html.contains("teach-review"));
    assert_eq!(db(&mut sim).0.borrow().taught_entries().unwrap().len(), 1);
    sim.click(button(T::Discard));
    assert!(db(&mut sim).0.borrow().taught_entries().unwrap().is_empty());
    assert!(!sim.html().contains("teach-review"));
}

#[test]
fn an_edit_that_breaks_the_entry_says_why_and_saves_nothing() {
    let (mut sim, _) = open(Some(vec![Ok(fixture!("learn_entry.dicul.workers_ai.json"))]));
    fill_dicul(&mut sim);
    sim.click(button(T::Teach));
    let before = db(&mut sim).0.borrow().taught_entries().unwrap();
    sim.click(button(T::Edit));
    for _ in 0..3 {
        if sim
            .find(|n| n.inside("sense-edit") && n.attr("dir") == Some("auto") && n.attr("value") != Some(""))
            .is_none()
        {
            break;
        }
        sim.input(
            |n| n.inside("sense-edit") && n.attr("dir") == Some("auto") && n.attr("value") != Some(""),
            "",
        );
    }
    sim.click(button(T::SaveEdits));
    assert!(sim.find(|n| n.has_class("no") && n.attr("role") == Some("status")).is_some(), "{}", sim.html());
    assert_eq!(db(&mut sim).0.borrow().taught_entries().unwrap(), before);
}

#[test]
fn problems_say_what_to_do() {
    // Nothing to learn from.
    let (mut sim, _) = open(Some(vec![]));
    sim.input(|n| n.attr("id") == Some("teach-word"), "mantul");
    sim.click(button(T::Teach));
    assert!(sim.html().contains(&en(T::NeedSource)));
    // No engine reachable: Kabar Cloud is the fix, with a way to sign in.
    let (mut sim, _) = open(None);
    sim.input(|n| n.attr("id") == Some("teach-word"), "mantul");
    sim.input(|n| n.tag == "textarea", "mantul artinya mantap betul");
    sim.click(button(T::Teach));
    let html = sim.html();
    assert!(html.contains(&en(T::NotConnected)), "{html}");
    sim.click(button(T::SignIn));
    assert_eq!(*sim.with(consume_context::<Ui>).screen.peek(), Screen::Devices);
    // A build with no Kabar Cloud says so instead.
    CONFIGURED.set(false);
    let (mut sim, _) = open(None);
    CONFIGURED.set(true);
    sim.input(|n| n.attr("id") == Some("teach-word"), "mantul");
    sim.input(|n| n.tag == "textarea", "mantul artinya mantap betul");
    sim.click(button(T::Teach));
    assert!(sim.html().contains(&en(T::NoCloudInBuild)));
    assert!(sim.find(button(T::SignIn)).is_none());
    // The engine's quota is used up.
    let (mut sim, _) = open(Some(vec![Err(EngineError::QuotaExhausted { retry_after_s: Some(60) })]));
    sim.input(|n| n.attr("id") == Some("teach-word"), "mantul");
    sim.input(|n| n.tag == "textarea", "mantul artinya mantap betul");
    sim.click(button(T::Teach));
    let html = sim.html();
    assert!(html.contains("class=\"no\""), "{html}");
    assert!(db(&mut sim).0.borrow().taught_entries().unwrap().is_empty());
}

#[test]
fn links_and_wiktionary_fetch_through_the_webview_and_fail_gently() {
    let (mut sim, _) = open(Some(vec![]));
    sim.input(|n| n.attr("id") == Some("teach-word"), "mantul");
    sim.click(button(T::ModeLink));
    assert!(sim.find(|n| n.attr("type") == Some("url")).is_some());
    sim.input(|n| n.attr("type") == Some("url"), "https://example.com/mantul");
    sim.click(button(T::Teach));
    // No WebView here: the page can't be read, so paste it instead.
    assert!(sim.html().contains(&en(T::LinkFailed)), "{}", sim.html());
    sim.click(button(T::ModeWiktionary));
    sim.click(button(T::Teach));
    assert!(sim.html().contains(&en(T::WiktionaryFailed)));
    // Google opens in the browser with the word.
    let google = sim.find(|n| n.tag == "a" && n.has_class("button")).unwrap();
    assert!(google.attr("href").unwrap().contains("mantul"));
}

#[test]
fn a_seed_from_the_reader_fills_the_form() {
    SEED.set(Some(TeachSeed {
        lang: Lang::Jv,
        word: "dicul".into(),
        context: Some("arp dicul piro".into()),
    }));
    let (mut sim, _) = open(Some(vec![]));
    let html = sim.html();
    assert!(html.contains("value=\"dicul\"") && html.contains("value=\"arp dicul piro\""), "{html}");
    assert!(sim.find(|n| n.attr("aria-checked") == Some("true")).unwrap().text.contains("Basa Jawa"));
}

#[test]
fn every_string_has_all_three_languages_and_errors_map_to_messages() {
    use kabar_core::ai::FriendlyReason;
    for t in [
        T::Title,
        T::Intro,
        T::Word,
        T::Context,
        T::Source,
        T::ModePaste,
        T::ModeLink,
        T::ModeWiktionary,
        T::Screenshot,
        T::SearchGoogle,
        T::PastePlaceholder,
        T::Teach,
        T::Fetching,
        T::Thinking,
        T::TaughtByYou,
        T::Draft,
        T::Unverified,
        T::Doubts,
        T::Lemma,
        T::Example,
        T::Sources,
        T::Approve,
        T::Edit,
        T::SaveEdits,
        T::Another,
        T::Discard,
        T::MyDictionary,
        T::ToReview,
        T::NeedWord,
        T::TooLong,
        T::NeedSource,
        T::LinkFailed,
        T::NotInWiktionary,
        T::WiktionaryFailed,
        T::ResetsLater,
        T::Unreachable,
        T::NotConnected,
        T::NoCloudInBuild,
        T::SignIn,
        T::TryAgain,
    ] {
        let [arz, ar, en] = [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En].map(|l| tr(l, t));
        assert!(!arz.is_empty() && !ar.is_empty() && !en.is_empty(), "{t:?}");
        assert!(arz.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)), "{t:?} arz is Arabic");
    }
    assert_eq!(teach_error(&TeachError::EmptyWord), T::NeedWord);
    assert_eq!(teach_error(&TeachError::WordTooLong), T::TooLong);
    assert_eq!(teach_error(&TeachError::NotInWiktionary), T::NotInWiktionary);
    let _ = FriendlyReason::TryAgain;
    assert_eq!(source_name(SourceKind::Screenshot, UiLanguage::En), tr(UiLanguage::En, T::Screenshot));
    assert_eq!(source_name(SourceKind::Pasted, UiLanguage::En), tr(UiLanguage::En, T::ModePaste));
    let mut t = Trilingual { arz: "a".into(), ar: "b".into(), en: "c".into() };
    for (v, val) in [("arz", "x"), ("ar", "y"), ("en", "z")] {
        set_voice(&mut t, v, val.into());
    }
    assert_eq!(
        (voice(&t, UiLanguage::ArEg), voice(&t, UiLanguage::Ar), voice(&t, UiLanguage::En)),
        ("x", "y", "z")
    );
}

/// A link the page can fetch is read and taught from.
#[test]
fn a_fetched_link_is_taught_from() {
    sim::on_eval(|js| {
        if js.contains("await dioxus.recv()") {
            vec![serde_json::json!({
                "ok": true,
                "body": "<title>Arti mantul</title><p>Mantul artinya mantap betul, alias keren banget.</p>"
            })]
        } else {
            vec![]
        }
    });
    let (mut sim, engine) = open(Some(vec![Ok(fixture!("learn_entry.dicul.workers_ai.json"))]));
    sim.input(|n| n.attr("id") == Some("teach-word"), "mantul");
    sim.click(button(T::ModeLink));
    sim.input(|n| n.attr("type") == Some("url"), " https://example.com/arti-mantul ");
    sim.click(button(T::Teach));
    let calls = engine.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].user.contains("mantap betul"), "the page's text reaches the model");
    drop(calls);
    let html = sim.html();
    assert!(html.contains("teach-review"), "{html}");
    assert!(html.contains("https://example.com/arti-mantul"), "the source link is listed");
}

/// Wiktionary's answer is read when the page gets it.
#[test]
fn a_wiktionary_entry_is_taught_from() {
    let json = include_str!("../../../../fixtures/learn/wiktionary.kabar.json").to_string();
    sim::on_eval(move |js| {
        if js.contains("await dioxus.recv()") {
            vec![serde_json::json!({ "ok": true, "body": json })]
        } else {
            vec![]
        }
    });
    let (mut sim, engine) = open(Some(vec![Ok(fixture!("learn_entry.dicul.workers_ai.json"))]));
    sim.input(|n| n.attr("id") == Some("teach-word"), "kabar");
    sim.click(button(T::ModeWiktionary));
    sim.click(button(T::Teach));
    let html = sim.html();
    let called = engine.calls.borrow().len();
    assert_eq!(called, 1, "{html}");
    assert!(html.contains("teach-review"));
    let fetched = sim::scripts().iter().filter(|s| s.contains("await dioxus.recv()")).count();
    assert_eq!(fetched, 1);
}

/// "Another version" asks the next engine and shows its draft instead.
#[test]
fn another_version_comes_from_the_next_engine() {
    let first = Scripted::default();
    first.replies.borrow_mut().push_back(Ok(fixture!("learn_entry.dicul.workers_ai.json")));
    let second = Scripted { id: Some(EngineId::GeminiFree), ..Scripted::default() };
    second.replies.borrow_mut().push_back(Ok(fixture!("learn_entry.dicul.workers_ai.json")));
    ENGINE.set(Some(first.clone()));
    SECOND.set(Some(second.clone()));
    let mut sim = Sim::new(app);
    fill_dicul(&mut sim);
    sim.click(button(T::Teach));
    let plan_first = first.calls.borrow().len() + second.calls.borrow().len();
    assert_eq!(plan_first, 1);
    sim.click(button(T::Another));
    assert_eq!(first.calls.borrow().len() + second.calls.borrow().len(), 2, "one more engine asked");
    let html = sim.html();
    assert!(html.contains("teach-review") && !html.contains("class=\"no\""), "{html}");
}
