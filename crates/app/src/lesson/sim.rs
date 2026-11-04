//! The lesson-player style of driving a screen in tests: pick elements with
//! a closure over [`Node`] and fire their real handlers. A thin front end
//! over the shared test kit: [`TestDom`] renders and dispatches (events
//! bubble as a tap does), [`FakeDoc`] is the WebView and
//! [`testkit::provide`] the contexts.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::html::{SerializedFormData, SerializedMouseData};
use dioxus::prelude::*;

pub(crate) use crate::test_dom::Node;
use crate::test_dom::TestDom;
use crate::testkit::{self, FakeDoc, Setup};

/// A screen under test.
pub(crate) struct Sim {
    pub dom: TestDom,
}

impl Sim {
    pub fn new(app: fn() -> Element) -> Self {
        let mut sim = Sim { dom: TestDom::unsettled(VirtualDom::new(app)) };
        sim.settle();
        sim
    }

    /// Runs ready tasks, effects and re-renders until nothing is left to do
    /// (tasks waiting on a timer or the WebView stay parked).
    pub fn settle(&mut self) {
        for _ in 0..20 {
            self.dom.render();
        }
    }

    /// Lets `ms` of real time pass (timers fire), then settles.
    pub fn wait(&mut self, ms: u64) {
        self.dom.sleep(ms);
        self.settle();
    }

    /// The rendered page without its `<style>` blocks.
    pub fn html(&mut self) -> String {
        self.settle();
        crate::test_screens::html(&self.dom)
    }

    /// Runs `f` inside the app's root scope (to read or set context state).
    pub fn with<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let out = self.dom.peek_app(f);
        self.settle();
        out
    }

    pub fn nodes(&self) -> Vec<Node> {
        self.dom.nodes()
    }

    pub fn find(&self, pick: impl Fn(&Node) -> bool) -> Option<Node> {
        self.nodes().into_iter().find(|n| pick(n))
    }

    pub fn count(&self, pick: impl Fn(&Node) -> bool) -> usize {
        self.nodes().iter().filter(|n| pick(n)).count()
    }

    fn fire(&mut self, event: &str, pick: &dyn Fn(&Node) -> bool, data: Box<dyn std::any::Any>) {
        let node = self.nodes().into_iter().find(|n| n.listens(event) && pick(n)).unwrap_or_else(|| {
            let seen: Vec<String> = self
                .nodes()
                .iter()
                .filter(|n| n.listens(event))
                .map(|n| format!("<{} class={:?}> {:?}", n.tag, n.class(), n.text))
                .collect();
            panic!("no element listening to {event} matches; listening:\n{}", seen.join("\n"))
        });
        assert!(!node.disabled(), "{event} on a disabled <{}>: {:?}", node.tag, node.text);
        self.dom.dispatch(node.el, event, data, true);
        self.settle();
    }

    /// Clicks the first element listening to clicks that matches (a
    /// disabled match fails the test: a user can't tap it).
    pub fn click(&mut self, pick: impl Fn(&Node) -> bool) {
        self.fire("click", &pick, Box::new(SerializedMouseData::default()));
    }

    /// Clicks the button whose text contains `text`.
    pub fn click_text(&mut self, text: &str) {
        self.click(|n| n.tag == "button" && n.text.contains(text));
    }

    /// Types `value` into the first matching input (an `input` event).
    pub fn input(&mut self, pick: impl Fn(&Node) -> bool, value: &str) {
        self.fire("input", &pick, Box::new(SerializedFormData::new(value.to_string(), vec![])));
    }

    /// Submits the first matching form.
    pub fn submit(&mut self, pick: impl Fn(&Node) -> bool) {
        self.fire("submit", &pick, Box::new(SerializedFormData::new(String::new(), vec![])));
    }

    /// The open script containing `pattern` sends `value` (a key press, a
    /// timer tick, a drop on the wall).
    pub fn page_sends(&mut self, pattern: &str, value: serde_json::Value) {
        page().send(pattern, value);
        self.settle();
    }
}

// ---------------------------------------------------------------------------
// The WebView for the next test app, set up before `Sim::new`
// ---------------------------------------------------------------------------

type Replies = Box<dyn FnMut(&str) -> Vec<serde_json::Value>>;

thread_local! {
    static REPLIES: RefCell<Option<Replies>> = const { RefCell::new(None) };
    static LIVE: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    /// The WebView of the app [`provide`] last set up.
    static PAGE: RefCell<Option<Rc<FakeDoc>>> = const { RefCell::new(None) };
}

fn page() -> Rc<FakeDoc> {
    PAGE.with_borrow(|p| p.clone()).expect("sim::provide sets up the page")
}

/// Every script run since the test app started.
pub(crate) fn scripts() -> Vec<String> {
    PAGE.with_borrow(|p| p.as_ref().map(|d| d.scripts()).unwrap_or_default())
}

/// What scripts send back right away in the next test app (none by
/// default: as if the WebView answered nothing). Set before `Sim::new`.
pub(crate) fn on_eval(f: impl FnMut(&str) -> Vec<serde_json::Value> + 'static) {
    REPLIES.set(Some(Box::new(f)));
}

/// Scripts containing `pattern` stay open in the next test app, waiting for
/// [`Sim::page_sends`]. Set before `Sim::new`.
pub(crate) fn live(pattern: &'static str) {
    LIVE.with_borrow_mut(|l| l.push(pattern));
}

/// The contexts every learning screen reads (settings, store, coach, Play
/// navigation, the word index and the Teach inbox), around an in-memory
/// store. Call at the top of a test app; returns the `Ui` and `Db`.
pub(crate) fn provide(width: kabar_core::layout::WidthClass) -> (crate::Ui, crate::Db) {
    testkit::provide(|| {
        let doc = FakeDoc::new();
        if let Some(f) = REPLIES.take() {
            doc.on_eval(f);
        }
        for pattern in LIVE.take() {
            doc.live(pattern);
        }
        PAGE.set(Some(doc.clone()));
        let settings = kabar_core::settings::Settings::default();
        Setup {
            lang: settings.ui_language,
            settings,
            width,
            screen: crate::Screen::Journey,
            doc: Some(doc),
            ..Setup::default()
        }
    })
}
