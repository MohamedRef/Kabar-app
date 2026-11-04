//! Test-only: find a rendered element's event listener by what the element
//! shows, and fire the event, so a test can press a real button in a
//! `VirtualDom` and check what the press did (the SSR renderer alone never
//! runs a handler).
//!
//! ```ignore
//! let mut dom = VirtualDom::new(app);
//! dom.rebuild_in_place();
//! test_dom::click(&mut dom, "All stats");
//! assert!(test_dom::html(&dom).contains("Your stats"));
//! ```

use std::any::Any;
use std::rc::Rc;

use dioxus::core::{AttributeValue, DynamicNode, ElementId, Event, TemplateAttribute, TemplateNode, VNode};
use dioxus::html::{
    PlatformEventData, SerializedFormData, SerializedHtmlEventConverter, SerializedMouseData,
};
use dioxus::prelude::*;

/// One element listening for an event.
#[derive(Clone, Debug)]
pub struct Listener {
    /// The event without its `on` (`click`, `change`, `keydown`).
    pub event: String,
    pub id: ElementId,
    pub tag: String,
    /// The element's attributes (static and dynamic, text values only).
    pub attrs: Vec<(String, String)>,
    /// All text inside the element, components included.
    pub text: String,
}

impl Listener {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    pub fn class(&self) -> &str {
        self.attr("class").unwrap_or_default()
    }
}

/// The page as HTML, without its `<style>` blocks (so a failing assertion
/// prints the page, not the stylesheet).
pub fn html(dom: &VirtualDom) -> String {
    let mut html = dioxus::ssr::render(dom);
    while let Some(start) = html.find("<style>") {
        let end = html[start..].find("</style>").map_or(html.len(), |e| start + e + "</style>".len());
        html.replace_range(start..end, "");
    }
    if let Ok(dir) = std::env::var("KABAR_TEST_HTML") {
        let name = std::thread::current().name().unwrap_or("page").replace("::", "-");
        let _ = std::fs::write(format!("{dir}/{name}.html"), &html);
    }
    html
}

/// Every listener on the page, in document order.
pub fn listeners(dom: &VirtualDom) -> Vec<Listener> {
    let mut out = Vec::new();
    if let Some(scope) = dom.get_scope(ScopeId::APP) {
        walk(dom, scope.root_node(), &mut out);
    }
    out
}

fn walk(dom: &VirtualDom, node: &VNode, out: &mut Vec<Listener>) {
    for root in node.template.roots.iter() {
        walk_template(dom, node, root, &[], out);
    }
}

/// Walks a template node, collecting listeners on elements in document order.
fn walk_template(dom: &VirtualDom, node: &VNode, t: &TemplateNode, _path: &[u8], out: &mut Vec<Listener>) {
    match t {
        TemplateNode::Element { tag, attrs, children, .. } => {
            let mut all = Vec::new();
            let mut events = Vec::new();
            for a in attrs.iter() {
                match a {
                    TemplateAttribute::Static { name, value, .. } => {
                        all.push((name.to_string(), value.to_string()));
                    }
                    TemplateAttribute::Dynamic { id } => {
                        for attr in node.dynamic_attrs[*id].iter() {
                            match &attr.value {
                                AttributeValue::Listener(_) => {
                                    if let Some(el) = node.mounted_dynamic_attribute(*id, dom) {
                                        let name = attr.name.trim_start_matches("on").to_string();
                                        events.push((name, el));
                                    }
                                }
                                AttributeValue::Text(v) => all.push((attr.name.to_string(), v.clone())),
                                AttributeValue::Bool(b) => all.push((attr.name.to_string(), b.to_string())),
                                AttributeValue::Int(i) => all.push((attr.name.to_string(), i.to_string())),
                                AttributeValue::Float(f) => all.push((attr.name.to_string(), f.to_string())),
                                _ => {}
                            }
                        }
                    }
                }
            }
            if !events.is_empty() {
                let mut text = String::new();
                for c in children.iter() {
                    text_of(dom, node, c, &mut text);
                }
                for (event, id) in events {
                    out.push(Listener {
                        event,
                        id,
                        tag: tag.to_string(),
                        attrs: all.clone(),
                        text: text.clone(),
                    });
                }
            }
            for c in children.iter() {
                walk_template(dom, node, c, &[], out);
            }
        }
        TemplateNode::Text { .. } => {}
        TemplateNode::Dynamic { id } => walk_dynamic(dom, node, *id, out),
    }
}

fn walk_dynamic(dom: &VirtualDom, node: &VNode, id: usize, out: &mut Vec<Listener>) {
    match &node.dynamic_nodes[id] {
        DynamicNode::Component(c) => {
            if let Some(scope) = c.mounted_scope(id, node, dom) {
                walk(dom, scope.root_node(), out);
            }
        }
        DynamicNode::Fragment(nodes) => {
            for n in nodes {
                walk(dom, n, out);
            }
        }
        DynamicNode::Text(_) | DynamicNode::Placeholder(_) => {}
    }
}

