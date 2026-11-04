//! Drives real components in tests: the production contexts on an in-memory
//! store, a scripted AI engine, a fake WebView `document` that records every
//! `eval` and lets a test answer it, and helpers that find a rendered button
//! or field by its text and fire the real event handler on it.
//!
//! SSR alone never runs `onclick`; this does, so a test can click "Send" and
//! check the stored row and the HTML that follows.

use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use dioxus::core::{AttributeValue, DynamicNode, ElementId, Event, TemplateAttribute, TemplateNode, VNode};
use dioxus::document::{Document, Eval, EvalError, Evaluator};
use dioxus::html::{
    PlatformEventData, SerializedFormData, SerializedHtmlEventConverter, SerializedKeyboardData,
    SerializedMouseData,
};
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

/// One `eval`: its script, what Rust sent to it, and what "JS" will answer.
pub struct FakeEval {
    pub script: String,
    pub sent: RefCell<Vec<serde_json::Value>>,
    inbox: RefCell<VecDeque<serde_json::Value>>,
    waker: RefCell<Option<Waker>>,
}

impl FakeEval {
    /// Queues a message from JS (`dioxus.send(..)`).
    pub fn reply(&self, v: serde_json::Value) {
        self.inbox.borrow_mut().push_back(v);
        if let Some(w) = self.waker.borrow_mut().take() {
            w.wake();
        }
    }
}

struct Handle(Rc<FakeEval>);

