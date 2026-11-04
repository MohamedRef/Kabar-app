//! A small test driver for rendered screens: finds elements in a live
//! `VirtualDom` by tag, attribute or text and fires their real event
//! handlers (click, input, submit), so tests exercise what a tap does
//! rather than a copy of the logic. Rendering for assertions is Dioxus SSR.
//!
//! Only handlers on the element itself run (no bubbling), which is how the
//! screens attach them anyway: the button, input or card that is tapped.

use std::any::Any;
use std::rc::Rc;

use dioxus::core::{AttributeValue, DynamicNode, ListenerCallback, TemplateAttribute, TemplateNode, VNode};
use dioxus::html::{
    PlatformEventData, SerializedFormData, SerializedHtmlEventConverter, SerializedMouseData,
};
use dioxus::prelude::*;

/// One rendered element: its attributes (static and dynamic), the text of
/// everything under it, and its listeners.
#[derive(Clone)]
pub(crate) struct Node {
    pub tag: &'static str,
    pub attrs: Vec<(&'static str, String)>,
    pub text: String,
    /// The class attributes of its ancestors, outermost first.
    pub within: Vec<String>,
    listeners: Vec<(&'static str, ListenerCallback)>,
}

impl Node {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| *n == name).map(|(_, v)| v.as_str())
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.attr("class").is_some_and(|c| c.split_whitespace().any(|c| c == class))
    }

    /// Whether an ancestor has `class`.
    pub fn inside(&self, class: &str) -> bool {
        self.within.iter().any(|c| c.split_whitespace().any(|c| c == class))
    }

    pub fn disabled(&self) -> bool {
        self.attr("disabled").is_some_and(|v| v == "true")
    }

    pub fn listens(&self, event: &str) -> bool {
        self.listeners.iter().any(|(n, _)| n.trim_start_matches("on") == event)
    }
}

/// A screen under test.
pub(crate) struct Sim {
    pub dom: VirtualDom,
    /// Timers (typing pauses, debounced saves) run on tokio, as in the app.
    rt: tokio::runtime::Runtime,
}

impl Sim {
    pub fn new(app: fn() -> Element) -> Self {
        dioxus::html::set_event_converter(Box::new(SerializedHtmlEventConverter));
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        let mut dom = VirtualDom::new(app);
        {
            let _rt = rt.enter();
            dom.rebuild_in_place();
        }
        let mut sim = Sim { dom, rt };
        sim.settle();
        sim
    }

    /// Runs ready tasks, effects and re-renders until nothing is left to do
    /// (tasks waiting on a timer or the WebView stay parked).
    pub fn settle(&mut self) {
        let _rt = self.rt.enter();
        for _ in 0..20 {
            self.dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
    }

    /// Lets `ms` of real time pass (timers fire), then settles.
    pub fn wait(&mut self, ms: u64) {
        self.rt.block_on(async { tokio::time::sleep(std::time::Duration::from_millis(ms)).await });
        self.settle();
    }

    /// The rendered page without its `<style>` blocks.
    pub fn html(&mut self) -> String {
        self.settle();
        let mut html = dioxus::ssr::render(&self.dom);
        while let Some(start) = html.find("<style>") {
            let end = html[start..].find("</style>").map_or(html.len(), |e| start + e + "</style>".len());
            html.replace_range(start..end, "");
        }
        html
    }

    /// Runs `f` inside the app's root scope (to read or set context state).
    pub fn with<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let out = {
            let _rt = self.rt.enter();
            self.dom.in_scope(ScopeId::APP, f)
        };
        self.settle();
        out
    }

    pub fn nodes(&self) -> Vec<Node> {
        let mut out = Vec::new();
        walk_vnode(&self.dom, self.dom.base_scope().root_node(), &mut Vec::new(), &mut out);
        out
    }

    pub fn find(&self, pick: impl Fn(&Node) -> bool) -> Option<Node> {
        self.nodes().into_iter().find(|n| pick(n))
    }

    pub fn count(&self, pick: impl Fn(&Node) -> bool) -> usize {
        self.nodes().iter().filter(|n| pick(n)).count()
    }

    fn fire(&mut self, event: &str, pick: &dyn Fn(&Node) -> bool, data: impl Fn() -> PlatformEventData) {
        let node = self.nodes().into_iter().find(|n| n.listens(event) && pick(n)).unwrap_or_else(|| {
            let seen: Vec<String> = self
                .nodes()
                .iter()
                .filter(|n| n.listens(event))
                .map(|n| format!("<{} class={:?}> {:?}", n.tag, n.attr("class").unwrap_or(""), n.text))
                .collect();
            panic!("no element listening to {event} matches; listening:\n{}", seen.join("\n"))
        });
        assert!(!node.disabled(), "{event} on a disabled <{}>: {:?}", node.tag, node.text);
        for (name, cb) in &node.listeners {
            if name.trim_start_matches("on") == event {
                let data: Rc<dyn Any> = Rc::new(data());
                let _rt = self.rt.enter();
                self.dom.in_runtime(|| cb.call(Event::new(data, true)));
            }
        }
        self.settle();
    }

