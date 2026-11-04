//! A small in-memory DOM for component tests: it applies the VirtualDom's
//! mutations, so a test can find a button by its text, click it, type into
//! a field, and read the screen again. SSR alone renders one frame and runs
//! no event handlers; this drives the real handlers the way a tap does.
//!
//! Only what the app's screens use is modelled: elements, text, attributes,
//! listeners and placeholders. Events carry default data (a click at 0,0, no
//! modifiers), which is all the handlers read.

use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::core::{AttributeValue, ElementId, Template, TemplateAttribute, TemplateNode, WriteMutations};
use dioxus::html::{PlatformEventData, SerializedHtmlEventConverter};
use dioxus::prelude::*;

use crate::testkit::{FakeDoc, Setup};

#[derive(Debug, Clone)]
enum Kind {
    Element { tag: String, attrs: Vec<(String, String)>, listeners: Vec<String> },
    Text(String),
    Placeholder,
}

#[derive(Debug, Clone)]
struct TreeNode {
    kind: Kind,
    parent: Option<usize>,
    children: Vec<usize>,
    id: Option<ElementId>,
}

/// HTML's boolean attributes: present or absent, never "false".
const BOOLEAN_ATTRIBUTES: &[&str] = &[
    "allowfullscreen",
    "allowpaymentrequest",
    "async",
    "autofocus",
    "autoplay",
    "checked",
    "controls",
    "default",
    "defer",
    "disabled",
    "formnovalidate",
    "hidden",
    "inert",
    "ismap",
    "itemscope",
    "loop",
    "multiple",
    "muted",
    "nomodule",
    "novalidate",
    "open",
    "playsinline",
    "readonly",
    "required",
    "reversed",
    "selected",
    "truespeed",
    "webkitdirectory",
];

/// The mutation target: an arena of nodes, the root at index 0.
#[derive(Default)]
struct Tree {
    nodes: Vec<TreeNode>,
    ids: HashMap<ElementId, usize>,
    stack: Vec<usize>,
}

impl Tree {
    fn new() -> Self {
        let mut t = Self::default();
        t.nodes.push(TreeNode {
            kind: Kind::Element { tag: "root".into(), attrs: Vec::new(), listeners: Vec::new() },
            parent: None,
            children: Vec::new(),
            id: Some(ElementId(0)),
        });
        t.ids.insert(ElementId(0), 0);
        t
    }

    fn add(&mut self, kind: Kind) -> usize {
        self.nodes.push(TreeNode { kind, parent: None, children: Vec::new(), id: None });
        self.nodes.len() - 1
    }

    fn build(&mut self, node: &TemplateNode) -> usize {
        match node {
            TemplateNode::Element { tag, attrs, children, .. } => {
                let attrs = attrs
                    .iter()
                    .filter_map(|a| match a {
                        TemplateAttribute::Static { name, value, .. } => {
                            Some((name.to_string(), value.to_string()))
                        }
                        TemplateAttribute::Dynamic { .. } => None,
                    })
                    .collect();
                let me = self.add(Kind::Element { tag: tag.to_string(), attrs, listeners: Vec::new() });
                for child in children.iter() {
                    let c = self.build(child);
                    self.attach(me, c, None);
                }
                me
            }
            TemplateNode::Text { text } => self.add(Kind::Text(text.to_string())),
            TemplateNode::Dynamic { .. } => self.add(Kind::Placeholder),
        }
    }

    fn assign(&mut self, node: usize, id: ElementId) {
        if let Some(old) = self.nodes[node].id
            && self.ids.get(&old) == Some(&node)
        {
            self.ids.remove(&old);
        }
        self.nodes[node].id = Some(id);
        self.ids.insert(id, node);
    }

    fn detach(&mut self, node: usize) {
        if let Some(p) = self.nodes[node].parent.take() {
            self.nodes[p].children.retain(|&c| c != node);
        }
    }

    /// Puts `child` under `parent`, at `at` or last.
    fn attach(&mut self, parent: usize, child: usize, at: Option<usize>) {
        self.detach(child);
        self.nodes[child].parent = Some(parent);
        match at {
            Some(i) => self.nodes[parent].children.insert(i, child),
            None => self.nodes[parent].children.push(child),
        }
    }

