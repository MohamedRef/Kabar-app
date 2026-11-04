//! Drives real components in tests: the production contexts on an in-memory
//! store, a scripted AI engine, a fake WebView `document` that records every
//! `eval` and lets a test answer it, and helpers that find a rendered button
//! or field by its text and fire the real event handler on it.
//!
//! SSR alone never runs `onclick`; this does, so a test can click "Send" and
//! check the stored row and the HTML that follows.
//!
//! This is the one test kit: [`crate::test_dom`] is its DOM driver (real
//! mutations, real event bubbling), [`FakeDoc`] its WebView and [`provide`]
//! its contexts. `test_dom::App`, `lesson::sim` and `test_screens` are thin
//! front ends over the same three pieces, in the styles older tests use.

use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use dioxus::document::{Document, Eval, EvalError, Evaluator};
use dioxus::html::{SerializedFormData, SerializedKeyboardData, SerializedMouseData};
use dioxus::prelude::*;
use dioxus::signals::Owner;
use kabar_core::ai::{EngineFuture, MemoryCache, PromptSet, RouteTable, Router};
use kabar_core::locale::UiLanguage;
use kabar_core::settings::Settings;
use kabar_protocol::ai::{EngineCall, EngineError, EngineId, EngineReply};

use crate::cloud::link::CloudLink;
use crate::coach::Coach;
use crate::store::Store;
use crate::teach::{Ai, TeachInbox};
use crate::test_dom::{El, TestDom};
use crate::{Db, Screen, Ui};
use kabar_core::layout::WidthClass;

// ---------------------------------------------------------------- AI engine

/// Replies queued per test; every call is recorded.
#[derive(Default)]
pub struct Script {
    pub replies: RefCell<VecDeque<Result<EngineReply, EngineError>>>,
    pub calls: RefCell<Vec<EngineCall>>,
}

#[derive(Clone)]
pub struct Scripted {
    pub id: EngineId,
    pub script: Rc<Script>,
}

impl kabar_core::ai::Engine for Scripted {
    fn id(&self) -> EngineId {
        self.id
    }
    fn is_free(&self) -> bool {
        true
    }
    fn call<'a>(&'a self, call: &'a EngineCall) -> EngineFuture<'a> {
        self.script.calls.borrow_mut().push(call.clone());
        let next = self
            .script
            .replies
            .borrow_mut()
            .pop_front()
            .unwrap_or(Err(EngineError::Unavailable { reason: "nothing scripted".into() }));
        Box::pin(std::future::ready(next))
    }
}