fn text_of(dom: &VirtualDom, node: &VNode, t: &TemplateNode, out: &mut String) {
    match t {
        TemplateNode::Element { children, .. } => {
            for c in children.iter() {
                text_of(dom, node, c, out);
            }
        }
        TemplateNode::Text { text } => out.push_str(text),
        TemplateNode::Dynamic { id } => match &node.dynamic_nodes[*id] {
            DynamicNode::Text(v) => out.push_str(&v.value),
            DynamicNode::Component(c) => {
                if let Some(scope) = c.mounted_scope(*id, node, dom) {
                    vnode_text(dom, scope.root_node(), out);
                }
            }
            DynamicNode::Fragment(nodes) => {
                for n in nodes {
                    vnode_text(dom, n, out);
                }
            }
            DynamicNode::Placeholder(_) => {}
        },
    }
}

fn vnode_text(dom: &VirtualDom, node: &VNode, out: &mut String) {
    for root in node.template.roots.iter() {
        text_of(dom, node, root, out);
    }
}

/// The one listener for `event` whose element matches `pick`. Panics, with
/// what is on the page, when there is none or more than one.
pub fn find(dom: &VirtualDom, event: &str, pick: impl Fn(&Listener) -> bool) -> Listener {
    let all = listeners(dom);
    let hits: Vec<&Listener> = all.iter().filter(|l| l.event == event && pick(l)).collect();
    match hits.as_slice() {
        [one] => (*one).clone(),
        _ => panic!(
            "{} {event} listeners match; on the page: {:#?}",
            hits.len(),
            all.iter().map(|l| (&l.event, &l.tag, l.class(), &l.text)).collect::<Vec<_>>()
        ),
    }
}

thread_local! {
    /// Tokio for the app's timers (`use_future` loops sleep on it).
    static RT: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
}

/// Runs `f` with this thread's tokio runtime entered.
fn in_rt<T>(f: impl FnOnce() -> T) -> T {
    RT.with(|rt| {
        let _entered = rt.enter();
        f()
    })
}

fn converter() {
    use std::sync::Once;
    static SET: Once = Once::new();
    SET.call_once(|| dioxus::html::set_event_converter(Box::new(SerializedHtmlEventConverter)));
}

/// Fires `event` with `data` at a listener, then re-renders.
pub fn fire(dom: &mut VirtualDom, l: &Listener, data: Box<dyn Any>) {
    converter();
    let event = Event::new(Rc::new(PlatformEventData::new(data)) as Rc<dyn Any>, true);
    in_rt(|| {
        dom.runtime().handle_event(&l.event, event, l.id);
        dom.render_immediate_to_vec();
    });
}

/// Clicks the one clickable element whose text contains `text`.
pub fn click(dom: &mut VirtualDom, text: &str) {
    let l = find(dom, "click", |l| l.text.contains(text));
    click_on(dom, &l);
}

/// Clicks the one clickable element `pick` matches.
pub fn click_where(dom: &mut VirtualDom, pick: impl Fn(&Listener) -> bool) {
    let l = find(dom, "click", pick);
    click_on(dom, &l);
}

pub fn click_on(dom: &mut VirtualDom, l: &Listener) {
    fire(dom, l, Box::new(SerializedMouseData::default()));
}

/// Changes a form control (a checkbox: "true"/"false").
pub fn change(dom: &mut VirtualDom, pick: impl Fn(&Listener) -> bool, value: &str) {
    let l = find(dom, "change", pick);
    let data = SerializedFormData { value: value.to_string(), values: Vec::new(), valid: true };
    fire(dom, &l, Box::new(data));
}

/// Lets spawned tasks, futures and background threads run for `ms`
/// milliseconds, re-rendering as they finish.
pub fn settle(dom: &mut VirtualDom, ms: u64) {
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    RT.with(|rt| {
        rt.block_on(async {
            while let Some(left) = end.checked_duration_since(std::time::Instant::now()) {
                if tokio::time::timeout(left, dom.wait_for_work()).await.is_err() {
                    break;
                }
                dom.render_immediate_to_vec();
            }
        })
    });
    in_rt(|| dom.render_immediate_to_vec());
}

// ------------------------------------------------------------- app fixture

/// What a mounted test app starts with.
pub struct Setup {
    pub lang: kabar_core::locale::UiLanguage,
    pub eastern: bool,
    /// Window width in dp (sets the width class).
    pub width: f64,
    pub screen: crate::Screen,
    pub store: crate::store::Store,
    /// Draws a screen; anything it doesn't draw shows as `screen: <name>`.
    pub view: fn(crate::Screen) -> Option<Element>,
}

impl Setup {
    pub fn new(
        store: crate::store::Store,
        screen: crate::Screen,
        view: fn(crate::Screen) -> Option<Element>,
    ) -> Self {
        Self { lang: kabar_core::locale::UiLanguage::En, eastern: false, width: 1280.0, screen, store, view }
    }