    fn pop(&mut self, m: usize) -> Vec<usize> {
        let at = self.stack.len() - m;
        self.stack.split_off(at)
    }

    fn node(&self, id: ElementId) -> usize {
        *self.ids.get(&id).unwrap_or_else(|| panic!("no node for {id:?}"))
    }

    fn at_path(&self, path: &[u8]) -> usize {
        let mut n = *self.stack.last().expect("empty stack");
        for &i in path {
            n = self.nodes[n].children[i as usize];
        }
        n
    }

    fn replace(&mut self, old: usize, new: Vec<usize>) {
        let parent = self.nodes[old].parent.expect("replacing a detached node");
        let i = self.nodes[parent].children.iter().position(|&c| c == old).unwrap();
        for (k, n) in new.into_iter().enumerate() {
            self.attach(parent, n, Some(i + k));
        }
        self.detach(old);
        self.forget(old);
    }

    fn insert_beside(&mut self, anchor: usize, new: Vec<usize>, after: bool) {
        let parent = self.nodes[anchor].parent.expect("inserting beside a detached node");
        for n in new.into_iter().rev() {
            let i = self.nodes[parent].children.iter().position(|&c| c == anchor).unwrap();
            self.attach(parent, n, Some(if after { i + 1 } else { i }));
        }
    }

    /// Drops the ids of a removed subtree, unless they were reused already.
    fn forget(&mut self, node: usize) {
        if let Some(id) = self.nodes[node].id
            && self.ids.get(&id) == Some(&node)
        {
            self.ids.remove(&id);
        }
        for c in self.nodes[node].children.clone() {
            self.forget(c);
        }
    }

    fn element(&mut self, node: usize) -> (&mut Vec<(String, String)>, &mut Vec<String>) {
        match &mut self.nodes[node].kind {
            Kind::Element { attrs, listeners, .. } => (attrs, listeners),
            k => panic!("not an element: {k:?}"),
        }
    }
}

impl WriteMutations for Tree {
    fn append_children(&mut self, id: ElementId, m: usize) {
        let parent = self.node(id);
        for c in self.pop(m) {
            self.attach(parent, c, None);
        }
    }

    fn assign_node_id(&mut self, path: &'static [u8], id: ElementId) {
        let n = self.at_path(path);
        self.assign(n, id);
    }

    fn create_placeholder(&mut self, id: ElementId) {
        let n = self.add(Kind::Placeholder);
        self.assign(n, id);
        self.stack.push(n);
    }

    fn create_text_node(&mut self, value: &str, id: ElementId) {
        let n = self.add(Kind::Text(value.to_string()));
        self.assign(n, id);
        self.stack.push(n);
    }

    fn load_template(&mut self, template: Template, index: usize, id: ElementId) {
        let n = self.build(&template.roots[index]);
        self.assign(n, id);
        self.stack.push(n);
    }

    fn replace_node_with(&mut self, id: ElementId, m: usize) {
        let new = self.pop(m);
        let old = self.node(id);
        self.replace(old, new);
    }

    fn replace_placeholder_with_nodes(&mut self, path: &'static [u8], m: usize) {
        let new = self.pop(m);
        let old = self.at_path(path);
        self.replace(old, new);
    }

    fn insert_nodes_after(&mut self, id: ElementId, m: usize) {
        let new = self.pop(m);
        let anchor = self.node(id);
        self.insert_beside(anchor, new, true);
    }

    fn insert_nodes_before(&mut self, id: ElementId, m: usize) {
        let new = self.pop(m);
        let anchor = self.node(id);
        self.insert_beside(anchor, new, false);
    }