/// A recorded reply from `fixtures/ai/`.
pub fn fixture(name: &str) -> EngineReply {
    let path = format!("{}/../../fixtures/ai/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

/// A router whose engines are the scripted ones (an empty list answers
/// every task "not available", like a device without Kabar Cloud).
pub fn router(engines: &[Scripted]) -> Router {
    let mut r = Router::new(RouteTable::builtin(), PromptSet::builtin(), Box::new(MemoryCache::default()));
    for e in engines {
        r = r.with_engine(Box::new(e.clone()));
    }
    r
}

// ----------------------------------------------------------------- document

/// What scripts answer right away, from their source (`dioxus.send`).
type Replies = Box<dyn FnMut(&str) -> Vec<serde_json::Value>>;

/// Messages waiting for Rust on one script, and who waits for them. Scripts
/// opened with [`FakeDoc::live`] share one per pattern.
#[derive(Default)]
struct Channel {
    queue: RefCell<VecDeque<serde_json::Value>>,
    wakers: RefCell<Vec<Waker>>,
}

impl Channel {
    fn push(&self, v: serde_json::Value) {
        self.queue.borrow_mut().push_back(v);
        for w in self.wakers.borrow_mut().drain(..) {
            w.wake();
        }
    }
}

/// One `eval`: its script, what Rust sent to it, and what "JS" will answer.
pub struct FakeEval {
    pub script: String,
    pub sent: RefCell<Vec<serde_json::Value>>,
    inbox: Rc<Channel>,
    /// Waits for more messages once its replies are read (a key listener, a
    /// timer, the audio engine); otherwise it finishes, like a one-shot script.
    open: bool,
}

impl FakeEval {
    /// Queues a message from JS (`dioxus.send(..)`).
    pub fn reply(&self, v: serde_json::Value) {
        self.inbox.push(v);
    }
}

struct Handle(Rc<FakeEval>);

impl Evaluator for Handle {
    fn send(&self, data: serde_json::Value) -> Result<(), EvalError> {
        self.0.sent.borrow_mut().push(data);
        Ok(())
    }
    fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Result<serde_json::Value, EvalError>> {
        match self.0.inbox.queue.borrow_mut().pop_front() {
            Some(v) => Poll::Ready(Ok(v)),
            None if self.0.open => {
                self.0.inbox.wakers.borrow_mut().push(cx.waker().clone());
                Poll::Pending
            }
            None => Poll::Ready(Err(EvalError::Finished)),
        }
    }
    fn poll_join(&mut self, _: &mut Context<'_>) -> Poll<Result<serde_json::Value, EvalError>> {
        Poll::Ready(Ok(serde_json::Value::Null))
    }
}

/// The one WebView stand-in for `document::eval` in tests. It keeps every
/// script; answers each with what [`reply_to`](Self::reply_to) or
/// [`on_eval`](Self::on_eval) scripted (nothing by default, as when no
/// voice exists); and keeps [`live`](Self::live) scripts (or every script,
/// with [`open`](Self::open)) waiting for a test to send them more.
pub struct FakeDoc {
    pub evals: RefCell<Vec<Rc<FakeEval>>>,
    replies: RefCell<Replies>,
    live: RefCell<Vec<(&'static str, Rc<Channel>)>>,
    all_open: bool,
    owners: RefCell<Vec<Owner>>,
}

impl FakeDoc {
    fn build(all_open: bool) -> Rc<Self> {
        Rc::new(Self {
            evals: RefCell::default(),
            replies: RefCell::new(Box::new(|_| Vec::new())),
            live: RefCell::default(),
            all_open,
            owners: RefCell::default(),
        })
    }

    /// Scripts finish once their scripted replies are read.
    pub fn new() -> Rc<Self> {
        Self::build(false)
    }

    /// Every script stays open, waiting for [`FakeEval::reply`].
    pub fn open() -> Rc<Self> {
        Self::build(true)
    }

    /// Answers scripts containing `needle` with `values`; others as before.
    pub fn reply_to(&self, needle: &'static str, values: Vec<serde_json::Value>) {
        let mut previous = std::mem::replace(&mut *self.replies.borrow_mut(), Box::new(|_| Vec::new()));
        *self.replies.borrow_mut() =
            Box::new(move |js| if js.contains(needle) { values.clone() } else { previous(js) });
    }

    /// Answers every script with what `f` returns for its source.
    pub fn on_eval(&self, f: impl FnMut(&str) -> Vec<serde_json::Value> + 'static) {
        *self.replies.borrow_mut() = Box::new(f);
    }

    /// Scripts containing `pattern` stay open and share one inbox, fed by
    /// [`send`](Self::send).
    pub fn live(&self, pattern: &'static str) {
        self.live.borrow_mut().push((pattern, Rc::default()));
    }

    /// The live scripts with `pattern` get `value` (a key press, a timer
    /// tick, a drop on the wall).
    pub fn send(&self, pattern: &str, value: serde_json::Value) {
        let live = self.live.borrow();
        let (_, channel) = live.iter().find(|(p, _)| *p == pattern).expect("not a live script");
        channel.push(value);
    }

    /// The newest eval whose script contains `needle`.
    pub fn find(&self, needle: &str) -> Option<Rc<FakeEval>> {
        self.evals.borrow().iter().rev().find(|e| e.script.contains(needle)).cloned()
    }

    /// Every script run so far.
    pub fn scripts(&self) -> Vec<String> {
        self.evals.borrow().iter().map(|e| e.script.clone()).collect()
    }

    /// How many scripts so far contain `needle`.
    pub fn ran(&self, needle: &str) -> usize {
        self.evals.borrow().iter().filter(|e| e.script.contains(needle)).count()
    }
}

impl Document for FakeDoc {
    fn eval(&self, js: String) -> Eval {
        let live = self.live.borrow().iter().find(|(p, _)| js.contains(p)).map(|(_, c)| c.clone());
        let (inbox, open) = match live {
            Some(c) => (c, true),
            None => {
                let c = Rc::new(Channel::default());
                c.queue.borrow_mut().extend((self.replies.borrow_mut())(&js));
                (c, self.all_open)
            }
        };
        let e = Rc::new(FakeEval { script: js, sent: RefCell::default(), inbox, open });
        self.evals.borrow_mut().push(e.clone());
        let owner = Owner::default();
        let boxed = owner.insert(Box::new(Handle(e)) as Box<dyn Evaluator>);
        self.owners.borrow_mut().push(owner);
        Eval::new(boxed)
    }
}

// ----------------------------------------------------------------- contexts

/// What a test sets up before the first render.
pub struct Setup {
    pub lang: UiLanguage,
    /// Settings other than the language (which `lang` sets).
    pub settings: Settings,
    pub width: WidthClass,
    /// The screen `Ui` starts on.
    pub screen: Screen,
    pub engines: Vec<Scripted>,
    /// The store to open (an empty in-memory one when neither this nor
    /// `db` is set).
    pub store: Option<Store>,
    /// A store the test keeps a handle to.
    pub db: Option<Db>,
    /// The WebView (a fresh [`FakeDoc::new`] when unset).
    pub doc: Option<Rc<FakeDoc>>,
    pub audio_dir: Option<std::path::PathBuf>,
    /// A signed-in Kabar Cloud on localhost ([`fake_cloud`]): blobs upload,
    /// TTS answers. Off: the link is not configured, as in a fresh build.
    pub cloud: bool,
    /// The app's Kabar Cloud status (`None`: no `Cloud` context, as in a
    /// build without the account screen).
    pub sync: Option<crate::cloud::SyncState>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            lang: UiLanguage::En,
            settings: Settings::default(),
            width: WidthClass::Compact,
            screen: Screen::Today,
            engines: Vec::new(),
            store: None,
            db: None,
            doc: None,
            audio_dir: None,
            cloud: false,
            sync: None,
        }
    }
}

/// A fresh in-memory store.
pub fn memory_db() -> Db {
    Db(Rc::new(RefCell::new(Store::open_in_memory().unwrap())))
}

/// Provides the app's shared contexts (the WebView, store, settings, coach,
/// navigation, AI router, Kabar Cloud link, reader and word index) from
/// `setup`, read on the first render. Call it at the top of a test's root
/// component; it returns the `Ui` and `Db` it provided.
pub fn provide(setup: impl FnOnce() -> Setup) -> (Ui, Db) {
    let first = use_hook(|| Rc::new(RefCell::new(Some(setup()))));
    // Read once: which contexts exist must not change between renders.
    let (lang, width, screen, cloud, has_sync, has_audio) = use_hook(|| {
        let s = first.borrow();
        let s = s.as_ref().expect("the first render");
        (s.lang, s.width, s.screen, s.cloud, s.sync.is_some(), s.audio_dir.is_some())
    });
    let setup = || std::cell::RefMut::map(first.borrow_mut(), |s| s.as_mut().expect("the first render"));
    use_context_provider(|| setup().doc.take().unwrap_or_else(FakeDoc::new) as Rc<dyn Document>);
    crate::persistence::use_provide_saves();
    let db = use_context_provider(|| {
        let mut s = setup();
        s.db.take().unwrap_or_else(|| {
            Db(Rc::new(RefCell::new(s.store.take().unwrap_or_else(|| Store::open_in_memory().unwrap()))))
        })
    });
    let ui = use_context_provider(|| Ui {
        settings: Signal::new(Settings { ui_language: lang, ..setup().settings.clone() }),
        width: Signal::new(width),
        tablet: Signal::new(false),
        screen: Signal::new(screen),
        lesson: Signal::new(None),
        lesson_from: Signal::new(Screen::Journey),
        focus: Signal::new(false),
    });
    if has_sync {
        use_context_provider(|| {
            let sync = setup().sync.take().unwrap();
            let device_id = db.0.borrow().device_id().clone();
            crate::account::Cloud {
                status: Signal::new(crate::cloud::CloudStatus {
                    sync,
                    ..crate::cloud::CloudStatus::not_configured(device_id)
                }),
                devices: Signal::new(None),
                usage: Signal::new(None),
                signing_in: Signal::new(false),
                login: Signal::new(Default::default()),
                revision: Signal::new(0),
                commands: Signal::new(None),
                push: Signal::new(false),
                push_sent: Signal::new(None),
            }
        });
    }
    let link = use_context_provider(CloudLink::default);
    use_hook(|| {
        if cloud {
            let base = fake_cloud();
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            link.connect(tx);
            spawn(async move {
                while let Some(cmd) = rx.recv().await {
                    if let crate::cloud::Command::Access { reply, .. } = cmd {
                        reply.send(Some((crate::cloud::api::Api::new(&base), "test-token".into()))).ok();
                    }
                }
            });
        }
    });
    use_context_provider(|| Ai(Rc::new(router(&setup().engines))));
    use_context_provider(|| TeachInbox(Signal::new(None)));
    use_context_provider(|| crate::settings::device::DevicePrefs::load(&db));
    use_context_provider(crate::settings::SettingsNav::new);
    use_context_provider(crate::play::PlayNav::new);
    use_context_provider(|| Coach::load(&db));
    use_context_provider(|| crate::reader::Inbox(Signal::new(None)));
    use_context_provider(|| crate::reader::ReaderState::new(lang));
    if has_audio {
        use_context_provider(|| {
            crate::audio::AudioCache::new(setup().audio_dir.take().unwrap(), link.clone())
        });
    }
    crate::annotated::use_words(&db);
    (ui, db)
}

#[derive(Clone)]
struct RootProps {
    body: fn() -> Element,
    setup: Rc<RefCell<Option<Setup>>>,
}

impl PartialEq for RootProps {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

fn root(p: RootProps) -> Element {
    provide(|| p.setup.borrow_mut().take().expect("set up once"));
    (p.body)()
}

/// A VirtualDom showing `body` inside the contexts `setup` describes.
pub fn mount(body: fn() -> Element, setup: Setup) -> VirtualDom {
    VirtualDom::new_with_props(root, RootProps { body, setup: Rc::new(RefCell::new(Some(setup))) })
}

/// A mounted component tree with the app's contexts around `root`, driven
/// by labels: what a learner reads on a button or field.
pub struct App {
    pub dom: TestDom,
    pub doc: Rc<FakeDoc>,
}

impl App {
    pub fn new(root: fn() -> Element, mut setup: Setup) -> Self {
        let doc = setup.doc.get_or_insert_with(FakeDoc::open).clone();
        let mut app = Self { dom: TestDom::unsettled(mount(root, setup)), doc };
        app.settle();
        app
    }

    /// Runs ready tasks and re-renders until nothing is left to do now.
    pub fn settle(&mut self) {
        assert!(self.dom.run_until_quiet(std::time::Duration::from_millis(25), 200), "the app never settled");
    }

    pub fn html(&self) -> String {
        self.dom.ssr()
    }

    /// Runs `f` inside the app's scope (to read contexts).
    pub fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        self.dom.peek_app(f)
    }

    pub fn db(&self) -> Db {
        self.with(consume_context::<Db>)
    }

    pub fn ui(&self) -> Ui {
        self.with(consume_context::<Ui>)
    }

    /// Every element with a listener for `event`.
    pub fn targets(&self, event: &str) -> Vec<Target> {
        self.dom
            .nodes()
            .into_iter()
            .filter(|n| n.listens(event))
            .map(|n| Target { el: n.el, text: n.text, attrs: n.attrs })
            .collect()
    }

    fn find(&self, event: &str, label: &str) -> Target {
        let all = self.targets(event);
        all.iter().find(|t| t.matches(label)).cloned().unwrap_or_else(|| {
            panic!(
                "no {event} target labelled {label:?}; have: {:#?}",
                all.iter().map(|t| (&t.text, &t.attrs)).collect::<Vec<_>>()
            )
        })
    }

    /// Whether a `click` target labelled `label` is on screen.
    pub fn has(&self, event: &str, label: &str) -> bool {
        self.targets(event).iter().any(|t| t.matches(label))
    }

    fn fire(&mut self, event: &str, el: El, data: Box<dyn Any>) {
        self.dom.dispatch(el, event, data, true);
        self.settle();
    }

    /// Clicks the element whose text, `aria-label`, `title` or class holds `label`.
    pub fn click(&mut self, label: &str) {
        let t = self.find("click", label);
        self.fire("click", t.el, Box::new(SerializedMouseData::default()));
    }

    /// Clicks the `n`th element (0-based) matching `label`.
    pub fn click_nth(&mut self, label: &str, n: usize) {
        let all: Vec<_> = self.targets("click").into_iter().filter(|t| t.matches(label)).collect();
        let t = all.get(n).unwrap_or_else(|| panic!("only {} click targets match {label:?}", all.len()));
        self.fire("click", t.el, Box::new(SerializedMouseData::default()));
    }

    fn form(value: &str) -> Box<dyn Any> {
        Box::new(SerializedFormData::new(value.to_string(), Vec::new()))
    }

    /// Types `value` into the field matching `label` (placeholder, aria-label…).
    pub fn input(&mut self, label: &str, value: &str) {
        let t = self.find("input", label);
        self.fire("input", t.el, Self::form(value));
    }

    pub fn change(&mut self, label: &str, value: &str) {
        let t = self.find("change", label);
        self.fire("change", t.el, Self::form(value));
    }

    pub fn submit(&mut self, label: &str) {
        let t = self.find("submit", label);
        self.fire("submit", t.el, Self::form(""));
    }

    pub fn key(&mut self, label: &str, key: Key, shift: bool) {
        let t = self.find("keydown", label);
        let mods = if shift { Modifiers::SHIFT } else { Modifiers::empty() };
        let data = SerializedKeyboardData::new(key, Code::Enter, Location::Standard, false, mods, false);
        self.fire("keydown", t.el, Box::new(data));
    }
}

/// An element with a listener, described by what a learner sees on it.
#[derive(Clone, Debug)]
pub struct Target {
    pub el: El,
    pub text: String,
    pub attrs: Vec<(String, String)>,
}

impl Target {
    fn matches(&self, label: &str) -> bool {
        self.text.contains(label)
            || self.attrs.iter().any(|(k, v)| {
                matches!(
                    k.as_str(),
                    "aria-label" | "aria_label" | "title" | "class" | "placeholder" | "value"
                ) && v.contains(label)
            })
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name || k.replace('_', "-") == name).map(|(_, v)| v.as_str())
    }
}

