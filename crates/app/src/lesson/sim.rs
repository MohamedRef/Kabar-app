//! A small test driver for rendered screens: finds elements in a live
//! `VirtualDom` by tag, attribute or text and fires their real event
//! handlers (click, input, submit), so tests exercise what a tap does
//! rather than a copy of the logic. Rendering for assertions is Dioxus SSR.
//!
//! Only handlers on the element itself run (no bubbling), which is how the
//! screens attach them anyway: the button, input or card that is tapped.
#![allow(dead_code)] // a kit: helpers land before every screen's tests use them

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
    use std::cell::RefCell;
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