    fn set_attribute(
        &mut self,
        name: &'static str,
        ns: Option<&'static str>,
        value: &AttributeValue,
        id: ElementId,
    ) {
        let n = self.node(id);
        let name = match ns {
            Some("style") => format!("style:{name}"),
            _ => name.to_string(),
        };
        let value = match value {
            AttributeValue::Text(s) => Some(s.clone()),
            AttributeValue::Float(f) => Some(f.to_string()),
            AttributeValue::Int(i) => Some(i.to_string()),
            AttributeValue::Bool(true) => Some("true".into()),
            // As the WebView's interpreter does: a false boolean attribute
            // (`disabled`, `checked`…) goes away, any other reads "false".
            AttributeValue::Bool(false) if BOOLEAN_ATTRIBUTES.contains(&name.as_str()) => None,
            AttributeValue::Bool(false) => Some("false".into()),
            AttributeValue::None => None,
            AttributeValue::Listener(_) | AttributeValue::Any(_) => return,
        };
        let (attrs, _) = self.element(n);
        attrs.retain(|(k, _)| *k != name);
        if let Some(v) = value {
            attrs.push((name, v));
        }
    }

    fn set_node_text(&mut self, value: &str, id: ElementId) {
        let n = self.node(id);
        self.nodes[n].kind = Kind::Text(value.to_string());
    }

    fn create_event_listener(&mut self, name: &'static str, id: ElementId) {
        let n = self.node(id);
        let (_, listeners) = self.element(n);
        if !listeners.iter().any(|l| l == name) {
            listeners.push(name.to_string());
        }
    }

    fn remove_event_listener(&mut self, name: &'static str, id: ElementId) {
        let n = self.node(id);
        let (_, listeners) = self.element(n);
        listeners.retain(|l| l != name);
    }

    fn remove_node(&mut self, id: ElementId) {
        let n = self.node(id);
        self.detach(n);
        self.forget(n);
    }

    fn push_root(&mut self, id: ElementId) {
        let n = self.node(id);
        self.stack.push(n);
    }
}

/// A mounted app plus its DOM. Tasks run on a tokio runtime of its own, as
/// in the app (timers, blocking file reads, the local fake cloud).
pub struct TestDom {
    pub vdom: VirtualDom,
    tree: Tree,
    rt: tokio::runtime::Runtime,
}

/// One element found on screen.
#[derive(Clone, Copy, Debug)]
pub struct El(usize);

/// An element as it is on screen now (see [`TestDom::nodes`]).
#[derive(Clone, Debug)]
pub struct Node {
    pub el: El,
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    /// All the text inside it.
    pub text: String,
    /// The class attributes of its ancestors, outermost first.
    pub within: Vec<String>,
    /// The events it listens to itself (`click`, `input`…).
    pub listeners: Vec<String>,
}

impl Node {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn class(&self) -> &str {
        self.attr("class").unwrap_or_default()
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.class().split_whitespace().any(|c| c == class)
    }

    /// Whether an ancestor has `class`.
    pub fn inside(&self, class: &str) -> bool {
        self.within.iter().any(|c| c.split_whitespace().any(|c| c == class))
    }

    pub fn disabled(&self) -> bool {
        self.attr("disabled").is_some_and(|v| v == "true")
    }

    pub fn listens(&self, event: &str) -> bool {
        self.listeners.iter().any(|l| l == event)
    }
}

impl TestDom {
    /// Mounts `app` and renders it.
    pub fn new(app: fn() -> Element) -> Self {
        Self::from_vdom(VirtualDom::new(app))
    }

    pub fn from_vdom(vdom: VirtualDom) -> Self {
        let mut me = Self::unsettled(vdom);
        me.settle();
        me
    }

    /// Mounts and renders once, leaving effects and tasks for the caller to run.
    pub fn unsettled(mut vdom: VirtualDom) -> Self {
        dioxus::html::set_event_converter(Box::new(SerializedHtmlEventConverter));
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
        let mut tree = Tree::new();
        {
            let _rt = rt.enter();
            vdom.rebuild(&mut tree);
        }
        Self { vdom, tree, rt }
    }

    /// Applies pending work (signal writes, effects, ready tasks).
    pub fn settle(&mut self) {
        let _rt = self.rt.enter();
        for _ in 0..8 {
            // Effects and ready futures run on `poll`; a bounded loop keeps
            // a test from hanging on a timer that never fires.
            let waker = std::task::Waker::noop();
            let mut cx = std::task::Context::from_waker(waker);
            let ready = {
                let mut work = std::pin::pin!(self.vdom.wait_for_work());
                work.as_mut().poll(&mut cx).is_ready()
            };
            if !ready {
                break;
            }
            self.vdom.render_immediate(&mut self.tree);
        }
        self.vdom.render_immediate(&mut self.tree);
    }