/// The HTML with tags stripped, whitespace collapsed: what a reader sees.
pub fn visible(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut in_style = false;
    let lower = html.to_string();
    let mut i = 0;
    let bytes = lower.as_bytes();
    while i < bytes.len() {
        if lower[i..].starts_with("<style") {
            in_style = true;
        }
        if in_style && lower[i..].starts_with("</style>") {
            in_style = false;
            i += "</style>".len();
            continue;
        }
        let c = lower[i..].chars().next().unwrap();
        if !in_style {
            match c {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    out.push(' ');
                }
                _ if !in_tag => out.push(c),
                _ => {}
            }
        }
        i += c.len_utf8();
    }
    let out = out
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    out.replace("&amp;", "&").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Checks a feature's string catalogue: every line is there in all three
/// languages, English has no Arabic letters, both Arabic voices are written
/// in Arabic (except `latin`, lines that are Latin on purpose), and Egyptian
/// differs from English (nothing was left untranslated).
pub fn check_catalogue<T: Copy + std::fmt::Debug + PartialEq>(
    all: &[T],
    tr: fn(UiLanguage, T) -> &'static str,
    latin: &[T],
) {
    let arabic = |s: &str| s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
    for &m in all {
        let (eg, ar, en) = (tr(UiLanguage::ArEg, m), tr(UiLanguage::Ar, m), tr(UiLanguage::En, m));
        for s in [eg, ar, en] {
            assert!(!s.trim().is_empty(), "{m:?} is empty");
        }
        assert!(!arabic(en), "{m:?}: English has Arabic letters: {en}");
        if !latin.contains(&m) {
            assert!(arabic(eg) && arabic(ar), "{m:?} is not in Arabic: {eg} / {ar}");
            assert_ne!(eg, en, "{m:?} is untranslated");
        }
    }
}

/// A Kabar Cloud stand-in on 127.0.0.1: every request gets `200 OK` (an
/// empty body, or a few bytes of "audio" for `/v1/tts`). Returns its base URL.
pub fn fake_cloud() -> String {
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let mut length = 0usize;
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body).ok();
            let answer: &[u8] = if line.contains("/v1/tts") { b"ID3fake-mp3" } else { b"" };
            let mut stream = reader.into_inner();
            let head =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", answer.len());
            stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(answer)).ok();
        }
    });
    url
}

/// A scripted engine reply holding `json` as the model's text.
pub fn reply(json: serde_json::Value) -> EngineReply {
    EngineReply { engine: EngineId::WorkersAi, text: json.to_string(), model: "test".into() }
}