    /// Clicks the first enabled-or-not element listening to clicks that
    /// matches (a disabled match fails the test: a user can't tap it).
    pub fn click(&mut self, pick: impl Fn(&Node) -> bool) {
        self.fire("click", &pick, || PlatformEventData::new(Box::new(SerializedMouseData::default())));
    }

    /// Clicks the button whose text contains `text`.
    pub fn click_text(&mut self, text: &str) {
        self.click(|n| n.tag == "button" && n.text.contains(text));
    }

    /// Types `value` into the first matching input (an `input` event).
    pub fn input(&mut self, pick: impl Fn(&Node) -> bool, value: &str) {
        let value = value.to_string();
        self.fire("input", &pick, move || {
            PlatformEventData::new(Box::new(SerializedFormData::new(value.clone(), vec![])))
        });
    }

    /// Submits the first matching form.
    pub fn submit(&mut self, pick: impl Fn(&Node) -> bool) {
        self.fire("submit", &pick, || {
            PlatformEventData::new(Box::new(SerializedFormData::new(String::new(), vec![])))
        });
    }
}

fn walk_vnode(dom: &VirtualDom, vnode: &VNode, up: &mut Vec<String>, out: &mut Vec<Node>) -> String {
    let mut text = String::new();
    for root in vnode.template.roots {
        text += &walk_template(dom, vnode, root, up, out);
    }
    text
}

fn walk_dynamic(
    dom: &VirtualDom,
    vnode: &VNode,
    id: usize,
    up: &mut Vec<String>,
    out: &mut Vec<Node>,
) -> String {
    match &vnode.dynamic_nodes[id] {
        DynamicNode::Text(t) => t.value.clone(),
        DynamicNode::Fragment(children) => children.iter().map(|c| walk_vnode(dom, c, up, out)).collect(),
        DynamicNode::Component(c) => match c.mounted_scope(id, vnode, dom) {
            Some(scope) => walk_vnode(dom, scope.root_node(), up, out),
            None => String::new(),
        },
        DynamicNode::Placeholder(_) => String::new(),
    }
}

fn walk_template(
    dom: &VirtualDom,
    vnode: &VNode,
    node: &TemplateNode,
    up: &mut Vec<String>,
    out: &mut Vec<Node>,
) -> String {
    match node {
        TemplateNode::Text { text } => text.to_string(),
        TemplateNode::Dynamic { id } => walk_dynamic(dom, vnode, *id, up, out),
        TemplateNode::Element { tag, attrs, children, .. } => {
            let mut el = Node {
                tag,
                attrs: Vec::new(),
                text: String::new(),
                within: up.clone(),
                listeners: Vec::new(),
            };
            for a in attrs.iter() {
                match a {
                    TemplateAttribute::Static { name, value, .. } => el.attrs.push((name, value.to_string())),
                    TemplateAttribute::Dynamic { id } => {
                        for attr in vnode.dynamic_attrs[*id].iter() {
                            match &attr.value {
                                AttributeValue::Listener(cb) => el.listeners.push((attr.name, cb.clone())),
                                AttributeValue::Text(s) => el.attrs.push((attr.name, s.clone())),
                                AttributeValue::Bool(b) => el.attrs.push((attr.name, b.to_string())),
                                AttributeValue::Int(i) => el.attrs.push((attr.name, i.to_string())),
                                AttributeValue::Float(f) => el.attrs.push((attr.name, f.to_string())),
                                AttributeValue::Any(_) | AttributeValue::None => {}
                            }
                        }
                    }
                }
            }
            let class = el.attr("class").unwrap_or("").to_string();
            let at = out.len();
            out.push(el);
            up.push(class);
            let mut text = String::new();
            for child in children.iter() {
                text += &walk_template(dom, vnode, child, up, out);
            }
            up.pop();
            out[at].text = text.clone();
            text
        }
    }
}

/// The contexts every learning screen reads (settings, store, coach, Play
/// navigation, the word index and the Teach inbox), around an in-memory
/// store. Call at the top of a test app; returns the `Ui` and `Db`.
pub(crate) fn provide(width: kabar_core::layout::WidthClass) -> (crate::Ui, crate::Db) {
    use_page();
    crate::persistence::use_provide_saves();
    let db = use_context_provider(|| {
        crate::Db(Rc::new(RefCell::new(crate::store::Store::open_in_memory().unwrap())))
    });
    let ui = use_context_provider(|| crate::Ui {
        settings: Signal::new(kabar_core::settings::Settings::default()),
        width: Signal::new(width),
        tablet: Signal::new(false),
        screen: Signal::new(crate::Screen::Journey),
        lesson: Signal::new(None),
        lesson_from: Signal::new(crate::Screen::Journey),
        focus: Signal::new(false),
    });
    use_context_provider(|| crate::coach::Coach::load(&db));
    use_context_provider(crate::play::PlayNav::new);
    use_context_provider(|| crate::teach::TeachInbox(Signal::new(None)));
    crate::annotated::use_words(&db);
    (ui, db)
}