    /// Re-renders what is dirty now, without running any task.
    pub fn render(&mut self) {
        let _rt = self.rt.enter();
        self.vdom.render_immediate(&mut self.tree);
    }

    /// Runs tasks on the runtime, re-rendering, until none is woken within
    /// `quiet` (false: still busy after `rounds`).
    pub fn run_until_quiet(&mut self, quiet: std::time::Duration, rounds: usize) -> bool {
        for _ in 0..rounds {
            let vdom = &mut self.vdom;
            let woke =
                self.rt.block_on(async { tokio::time::timeout(quiet, vdom.wait_for_work()).await.is_ok() });
            if !woke {
                return true;
            }
            self.render();
        }
        false
    }

    /// Runs tasks for `ms` of real time, re-rendering as they finish.
    pub fn run_for(&mut self, ms: u64) {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        while let Some(left) = end.checked_duration_since(std::time::Instant::now()) {
            let vdom = &mut self.vdom;
            if self.rt.block_on(async { tokio::time::timeout(left, vdom.wait_for_work()).await.is_err() }) {
                break;
            }
            self.render();
        }
        self.render();
    }

    /// Lets `ms` of real time pass on the runtime (timers come due) without
    /// rendering.
    pub fn sleep(&mut self, ms: u64) {
        self.rt.block_on(async { tokio::time::sleep(std::time::Duration::from_millis(ms)).await });
    }

