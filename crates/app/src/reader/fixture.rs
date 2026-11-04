//! Test harness for the Reader and annotated text: the app's contexts over
//! an in-memory store, a scripted AI engine fed from `fixtures/ai/`, and a
//! way to drive spawned work to the end. Renders with Dioxus SSR, so tests
//! assert on the HTML the learner would see. No live calls.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use kabar_core::ai::{Engine, EngineFuture, MemoryCache, PromptSet, RouteTable, Router};
use kabar_core::layout::WidthClass;
use kabar_core::locale::UiLanguage;
use kabar_core::settings::Settings;
use kabar_protocol::ai::{EngineCall, EngineError, EngineId, EngineReply};

use super::{Inbox, ReaderState};
use crate::account::Cloud;
use crate::cloud::CloudStatus;
use crate::store::Store;
use crate::teach::{Ai, TeachInbox};
use crate::{Db, Screen, Ui};

/// Replies an engine gives, in order; each call is recorded.
#[derive(Default)]
pub struct Script {
    pub replies: RefCell<VecDeque<Result<EngineReply, EngineError>>>,
    pub calls: RefCell<Vec<EngineCall>>,
}

impl Script {
    /// A script that answers `values` (task outputs as JSON text) in order.
    pub fn answering(values: &[&str]) -> Rc<Self> {
        let replies = values
            .iter()
            .map(|v| {
                Ok(EngineReply { engine: EngineId::WorkersAi, text: v.to_string(), model: "test".into() })
            })
            .collect();
        Rc::new(Self { replies: RefCell::new(replies), calls: RefCell::default() })
    }

    /// A script whose every call fails with `e`.
    pub fn failing(e: EngineError) -> Rc<Self> {
        Rc::new(Self { replies: RefCell::new(vec![Err(e); 8].into()), calls: RefCell::default() })
    }
}

struct Scripted {
    id: EngineId,
    script: Rc<Script>,
}

impl Engine for Scripted {
    fn id(&self) -> EngineId {
        self.id
    }
    fn is_free(&self) -> bool {
        true
    }
    fn call<'a>(&'a self, call: &'a EngineCall) -> EngineFuture<'a> {
        self.script.calls.borrow_mut().push(call.clone());
        let next = self.script.replies.borrow_mut().pop_front();
        Box::pin(async move {
            next.unwrap_or_else(|| Err(EngineError::Unavailable { reason: "no more replies".into() }))
        })
    }
}

/// The app's router with one scripted engine as `id`.
pub fn router(id: EngineId, script: Rc<Script>) -> Ai {
    let r = Router::new(RouteTable::builtin(), PromptSet::builtin(), Box::new(MemoryCache::default()))
        .with_engine(Box::new(Scripted { id, script }));
    Ai(Rc::new(r))
}

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
}

fn harness(case: Case) -> Element {
    crate::persistence::use_provide_saves();
    let db = use_context_provider(|| Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap()))));
    use_context_provider(|| {
        let mut settings = Settings { ui_language: case.lang, ..Settings::default() };
        (case.settings)(&mut settings);
        Ui {
            settings: Signal::new(settings),
            width: Signal::new(case.width),
            tablet: Signal::new(case.tablet),
            screen: Signal::new(Screen::Reader),
            lesson: Signal::new(None),
            lesson_from: Signal::new(Screen::Journey),
            focus: Signal::new(false),
        }
    });
    let st = use_context_provider(|| ReaderState::new(case.lang));
    use_context_provider(|| Inbox(Signal::new(None)));
    use_context_provider(|| TeachInbox(Signal::new(None)));
    use_context_provider(crate::play::PlayNav::new);
    use_context_provider(|| crate::coach::Coach::load(&db));
    use_context_provider(|| crate::settings::device::DevicePrefs::load(&db));
    use_context_provider(|| {
        let device_id = db.0.borrow().device_id().clone();
        let mut status = CloudStatus::not_configured(device_id);
        if case.signed_in {
            status.configured = true;
            status.email = Some("learner@example.com".into());
        }
        Cloud {
            status: Signal::new(status),
            devices: Signal::new(None),
            usage: Signal::new(None),
            signing_in: Signal::new(false),
            revision: Signal::new(0),
            commands: Signal::new(None),
            push: Signal::new(false),
            push_sent: Signal::new(None),
        }
    });
    if let Some(script) = case.ai.clone() {
        use_context_provider(|| router(case.engine, script));
    }
    crate::annotated::use_words(&db);
    use_hook(|| (case.prep)(&db, st));
    (case.body)()
}

/// A rendered screen that can be driven and re-rendered.
pub struct Screenful {
    pub dom: VirtualDom,
    rt: tokio::runtime::Runtime,
}

impl Screenful {
    pub fn new(case: Case) -> Self {
        let mut dom = VirtualDom::new_with_props(harness, case);
        dom.rebuild_in_place();
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        Self { dom, rt }
    }

    /// The HTML as it stands now.
    pub fn html(&self) -> String {
        dioxus::ssr::render(&self.dom)
    }

    /// Runs spawned tasks, effects and resources until nothing is left to
    /// do (or a short wait passes), then re-renders.
    pub fn settle(&mut self) -> String {
        for _ in 0..50 {
            let dom = &mut self.dom;
            let worked = self.rt.block_on(async {
                tokio::time::timeout(Duration::from_millis(30), dom.wait_for_work()).await.is_ok()
            });
            self.dom.render_immediate(&mut dioxus::core::NoOpMutations);
            if !worked {
                break;
            }
        }
        self.html()
    }

    /// Runs `f` with the harness's contexts, as an event handler would.
    pub fn act<R>(&mut self, f: impl FnOnce(&Db, ReaderState) -> R) -> R {
        let out =
            self.dom.in_scope(ScopeId::APP, || f(&consume_context::<Db>(), consume_context::<ReaderState>()));
        self.dom.render_immediate(&mut dioxus::core::NoOpMutations);
        out
    }

    pub fn ctx<T: Clone + 'static>(&self) -> T {
        self.dom.in_scope(ScopeId::APP, consume_context::<T>)
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