impl Evaluator for Handle {
    fn send(&self, data: serde_json::Value) -> Result<(), EvalError> {
        self.0.sent.borrow_mut().push(data);
        Ok(())
    }
    fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Result<serde_json::Value, EvalError>> {
        match self.0.inbox.borrow_mut().pop_front() {
            Some(v) => Poll::Ready(Ok(v)),
            None => {
                *self.0.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
    fn poll_join(&mut self, _: &mut Context<'_>) -> Poll<Result<serde_json::Value, EvalError>> {
        Poll::Ready(Ok(serde_json::Value::Null))
    }
}

/// A WebView stand-in: keeps every `eval` so tests can read the scripts and
/// answer the long-lived ones (the audio engine, the ink canvas).
#[derive(Default)]
pub struct FakeDoc {
    pub evals: RefCell<Vec<Rc<FakeEval>>>,
    owners: RefCell<Vec<Owner>>,
}

impl FakeDoc {
    /// The newest eval whose script contains `needle`.
    pub fn find(&self, needle: &str) -> Option<Rc<FakeEval>> {
        self.evals.borrow().iter().rev().find(|e| e.script.contains(needle)).cloned()
    }
    pub fn scripts(&self) -> Vec<String> {
        self.evals.borrow().iter().map(|e| e.script.clone()).collect()
    }
}

impl Document for FakeDoc {
    fn eval(&self, js: String) -> Eval {
        let e = Rc::new(FakeEval {
            script: js,
            sent: RefCell::default(),
            inbox: RefCell::default(),
            waker: RefCell::default(),
        });
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
    pub engines: Vec<Scripted>,
    pub store: Option<Store>,
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
            engines: Vec::new(),
            store: None,
            audio_dir: None,
            cloud: false,
            sync: None,
        }
    }
}

thread_local! {
    static SETUP: RefCell<Option<Setup>> = const { RefCell::new(None) };
    static ROOT: RefCell<Option<fn() -> Element>> = const { RefCell::new(None) };
    static DOC: RefCell<Option<Rc<FakeDoc>>> = const { RefCell::new(None) };
}

fn harness() -> Element {
    let (lang, settings, engines, audio_dir, cloud, sync) = use_hook(|| {
        let s = SETUP.with(|s| s.borrow_mut().take()).unwrap_or_default();
        Rc::new((s.lang, s.settings, s.engines, s.audio_dir, s.cloud, s.sync))
    })
    .as_ref()
    .clone();
    use_context_provider(|| DOC.with(|d| d.borrow().clone()).unwrap() as Rc<dyn Document>);
    crate::persistence::use_provide_saves();
    let db = use_context_provider(|| {
        let store =
            SETUP_STORE.with(|s| s.borrow_mut().take()).unwrap_or_else(|| Store::open_in_memory().unwrap());
        Db(Rc::new(RefCell::new(store)))
    });
    let ui = use_context_provider(|| Ui {
        settings: Signal::new(Settings { ui_language: lang, ..settings }),
        width: Signal::new(WidthClass::Compact),
        tablet: Signal::new(false),
        screen: Signal::new(Screen::Today),
        lesson: Signal::new(None),
        lesson_from: Signal::new(Screen::Journey),
        focus: Signal::new(false),
    });
    let _ = ui;
    if let Some(sync) = sync {
        use_context_provider(|| {
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
    use_context_provider(|| Ai(Rc::new(router(&engines))));
    use_context_provider(|| TeachInbox(Signal::new(None)));
    use_context_provider(|| Coach::load(&db));
    use_context_provider(|| crate::reader::Inbox(Signal::new(None)));
    use_context_provider(|| crate::reader::ReaderState::new(lang));
    if let Some(dir) = audio_dir {
        use_context_provider(|| crate::audio::AudioCache::new(dir, link.clone()));
    }
    crate::annotated::use_words(&db);
    let root = ROOT.with(|r| r.borrow().unwrap());
    rsx! { {root()} }
}

thread_local! {
    static SETUP_STORE: RefCell<Option<Store>> = const { RefCell::new(None) };
}

/// A mounted component tree with the app's contexts around `root`.
pub struct App {
    pub dom: VirtualDom,
    pub doc: Rc<FakeDoc>,
    /// File reads and the like run on tokio's blocking pool, as in the app.
    rt: tokio::runtime::Runtime,
}

impl App {
    pub fn new(root: fn() -> Element, mut setup: Setup) -> Self {
        dioxus::html::set_event_converter(Box::new(SerializedHtmlEventConverter));
        let doc = Rc::new(FakeDoc::default());
        SETUP_STORE.with(|s| *s.borrow_mut() = setup.store.take());
        SETUP.with(|s| *s.borrow_mut() = Some(setup));
        ROOT.with(|r| *r.borrow_mut() = Some(root));
        DOC.with(|d| *d.borrow_mut() = Some(doc.clone()));
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let mut dom = VirtualDom::new(harness);
        rt.block_on(async { dom.rebuild_in_place() });
        // Nothing may outlive the test in thread-locals (their owners drop late).
        DOC.with(|d| d.borrow_mut().take());
        let mut app = Self { dom, doc, rt };
        app.settle();
        app
    }

    /// Runs ready tasks and re-renders until nothing is left to do now.
    pub fn settle(&mut self) {
        for _ in 0..200 {
            let dom = &mut self.dom;
            let ready = self.rt.block_on(async {
                tokio::time::timeout(std::time::Duration::from_millis(25), dom.wait_for_work()).await.is_ok()
            });
            if !ready {
                return;
            }
            let dom = &mut self.dom;
            self.rt.block_on(async { dom.render_immediate(&mut dioxus::core::NoOpMutations) });
        }
        panic!("the app never settled");
    }

    pub fn html(&self) -> String {
        dioxus::ssr::render(&self.dom)
    }

    /// Runs `f` inside the app's scope (to read contexts).
    pub fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        self.dom.in_scope(ScopeId::APP, f)
    }

    pub fn db(&self) -> Db {
        self.with(consume_context::<Db>)
    }

    pub fn ui(&self) -> Ui {
        self.with(consume_context::<Ui>)
    }

    /// Every element with a listener for `event`.
    pub fn targets(&self, event: &str) -> Vec<Target> {
        let mut out = Vec::new();
        if let Some(scope) = self.dom.get_scope(ScopeId::APP) {
            walk(&self.dom, scope.root_node(), &mut out);
        }
        out.retain(|t| t.event == event);
        out
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

    fn fire(&mut self, event: &str, id: ElementId, data: Box<dyn Any>) {
        let ev = Event::new(Rc::new(PlatformEventData::new(data)) as Rc<dyn Any>, true);
        let runtime = self.dom.runtime();
        self.rt.block_on(async { runtime.handle_event(event, ev, id) });
        self.settle();
    }

    /// Clicks the element whose text, `aria-label`, `title` or class holds `label`.
    pub fn click(&mut self, label: &str) {
        let t = self.find("click", label);
        self.fire("click", t.id, Box::new(SerializedMouseData::default()));
    }

    /// Clicks the `n`th element (0-based) matching `label`.
    pub fn click_nth(&mut self, label: &str, n: usize) {
        let all: Vec<_> = self.targets("click").into_iter().filter(|t| t.matches(label)).collect();
        let t = all.get(n).unwrap_or_else(|| panic!("only {} click targets match {label:?}", all.len()));
        self.fire("click", t.id, Box::new(SerializedMouseData::default()));
    }

    fn form(value: &str) -> Box<dyn Any> {
        Box::new(SerializedFormData::new(value.to_string(), Vec::new()))
    }

    /// Types `value` into the field matching `label` (placeholder, aria-label…).
    pub fn input(&mut self, label: &str, value: &str) {
        let t = self.find("input", label);
        self.fire("input", t.id, Self::form(value));
    }

    pub fn change(&mut self, label: &str, value: &str) {
        let t = self.find("change", label);
        self.fire("change", t.id, Self::form(value));
    }

    pub fn submit(&mut self, label: &str) {
        let t = self.find("submit", label);
        self.fire("submit", t.id, Self::form(""));
    }

    pub fn key(&mut self, label: &str, key: Key, shift: bool) {
        let t = self.find("keydown", label);
        let mods = if shift { Modifiers::SHIFT } else { Modifiers::empty() };
        let data = SerializedKeyboardData::new(key, Code::Enter, Location::Standard, false, mods, false);
        self.fire("keydown", t.id, Box::new(data));
    }
}

/// An element with a listener, described by what a learner sees on it.
#[derive(Clone, Debug)]
pub struct Target {
    pub id: ElementId,
    pub event: String,
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

fn walk(dom: &VirtualDom, node: &VNode, out: &mut Vec<Target>) {
    for (i, root) in node.template.roots.iter().enumerate() {
        visit(dom, node, root, &mut vec![i as u8], out);
    }
}

fn visit(dom: &VirtualDom, vnode: &VNode, t: &TemplateNode, path: &mut Vec<u8>, out: &mut Vec<Target>) {
    match t {
        TemplateNode::Element { attrs, children, .. } => {
            let mut listed = Vec::new();
            let mut listeners = Vec::new();
            for a in attrs.iter() {
                match a {
                    TemplateAttribute::Static { name, value, .. } => {
                        listed.push((name.to_string(), value.to_string()))
                    }
                    TemplateAttribute::Dynamic { id } => {
                        for attr in vnode.dynamic_attrs[*id].iter() {
                            match &attr.value {
                                AttributeValue::Listener(_) => {
                                    if let Some(el) = vnode.mounted_dynamic_attribute(*id, dom) {
                                        listeners.push((attr.name.trim_start_matches("on").to_string(), el));
                                    }
                                }
                                AttributeValue::Text(v) => listed.push((attr.name.to_string(), v.clone())),
                                AttributeValue::Bool(b) => {
                                    listed.push((attr.name.to_string(), b.to_string()))
                                }
                                AttributeValue::Int(n) => listed.push((attr.name.to_string(), n.to_string())),
                                AttributeValue::Float(n) => {
                                    listed.push((attr.name.to_string(), n.to_string()))
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            if !listeners.is_empty() {
                let mut text = String::new();
                for c in children.iter() {
                    text_of(dom, vnode, c, &mut text);
                }
                for (event, id) in listeners {
                    out.push(Target { id, event, text: text.clone(), attrs: listed.clone() });
                }
            }
            for (j, c) in children.iter().enumerate() {
                path.push(j as u8);
                visit(dom, vnode, c, path, out);
                path.pop();
            }
        }
        TemplateNode::Text { .. } => {}
        TemplateNode::Dynamic { id } => match &vnode.dynamic_nodes[*id] {
            DynamicNode::Component(c) => {
                if let Some(scope) = c.mounted_scope(*id, vnode, dom) {
                    walk(dom, scope.root_node(), out);
                }
            }
            DynamicNode::Fragment(nodes) => nodes.iter().for_each(|n| walk(dom, n, out)),
            _ => {}
        },
    }
}

fn text_of(dom: &VirtualDom, vnode: &VNode, t: &TemplateNode, out: &mut String) {
    match t {
        TemplateNode::Element { children, .. } => children.iter().for_each(|c| text_of(dom, vnode, c, out)),
        TemplateNode::Text { text } => out.push_str(text),
        TemplateNode::Dynamic { id } => match &vnode.dynamic_nodes[*id] {
            DynamicNode::Text(v) => out.push_str(&v.value),
            DynamicNode::Fragment(nodes) => {
                for n in nodes {
                    n.template.roots.iter().for_each(|r| text_of(dom, n, r, out));
                }
            }
            DynamicNode::Component(c) => {
                if let Some(scope) = c.mounted_scope(*id, vnode, dom) {
                    let n = scope.root_node();
                    n.template.roots.iter().for_each(|r| text_of(dom, n, r, out));
                }
            }
            _ => {}
        },
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