    /// Runs `f` inside the app's root scope (to read or write its contexts).
    pub fn in_app<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let out = self.peek_app(f);
        self.settle();
        out
    }

    /// Runs `f` inside the app's root scope without settling after.
    pub fn peek_app<T>(&self, f: impl FnOnce() -> T) -> T {
        let _rt = self.rt.enter();
        self.vdom.in_scope(ScopeId::APP, f)
    }

    /// The page as the SSR renderer draws it (escaped like a real page).
    pub fn ssr(&self) -> String {
        dioxus::ssr::render(&self.vdom)
    }

    fn text_of(&self, n: usize, out: &mut String) {
        match &self.tree.nodes[n].kind {
            Kind::Text(t) => out.push_str(t),
            Kind::Placeholder => {}
            Kind::Element { .. } => {
                for &c in &self.tree.nodes[n].children {
                    self.text_of(c, out);
                }
            }
        }
    }

    /// The visible text of an element (all its text, joined).
    pub fn text(&self, el: El) -> String {
        let mut s = String::new();
        self.text_of(el.0, &mut s);
        s
    }

    /// All the text on screen.
    pub fn screen(&self) -> String {
        self.text(El(0))
    }

    fn html_of(&self, n: usize, out: &mut String) {
        match &self.tree.nodes[n].kind {
            Kind::Text(t) => out.push_str(t),
            Kind::Placeholder => {}
            Kind::Element { tag, attrs, .. } => {
                out.push('<');
                out.push_str(tag);
                for (k, v) in attrs {
                    out.push_str(&format!(" {k}=\"{v}\""));
                }
                out.push('>');
                for &c in &self.tree.nodes[n].children {
                    self.html_of(c, out);
                }
                out.push_str(&format!("</{tag}>"));
            }
        }
    }

    /// The screen as rough HTML, for assertions on classes and attributes.
    pub fn html(&self) -> String {
        let mut s = String::new();
        self.html_of(0, &mut s);
        s
    }

    fn walk(&self, n: usize, out: &mut Vec<usize>) {
        out.push(n);
        for &c in &self.tree.nodes[n].children {
            self.walk(c, out);
        }
    }

    fn all(&self) -> Vec<usize> {
        let mut v = Vec::new();
        self.walk(0, &mut v);
        v
    }

    pub fn tag(&self, el: El) -> &str {
        match &self.tree.nodes[el.0].kind {
            Kind::Element { tag, .. } => tag,
            _ => "",
        }
    }

    pub fn attr(&self, el: El, name: &str) -> Option<&str> {
        match &self.tree.nodes[el.0].kind {
            Kind::Element { attrs, .. } => attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()),
            _ => None,
        }
    }

    pub fn has_class(&self, el: El, class: &str) -> bool {
        self.attr(el, "class").is_some_and(|c| c.split_whitespace().any(|x| x == class))
    }

    /// Every element with this tag.
    pub fn find_all(&self, tag: &str) -> Vec<El> {
        self.all().into_iter().map(El).filter(|&e| self.tag(e) == tag).collect()
    }

    /// Every element carrying `class`.
    pub fn by_class(&self, class: &str) -> Vec<El> {
        self.all().into_iter().map(El).filter(|&e| self.has_class(e, class)).collect()
    }

    /// Whether `inner` sits inside `outer` (or is it).
    pub fn contains(&self, outer: El, inner: El) -> bool {
        let mut n = Some(inner.0);
        while let Some(i) = n {
            if i == outer.0 {
                return true;
            }
            n = self.tree.nodes[i].parent;
        }
        false
    }

    /// The buttons on screen, in document order.
    pub fn buttons(&self) -> Vec<El> {
        self.find_all("button")
    }

    /// The first `tag` whose text contains `text`; panics with the screen if none.
    pub fn find(&self, tag: &str, text: &str) -> El {
        self.try_find(tag, text)
            .unwrap_or_else(|| panic!("no <{tag}> containing {text:?} on screen:\n{}", self.html()))
    }

    pub fn try_find(&self, tag: &str, text: &str) -> Option<El> {
        self.find_all(tag).into_iter().find(|&e| self.text(e).contains(text))
    }

    /// The button whose text contains `text`.
    pub fn button(&self, text: &str) -> El {
        self.find("button", text)
    }

    pub fn disabled(&self, el: El) -> bool {
        self.attr(el, "disabled").is_some()
    }

    fn listener_on(&self, el: El, name: &str) -> Option<ElementId> {
        // Like the browser, the event starts at the element; the runtime
        // bubbles it up the component tree from the nearest listener.
        let mut n = Some(el.0);
        while let Some(i) = n {
            let node = &self.tree.nodes[i];
            if let Kind::Element { listeners, .. } = &node.kind
                && listeners.iter().any(|l| l == name)
            {
                return node.id;
            }
            n = node.parent;
        }
        None
    }

    fn fire(&mut self, el: El, name: &str, data: Box<dyn Any>, bubbles: bool) {
        self.dispatch(el, name, data, bubbles);
        self.settle();
    }

    /// Sends `name` to `el` (the runtime bubbles it from the nearest
    /// listener, as a tap does) without settling after.
    pub fn dispatch(&mut self, el: El, name: &str, data: Box<dyn Any>, bubbles: bool) {
        let id = self
            .listener_on(el, name)
            .unwrap_or_else(|| panic!("nothing listens to {name} at {:?}:\n{}", self.text(el), self.html()));
        let event = dioxus::core::Event::new(Rc::new(PlatformEventData::new(data)) as Rc<dyn Any>, bubbles);
        let _rt = self.rt.enter();
        self.vdom.runtime().handle_event(name, event, id);
    }

    /// Every element on screen, in document order, as a snapshot a test can
    /// filter on (tag, attributes, text, ancestors' classes, listeners).
    pub fn nodes(&self) -> Vec<Node> {
        let mut out = Vec::new();
        self.snapshot(0, &mut Vec::new(), &mut out);
        out
    }

    fn snapshot(&self, n: usize, within: &mut Vec<String>, out: &mut Vec<Node>) {
        let Kind::Element { tag, attrs, listeners } = &self.tree.nodes[n].kind else { return };
        if n != 0 {
            out.push(Node {
                el: El(n),
                tag: tag.clone(),
                attrs: attrs.clone(),
                text: self.text(El(n)),
                within: within.clone(),
                listeners: listeners.clone(),
            });
        }
        within.push(self.attr(El(n), "class").unwrap_or_default().to_string());
        for &c in &self.tree.nodes[n].children {
            self.snapshot(c, within, out);
        }
        within.pop();
    }

    /// Taps an element (a click event at it, bubbling).
    pub fn click(&mut self, el: El) {
        assert!(!self.disabled(el), "clicking a disabled element: {}", self.text(el));
        self.fire(el, "click", Box::new(dioxus::html::SerializedMouseData::default()), true);
    }

    /// Taps the button whose text contains `text`.
    pub fn click_text(&mut self, text: &str) {
        let b = self.button(text);
        self.click(b);
    }

    /// Taps the first `tag` whose text contains `text`.
    pub fn click_on(&mut self, tag: &str, text: &str) {
        let e = self.find(tag, text);
        self.click(e);
    }

    /// Types `value` into a field (an `input` event with the new value).
    pub fn input(&mut self, el: El, value: &str) {
        let data = dioxus::html::SerializedFormData::new(value.to_string(), Vec::new());
        self.fire(el, "input", Box::new(data), true);
    }

    /// Submits a form (`submit`, as Enter in its field does).
    pub fn submit(&mut self, form: El) {
        let data = dioxus::html::SerializedFormData::new(String::new(), Vec::new());
        self.fire(form, "submit", Box::new(data), true);
    }

    /// Sets a select or field and sends `change`.
    pub fn change(&mut self, el: El, value: &str) {
        let data = dioxus::html::SerializedFormData::new(value.to_string(), Vec::new());
        self.fire(el, "change", Box::new(data), true);
    }
}