// ---------------------------------------------------------------------------
// A scripted page: what the WebView's JavaScript would send back
// ---------------------------------------------------------------------------

use dioxus::document::{Document, Eval, EvalError, Evaluator};
use std::cell::RefCell;
use std::collections::VecDeque;

/// Decides what a script sends back to Rust (`dioxus.send`), from its source.
type Replies = Box<dyn FnMut(&str) -> Vec<serde_json::Value>>;

thread_local! {
    static REPLIES: RefCell<Option<Replies>> = const { RefCell::new(None) };
    static SCRIPTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Scripts that stay open (key listeners, timers), by a piece of their
    /// source, with what the page has sent them and who waits for it.
    static LIVE: RefCell<Vec<Live>> = const { RefCell::new(Vec::new()) };
}

struct Live {
    pattern: &'static str,
    queue: Rc<RefCell<VecDeque<serde_json::Value>>>,
    wakers: Rc<RefCell<Vec<std::task::Waker>>>,
}

/// Every script run since the test app started.
pub(crate) fn scripts() -> Vec<String> {
    SCRIPTS.with_borrow(Clone::clone)
}

/// What scripts send back right away in the next test app (none by
/// default: as if the WebView answered nothing). Set before `Sim::new`.
pub(crate) fn on_eval(f: impl FnMut(&str) -> Vec<serde_json::Value> + 'static) {
    REPLIES.set(Some(Box::new(f)));
}

/// Scripts containing `pattern` stay open in the next test app, waiting for
/// [`Sim::page_sends`]. Set before `Sim::new`.
pub(crate) fn live(pattern: &'static str) {
    LIVE.with_borrow_mut(|l| l.push(Live { pattern, queue: Rc::default(), wakers: Rc::default() }));
}

impl Sim {
    /// The open script containing `pattern` sends `value` (a key press, a
    /// timer tick, a drop on the wall).
    pub fn page_sends(&mut self, pattern: &str, value: serde_json::Value) {
        let wakers = LIVE.with_borrow(|l| {
            let live = l.iter().find(|x| x.pattern == pattern).expect("not a live script");
            live.queue.borrow_mut().push_back(value);
            std::mem::take(&mut *live.wakers.borrow_mut())
        });
        for w in wakers {
            w.wake();
        }
        self.settle();
    }
}

struct Page {
    owners: RefCell<Vec<dioxus::signals::Owner>>,
}

enum Scripted {
    Once(VecDeque<serde_json::Value>),
    Open(Rc<RefCell<VecDeque<serde_json::Value>>>, Rc<RefCell<Vec<std::task::Waker>>>),
}

impl Evaluator for Scripted {
    fn send(&self, _: serde_json::Value) -> Result<(), EvalError> {
        Ok(())
    }
    fn poll_recv(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<serde_json::Value, EvalError>> {
        match self {
            Scripted::Once(q) => std::task::Poll::Ready(q.pop_front().ok_or(EvalError::Finished)),
            Scripted::Open(q, wakers) => match q.borrow_mut().pop_front() {
                Some(v) => std::task::Poll::Ready(Ok(v)),
                None => {
                    wakers.borrow_mut().push(cx.waker().clone());
                    std::task::Poll::Pending
                }
            },
        }
    }
    fn poll_join(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<serde_json::Value, EvalError>> {
        std::task::Poll::Ready(Ok(serde_json::Value::Null))
    }
}

impl Document for Page {
    fn eval(&self, js: String) -> Eval {
        let open = LIVE.with_borrow(|l| {
            l.iter()
                .find(|x| js.contains(x.pattern))
                .map(|x| Scripted::Open(x.queue.clone(), x.wakers.clone()))
        });
        let scripted = open.unwrap_or_else(|| {
            Scripted::Once(REPLIES.with_borrow_mut(|r| r.as_mut().map(|f| f(&js)).unwrap_or_default()).into())
        });
        SCRIPTS.with_borrow_mut(|s| s.push(js));
        let owner = dioxus::signals::Owner::default();
        let eval = Eval::new(owner.insert(Box::new(scripted) as Box<dyn Evaluator>));
        self.owners.borrow_mut().push(owner);
        eval
    }
}

/// Provides the scripted page as the document (`sim::provide` does this).
fn use_page() {
    SCRIPTS.with_borrow_mut(Vec::clear);
    use_context_provider(|| Rc::new(Page { owners: RefCell::default() }) as Rc<dyn Document>);
}