    pub fn lang(mut self, lang: kabar_core::locale::UiLanguage) -> Self {
        self.lang = lang;
        self
    }

    pub fn eastern(mut self) -> Self {
        self.eastern = true;
        self
    }

    pub fn width(mut self, width: f64) -> Self {
        self.width = width;
        self
    }
}

thread_local! {
    static SETUP: std::cell::RefCell<Option<Setup>> = const { std::cell::RefCell::new(None) };
}

/// Mounts the app with `setup`'s store, settings and first screen, renders
/// it, and returns the dom.
pub fn mount(setup: Setup) -> VirtualDom {
    SETUP.with(|s| *s.borrow_mut() = Some(setup));
    let mut dom = VirtualDom::new(root);
    in_rt(|| dom.rebuild_in_place());
    dom
}

fn nothing(_: crate::Screen) -> Option<Element> {
    None
}

#[derive(Clone, Copy)]
struct View(fn(crate::Screen) -> Option<Element>);

fn root() -> Element {
    use crate::{Db, Ui};
    // The setup is read once, on the first render.
    let first = use_hook(|| {
        let setup = SETUP.with(|s| s.borrow_mut().take()).expect("mount() sets up the app");
        Rc::new(std::cell::RefCell::new(Some(setup)))
    });
    let (lang, eastern, width, screen, view) = {
        let s = first.borrow();
        match s.as_ref() {
            Some(s) => (s.lang, s.eastern, s.width, s.screen, s.view),
            None => (
                kabar_core::locale::UiLanguage::En,
                false,
                0.0,
                crate::Screen::Today,
                nothing as fn(crate::Screen) -> Option<Element>,
            ),
        }
    };
    crate::persistence::use_provide_saves();
    let db = use_context_provider(|| {
        let store = std::mem::replace(
            &mut first.borrow_mut().as_mut().expect("first render").store,
            crate::store::Store::open_in_memory().expect("in-memory store"),
        );
        Db(Rc::new(std::cell::RefCell::new(store)))
    });
    let ui = use_context_provider(|| {
        let settings = kabar_core::settings::Settings {
            ui_language: lang,
            eastern_digits: eastern,
            ..db.0.borrow().settings().unwrap_or_default()
        };
        Ui {
            settings: Signal::new(settings),
            width: Signal::new(kabar_core::layout::WidthClass::from_width(width)),
            tablet: Signal::new(false),
            screen: Signal::new(screen),
            lesson: Signal::new(None),
            lesson_from: Signal::new(crate::Screen::Journey),
            focus: Signal::new(false),
        }
    });
    use_context_provider(|| crate::coach::Coach::load(&db));
    crate::annotated::use_words(&db);
    let view = use_hook(|| View(view));
    // Later renders keep the first render's view.
    first.borrow_mut().take();
    let now = *ui.screen.read();
    let lesson = ui.lesson.read().clone();
    rsx! {
        div { id: "test-app", "data-screen": "{now:?}", "data-lesson": lesson.unwrap_or_default(),
            match (view.0)(now) {
                Some(page) => page,
                None => rsx! { p { class: "test-screen", "screen: {now:?}" } },
            }
        }
    }
}

/// The screen the app shows now.
pub fn screen(dom: &VirtualDom) -> crate::Screen {
    dom.in_scope(ScopeId::APP, || *consume_context::<crate::Ui>().screen.peek())
}

/// The lesson the player was asked to open.
pub fn lesson(dom: &VirtualDom) -> Option<String> {
    dom.in_scope(ScopeId::APP, || consume_context::<crate::Ui>().lesson.peek().clone())
}

/// The app's database.
pub fn db(dom: &VirtualDom) -> crate::Db {
    dom.in_scope(ScopeId::APP, consume_context::<crate::Db>)
}

/// Today, as the app's coach sees it.
pub fn today(dom: &VirtualDom) -> kabar_protocol::coach::LocalDate {
    dom.in_scope(ScopeId::APP, || consume_context::<crate::coach::Coach>().today())
}

/// Today for a store about to be mounted with the default profile.
pub fn today_now() -> kabar_protocol::coach::LocalDate {
    kabar_protocol::coach::LocalDate::from_unix_ms(
        crate::store::now_ms(),
        kabar_protocol::coach::CoachProfile::default().utc_offset_min,
    )
}

/// `text` as the HTML shows it (`'` → `&#39;`, `&` → `&amp;`, …).
pub fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Whether the page shows `text` (escaped as HTML would).
pub fn shows(html: &str, text: &str) -> bool {
    html.contains(&esc(text))
}

/// A template's text up to its first `{placeholder}`.
pub fn lead(template: &str) -> &str {
    template.split('{').next().unwrap_or_default()
}

/// Types `value` into the one text field `pick` matches.
pub fn input(dom: &mut VirtualDom, pick: impl Fn(&Listener) -> bool, value: &str) {
    let l = find(dom, "input", pick);
    let data = SerializedFormData { value: value.to_string(), values: Vec::new(), valid: true };
    fire(dom, &l, Box::new(data));
}