/// A fresh in-memory store.
pub fn memory_db() -> crate::Db {
    crate::testkit::memory_db()
}

/// One screen mounted with the app's contexts, its store and its WebView.
pub struct App {
    pub dom: TestDom,
    pub db: crate::Db,
    pub doc: Rc<FakeDoc>,
}

impl App {
    /// Mounts `body` over `db` (seed it first) in `lang`, with `doc` as the WebView.
    pub fn with(
        lang: kabar_core::locale::UiLanguage,
        db: crate::Db,
        doc: Rc<FakeDoc>,
        body: fn() -> Element,
    ) -> Self {
        let setup = Setup { lang, db: Some(db.clone()), doc: Some(doc.clone()), ..Setup::default() };
        let dom = TestDom::from_vdom(crate::testkit::mount(body, setup));
        Self { dom, db, doc }
    }

    /// Mounts `body` over an empty store, in `lang`, with no voices.
    pub fn new(lang: kabar_core::locale::UiLanguage, body: fn() -> Element) -> Self {
        Self::with(lang, memory_db(), FakeDoc::new(), body)
    }

    /// A context the app provides (Ui, Coach, PlayNav...).
    pub fn ctx<T: Clone + 'static>(&mut self) -> T {
        self.dom.in_app(consume_context::<T>)
    }

    /// The screen `Ui.screen` is on now.
    pub fn screen_now(&mut self) -> crate::Screen {
        let ui = self.ctx::<crate::Ui>();
        *ui.screen.peek()
    }

    /// The activity log as stored.
    pub fn activity(&self) -> Vec<kabar_protocol::coach::ActivityEvent> {
        self.db.0.borrow().activity(None).unwrap()
    }
}

impl std::ops::Deref for App {
    type Target = TestDom;
    fn deref(&self) -> &TestDom {
        &self.dom
    }
}

impl std::ops::DerefMut for App {
    fn deref_mut(&mut self) -> &mut TestDom {
        &mut self.dom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_reach_handlers_and_the_screen_follows() {
        fn app() -> Element {
            let mut n = use_signal(|| 0);
            let mut text = use_signal(String::new);
            rsx! {
                p { "count {n}" }
                if n() > 1 { span { class: "many big", "many" } }
                div { onclick: move |_| n += 10,
                    button { onclick: move |_| n += 1, "add" }
                }
                input { oninput: move |e| text.set(e.value()) }
                p { "typed {text}" }
                for i in 0..n().min(3) { em { key: "{i}", "{i}" } }
            }
        }
        let mut dom = TestDom::new(app);
        assert!(dom.screen().contains("count 0"));
        dom.click_text("add");
        // The click bubbles: the button's +1 and the div's +10.
        assert!(dom.screen().contains("count 11"), "{}", dom.html());
        assert_eq!(dom.by_class("big").len(), 1);
        assert_eq!(dom.find_all("em").len(), 3);
        let field = dom.find_all("input")[0];
        dom.input(field, "halo");
        assert!(dom.screen().contains("typed halo"));
    }
}
