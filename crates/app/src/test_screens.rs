//! Test-only: find a rendered element's event listener by what the element
//! shows, and fire the event, so a test can press a real button in a
//! `VirtualDom` and check what the press did (the SSR renderer alone never
//! runs a handler).
//!
//! ```ignore
//! let mut dom = test_screens::mount(Setup::new(store, Screen::Today, view));
//! test_screens::click(&mut dom, "All stats");
//! assert!(test_screens::html(&dom).contains("Your stats"));
//! ```
//!
//! A thin front end over the shared test kit ([`crate::testkit`]): the
//! DOM is [`TestDom`], the contexts come from `testkit::provide`.

use std::any::Any;

use dioxus::html::{SerializedFormData, SerializedMouseData};
use dioxus::prelude::*;

use crate::test_dom::{El, TestDom};

/// One element listening for an event.
#[derive(Clone, Debug)]
pub struct Listener {
    /// The event without its `on` (`click`, `change`, `keydown`).
    pub event: String,
    pub el: El,
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
pub fn html(dom: &TestDom) -> String {
    let mut html = dom.ssr();
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
pub fn listeners(dom: &TestDom) -> Vec<Listener> {
    let mut out = Vec::new();
    for n in dom.nodes() {
        for event in &n.listeners {
            out.push(Listener {
                event: event.clone(),
                el: n.el,
                tag: n.tag.clone(),
                attrs: n.attrs.clone(),
                text: n.text.clone(),
            });
        }
    }
    out
}

/// The one listener for `event` whose element matches `pick`. Panics, with
/// what is on the page, when there is none or more than one.
pub fn find(dom: &TestDom, event: &str, pick: impl Fn(&Listener) -> bool) -> Listener {
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

/// Fires `event` with `data` at a listener, then re-renders.
pub fn fire(dom: &mut TestDom, l: &Listener, data: Box<dyn Any>) {
    dom.dispatch(l.el, &l.event, data, true);
    dom.render();
}

/// Clicks the one clickable element whose text contains `text`.
pub fn click(dom: &mut TestDom, text: &str) {
    let l = find(dom, "click", |l| l.text.contains(text));
    click_on(dom, &l);
}

/// Clicks the one clickable element `pick` matches.
pub fn click_where(dom: &mut TestDom, pick: impl Fn(&Listener) -> bool) {
    let l = find(dom, "click", pick);
    click_on(dom, &l);
}

pub fn click_on(dom: &mut TestDom, l: &Listener) {
    fire(dom, l, Box::new(SerializedMouseData::default()));
}

/// Changes a form control (a checkbox: "true"/"false").
pub fn change(dom: &mut TestDom, pick: impl Fn(&Listener) -> bool, value: &str) {
    let l = find(dom, "change", pick);
    let data = SerializedFormData { value: value.to_string(), values: Vec::new(), valid: true };
    fire(dom, &l, Box::new(data));
}

/// Lets spawned tasks, futures and background threads run for `ms`
/// milliseconds, re-rendering as they finish.
pub fn settle(dom: &mut TestDom, ms: u64) {
    dom.run_for(ms);
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

/// Mounts the app with `setup`'s store, settings and first screen, renders
/// it, and returns the dom.
pub fn mount(setup: Setup) -> TestDom {
    let Setup { lang, eastern, width, screen, store, view } = setup;
    let settings = kabar_core::settings::Settings {
        ui_language: lang,
        eastern_digits: eastern,
        ..store.settings().unwrap_or_default()
    };
    VIEW.set(Some(view));
    let setup = crate::testkit::Setup {
        lang,
        settings,
        width: kabar_core::layout::WidthClass::from_width(width),
        screen,
        store: Some(store),
        ..Default::default()
    };
    TestDom::unsettled(crate::testkit::mount(app, setup))
}

thread_local! {
    /// The view of the app [`mount`] sets up next.
    static VIEW: std::cell::Cell<Option<ViewFn>> = const { std::cell::Cell::new(None) };
}

/// Draws a screen, or `None` for one the test doesn't draw.
type ViewFn = fn(crate::Screen) -> Option<Element>;

#[derive(Clone, Copy)]
struct View(ViewFn);

fn app() -> Element {
    // The view is read once, on the first render.
    let view = use_hook(|| View(VIEW.take().expect("mount() sets up the app")));
    let ui = consume_context::<crate::Ui>();
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
pub fn screen(dom: &TestDom) -> crate::Screen {
    dom.peek_app(|| *consume_context::<crate::Ui>().screen.peek())
}

/// The lesson the player was asked to open.
pub fn lesson(dom: &TestDom) -> Option<String> {
    dom.peek_app(|| consume_context::<crate::Ui>().lesson.peek().clone())
}

/// The app's database.
pub fn db(dom: &TestDom) -> crate::Db {
    dom.peek_app(consume_context::<crate::Db>)
}

/// Today, as the app's coach sees it.
pub fn today(dom: &TestDom) -> kabar_protocol::coach::LocalDate {
    dom.peek_app(|| consume_context::<crate::coach::Coach>().today())
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
pub fn input(dom: &mut TestDom, pick: impl Fn(&Listener) -> bool, value: &str) {
    let l = find(dom, "input", pick);
    let data = SerializedFormData { value: value.to_string(), values: Vec::new(), valid: true };
    fire(dom, &l, Box::new(data));
}
