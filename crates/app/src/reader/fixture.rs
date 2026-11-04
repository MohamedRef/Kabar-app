//! Test harness for the Reader and annotated text: the shared test kit's
//! contexts ([`crate::testkit::provide`]) over an in-memory store, a
//! scripted AI engine fed from `fixtures/ai/`, and a way to drive spawned
//! work to the end on [`TestDom`]. Asserts on the SSR HTML the learner
//! would see. No live calls.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use kabar_core::layout::WidthClass;
use kabar_core::locale::UiLanguage;
use kabar_core::settings::Settings;
use kabar_protocol::ai::EngineId;

use super::ReaderState;
use crate::Db;
use crate::test_dom::TestDom;
pub use crate::testkit::Script;
use crate::testkit::{self, Scripted, Setup};

/// One screen to render and the world around it.
#[derive(Clone)]
pub struct Case {
    pub lang: UiLanguage,
    pub width: WidthClass,
    pub tablet: bool,
    pub settings: fn(&mut Settings),
    /// Runs once before the first render, with the contexts in place.
    pub prep: fn(&Db, ReaderState),
    pub body: fn() -> Element,
    /// An AI engine answering from a script, as `engine`.
    pub ai: Option<Rc<Script>>,
    pub engine: EngineId,
    /// Signed in to Kabar Cloud.
    pub signed_in: bool,
}

impl PartialEq for Case {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Case {
    pub fn new(body: fn() -> Element) -> Self {
        Self {
            lang: UiLanguage::En,
            width: WidthClass::Expanded,
            tablet: false,
            settings: |_| {},
            prep: |_, _| {},
            body,
            ai: None,
            engine: EngineId::WorkersAi,
            signed_in: false,
        }
    }

    /// The shared test kit's setup for this case.
    fn setup(&self) -> Setup {
        let mut settings = Settings { ui_language: self.lang, ..Settings::default() };
        (self.settings)(&mut settings);
        Setup {
            lang: self.lang,
            settings,
            width: self.width,
            tablet: self.tablet,
            screen: crate::Screen::Reader,
            engines: self
                .ai
                .iter()
                .map(|script| Scripted { id: self.engine, script: script.clone() })
                .collect(),
            ai: self.ai.is_some(),
            account: true,
            signed_in: self.signed_in,
            ..Setup::default()
        }
    }
}

fn harness(case: Case) -> Element {
    let (_, db) = testkit::provide(|| case.setup());
    let st = consume_context::<ReaderState>();
    use_hook(|| (case.prep)(&db, st));
    (case.body)()
}

/// A rendered screen that can be driven and re-rendered.
pub struct Screenful {
    pub dom: TestDom,
}

impl Screenful {
    pub fn new(case: Case) -> Self {
        Self { dom: TestDom::unsettled(VirtualDom::new_with_props(harness, case)) }
    }

    /// The HTML as it stands now.
    pub fn html(&self) -> String {
        self.dom.ssr()
    }

    /// Runs spawned tasks, effects and resources until nothing is left to
    /// do (or a short wait passes), then re-renders.
    pub fn settle(&mut self) -> String {
        self.dom.run_until_quiet(Duration::from_millis(30), 50);
        self.dom.render();
        self.html()
    }

    /// Runs `f` with the harness's contexts, as an event handler would.
    pub fn act<R>(&mut self, f: impl FnOnce(&Db, ReaderState) -> R) -> R {
        let out = self.dom.peek_app(|| f(&consume_context::<Db>(), consume_context::<ReaderState>()));
        self.dom.render();
        out
    }

    pub fn ctx<T: Clone + 'static>(&self) -> T {
        self.dom.peek_app(consume_context::<T>)
    }
}

/// Renders `case` once and returns its HTML after spawned work settled.
pub fn render(case: Case) -> String {
    let mut s = Screenful::new(case);
    s.settle()
}

/// Saves `body` as a pasted text and opens it in the Reader.
pub fn open_text(db: &Db, st: ReaderState, body: &str) {
    let text = db.0.borrow_mut().save_text(body, crate::store::TextKind::Pasted, None, None).unwrap();
    st.load(db, text);
}

/// The plain text of some HTML: tags dropped, the entities SSR writes decoded.
pub fn text_of(html: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in html.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    for (entity, c) in [
        ("&#39;", "'"),
        ("&#34;", "\""),
        ("&quot;", "\""),
        ("&lt;", "<"),
        ("&#60;", "<"),
        ("&gt;", ">"),
        ("&#62;", ">"),
        ("&amp;", "&"),
    ] {
        out = out.replace(entity, c);
    }
    out
}

/// The English (or other) text of a message, as the screen shows it.
pub fn msg(lang: UiLanguage, m: crate::i18n::Msg) -> &'static str {
    crate::i18n::t(lang, m)
}
