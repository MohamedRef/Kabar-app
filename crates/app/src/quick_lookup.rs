//! Mac Quick Lookup (SPEC §11): a global hotkey (⌘⇧K on the Mac, Ctrl+Shift+K
//! elsewhere) opens a small always-on-top window that reads whatever is on the
//! clipboard exactly like the Reader does: words colored by status, hover to
//! peek, click for the same word card.
//!
//! The window is a second `VirtualDom` on the same main thread, so it shares
//! the main window's `Db` (an `Rc`) but not its signals: it gets its own `Ui`
//! and `ReaderState`, and talks back to the main window over a channel
//! ("open this in the Reader", "teach this word", "statuses changed").
//!
//! Desktop builds only; Android never compiles this module.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::desktop::tao::dpi::PhysicalPosition;
use dioxus::desktop::{
    Config, DesktopContext, HotKeyState, LogicalSize, WeakDesktopContext, WindowBuilder,
    WindowCloseBehaviour, use_global_shortcut, use_wry_event_handler, window,
};
use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::StreamExt;
use kabar_core::layout::WidthClass;
use kabar_core::locale::UiLanguage;
use kabar_protocol::reader::WordStatus;

use crate::i18n::{Msg, t};
use crate::reader::{self, ReaderState};
use crate::store::{Highlight, SavedText, TextKind};
use crate::teach::{TeachInbox, TeachSeed};
use crate::{Db, MAIN_CSS, Screen, Ui};

/// `CmdOrCtrl` is ⌘ on macOS and Ctrl on Linux and Windows.
const HOTKEY: &str = "CmdOrCtrl+Shift+KeyK";

/// What the hotkey looks like, for the empty-clipboard hint.
const HOTKEY_LABEL: &str = if cfg!(target_os = "macos") { "⌘ ⇧ K" } else { "Ctrl + Shift + K" };

/// Logical size of the lookup window.
const SIZE: (f64, f64) = (420.0, 520.0);

/// Messages from the lookup window to the main window.
enum ToMain {
    /// Open this (already saved) text in the Reader and bring the main window up.
    Open(SavedText),
    /// The word card's "Teach Kabar" (or "review the draft"): open Teach.
    Teach(Option<TeachSeed>),
    /// Statuses or highlights changed: recolor the Reader if it shows them.
    Marks,
}

/// Shared between the main window (hotkey) and the lookup window.
#[derive(Clone)]
pub struct Link(Rc<LinkInner>);

struct LinkInner {
    to_main: UnboundedSender<ToMain>,
    /// The open lookup window and a poke that makes it re-read the clipboard.
    open: RefCell<Option<(WeakDesktopContext, UnboundedSender<()>)>>,
    /// A window was requested but hasn't mounted yet (debounces key repeat).
    opening: Cell<bool>,
}

impl PartialEq for Link {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Link {
    fn window(&self) -> Option<DesktopContext> {
        self.0.open.borrow().as_ref().and_then(|(w, _)| w.upgrade())
    }

    fn send(&self, msg: ToMain) {
        let _ = self.0.to_main.unbounded_send(msg);
    }
}

/// Registers the hotkey in the main window and handles what the lookup
/// window sends back. Call once, from the main window's root.
pub fn use_quick_lookup(db: Db, ui: Ui, reader: ReaderState, teach: TeachInbox) {
    let main = use_hook(window);
    let link = use_hook({
        let db = db.clone();
        let main = main.clone();
        move || {
            let (to_main, mut rx) = unbounded::<ToMain>();
            spawn(async move {
                let (mut screen, mut seed) = (ui.screen, teach.0);
                while let Some(msg) = rx.next().await {
                    match msg {
                        ToMain::Open(text) => {
                            reader.load(&db, text);
                            screen.set(Screen::Reader);
                            bring_to_front(&main);
                        }
                        ToMain::Teach(s) => {
                            if s.is_some() {
                                seed.set(s);
                            }
                            screen.set(Screen::Teach);
                            bring_to_front(&main);
                        }
                        ToMain::Marks => reader.reload_marks(&db),
                    }
                }
            });
            Link(Rc::new(LinkInner { to_main, open: RefCell::new(None), opening: Cell::new(false) }))
        }
    });

    let shortcut = use_global_shortcut(HOTKEY, {
        let link = link.clone();
        move |state| {
            if state == HotKeyState::Pressed {
                show(&db, &link, ui.lang());
            }
        }
    });
    use_hook(move || {
        // Linux registers only under X11; the Mac needs no permission.
        if let Err(e) = shortcut {
            eprintln!("kabar: Quick Lookup hotkey unavailable: {e:?}");
        }
    });

    // Closing the main window closes the lookup too, so Kabar doesn't linger
    // with only the small window.
    use_wry_event_handler(move |event, _| {
        use dioxus::desktop::tao::event::{Event, WindowEvent};
        if let Event::WindowEvent {
            window_id,
            event: WindowEvent::CloseRequested | WindowEvent::Destroyed,
            ..
        } = event
            && *window_id == main.id()
            && let Some(w) = link.window()
        {
            w.close();
        }
    });
}

fn bring_to_front(w: &DesktopContext) {
    w.set_visible(true);
    w.set_minimized(false);
    w.set_focus();
}

/// The hotkey: open the lookup window, or refresh the open one.
fn show(db: &Db, link: &Link, lang: UiLanguage) {
    if let Some(w) = link.window() {
        if let Some((_, poke)) = link.0.open.borrow().as_ref() {
            let _ = poke.unbounded_send(());
        }
        bring_to_front(&w);
        return;
    }
    if link.0.opening.replace(true) {
        return;
    }
    let main = window();
    let dom =
        VirtualDom::new_with_props(QuickLookup, QuickLookupProps { db: db.clone(), link: link.clone() });
    let cfg = Config::new()
        .with_window(builder(&main, lang))
        .with_menu(None)
        .with_disable_context_menu(true)
        .with_close_behaviour(WindowCloseBehaviour::WindowCloses);
    // The window mounts on the next turn of the event loop and registers itself.
    let _ = main.new_window(dom, cfg);
}

/// Small, always on top, next to the mouse (kept on its screen), or centered
/// on the main window's screen when the cursor position is unknown.
fn builder(main: &DesktopContext, lang: UiLanguage) -> WindowBuilder {
    let b = WindowBuilder::new()
        .with_title(format!("Kabar · {}", t(lang, Msg::QuickLookup)))
        .with_inner_size(LogicalSize::new(SIZE.0, SIZE.1))
        .with_min_inner_size(LogicalSize::new(320.0, 360.0))
        .with_always_on_top(true)
        .with_focused(true);
    let cursor = main.cursor_position().ok();
    let monitor = match cursor {
        Some(c) => main.monitor_from_point(c.x, c.y),
        None => main.current_monitor(),
    };
    let Some(m) = monitor else { return b };
    let scale = m.scale_factor();
    let (pos, size) = (m.position(), m.size());
    let screen = Rect { x: pos.x as f64, y: pos.y as f64, w: size.width as f64, h: size.height as f64 };
    let (x, y) = place(cursor.map(|c| (c.x, c.y)), screen, (SIZE.0 * scale, SIZE.1 * scale));
    b.with_position(PhysicalPosition::new(x, y))
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Top-left corner for a `win`-sized window: just below and after the cursor,
/// clamped inside `screen`; centered without a cursor. Physical pixels.
fn place(cursor: Option<(f64, f64)>, screen: Rect, win: (f64, f64)) -> (f64, f64) {
    const GAP: f64 = 16.0;
    let clamp = |v: f64, lo: f64, len: f64, size: f64| v.min(lo + len - size).max(lo);
    match cursor {
        Some((cx, cy)) => {
            (clamp(cx + GAP, screen.x, screen.w, win.0), clamp(cy + GAP, screen.y, screen.h, win.1))
        }
        None => {
            (screen.x + ((screen.w - win.0) / 2.0).max(0.0), screen.y + ((screen.h - win.1) / 2.0).max(0.0))
        }
    }
}

/// The clipboard's text, if it has any that isn't blank.
fn clipboard_text() -> Option<String> {
    let text = arboard::Clipboard::new().and_then(|mut c| c.get_text());
    match text {
        Ok(s) if !s.trim().is_empty() => Some(s),
        Ok(_) => None,
        // Empty or non-text (an image, a file) is the common case, not an error.
        Err(arboard::Error::ContentNotAvailable) => None,
        Err(e) => {
            eprintln!("kabar: reading the clipboard failed: {e}");
            None
        }
    }
}

/// Marks as last loaded, to tell the learner's changes from a fresh load.
type Marks = (HashMap<String, WordStatus>, Vec<Highlight>);

#[component]
fn QuickLookup(db: Db, link: Link) -> Element {
    let lang = use_hook(|| db.0.borrow().settings().unwrap_or_default().ui_language);
    let ui = use_context_provider(|| Ui {
        settings: Signal::new(db.0.borrow().settings().unwrap_or_default()),
        width: Signal::new(WidthClass::Compact),
        screen: Signal::new(Screen::Reader),
        lesson: Signal::new(None),
    });
    use_context_provider(|| db.clone());
    let teach = use_context_provider(|| TeachInbox(Signal::new(None)));
    let st = use_context_provider(|| ReaderState::new(lang));
    reader::use_input_layer(db.clone(), st);
    let baseline = use_hook(|| Rc::new(RefCell::new(Marks::default())));
    // Whether the shown text is saved yet (it is once something refers to it).
    let mut kept = use_signal(|| false);

    // Reads the clipboard into the window. The same text stays as it is, with
    // its highlights; a new text replaces it, unsaved until it's needed.
    let refresh = use_callback({
        let db = db.clone();
        let baseline = baseline.clone();
        move |()| {
            let mut ui = ui;
            ui.settings.set(db.0.borrow().settings().unwrap_or_default());
            let mut st = st;
            let Some(body) = clipboard_text() else {
                st.open.set(None);
                st.peek.set(None);
                st.panel.set(None);
                return;
            };
            if st.open.peek().as_ref().is_some_and(|o| o.text.body == body) {
                return;
            }
            // Looked up (and kept) a moment ago: reuse it, with its highlights,
            // instead of saving the same text twice.
            let latest =
                db.0.borrow().recent_texts(1).unwrap_or_default().into_iter().find(|t| t.body == body);
            kept.set(latest.is_some());
            let text = latest.unwrap_or_else(|| SavedText {
                id: uuid::Uuid::new_v4().to_string(),
                title: String::new(),
                body,
                kind: TextKind::Pasted,
                source: None,
                layout: None,
            });
            st.load(&db, text);
            *baseline.borrow_mut() = (st.statuses.peek().clone(), st.highlights.peek().clone());
        }
    });

    // Saves the shown text under its id (once) and returns it.
    let keep = use_callback({
        let db = db.clone();
        move |()| -> Option<SavedText> {
            let open = st.open.peek().clone()?;
            if *kept.peek() {
                return Some(open.text);
            }
            let t = &open.text;
            match db.0.borrow_mut().save_text_as(&t.id, &t.body, t.kind, t.source.as_deref(), None) {
                Ok(saved) => {
                    kept.set(true);
                    Some(saved)
                }
                Err(e) => {
                    eprintln!("kabar: saving the looked-up text failed: {e}");
                    None
                }
            }
        }
    });

    // Register with the main window, read the clipboard, take focus, and
    // listen for the hotkey's "read it again" poke.
    use_hook({
        let link = link.clone();
        move || {
            let me = window();
            let (poke, mut pokes) = unbounded::<()>();
            *link.0.open.borrow_mut() = Some((Rc::downgrade(&me), poke));
            link.0.opening.set(false);
            spawn(async move {
                refresh.call(());
                me.set_focus();
                let _ = me.webview.focus();
                while pokes.next().await.is_some() {
                    refresh.call(());
                }
            });
        }
    });
    use_drop({
        let link = link.clone();
        move || {
            *link.0.open.borrow_mut() = None;
        }
    });
    // Escape closes the window.
    use_future(|| async {
        let mut eval = document::eval(
            r#"
            document.addEventListener("keydown", (e) => { if (e.key === "Escape") dioxus.send(true); });
            await new Promise(() => {});
            "#,
        );
        while eval.recv::<bool>().await.is_ok() {
            window().close();
        }
    });

    // The learner changed a status or saved a phrase: the text is worth
    // keeping (highlights and saved words point to it), and the Reader in
    // the main window recolors.
    use_effect({
        let link = link.clone();
        move || {
            let now: Marks = (st.statuses.read().clone(), st.highlights.read().clone());
            if *baseline.borrow() == now {
                return;
            }
            *baseline.borrow_mut() = now;
            keep.call(());
            link.send(ToMain::Marks);
        }
    });

    // The word card's teach buttons switch this window's screen to Teach:
    // hand that to the main window instead.
    use_effect({
        let link = link.clone();
        move || {
            if *ui.screen.read() != Screen::Teach {
                return;
            }
            let mut screen = ui.screen;
            screen.set(Screen::Reader);
            let mut slot = teach.0;
            let seed = slot.peek().clone();
            slot.set(None);
            link.send(ToMain::Teach(seed));
            window().close();
        }
    });

    let settings = ui.settings.read();
    let lang = settings.ui_language;
    let open = st.open.read().clone();
    let has_text = open.is_some();

    rsx! {
        document::Title { "Kabar · {ui.t(Msg::QuickLookup)}" }
        style { {MAIN_CSS} }
        div {
            class: "app quick-lookup",
            lang: lang.tag(),
            dir: lang.dir().as_str(),
            "data-theme": settings.theme.as_str(),
            "data-width": WidthClass::Compact.as_str(),
            header { class: "ql-bar",
                h1 { "{ui.t(Msg::QuickLookup)}" }
                button {
                    class: "ghost",
                    title: "{ui.t(Msg::QuickLookupAgain)}",
                    aria_label: "{ui.t(Msg::QuickLookupAgain)}",
                    onclick: move |_| refresh.call(()),
                    "↻"
                }
                button {
                    class: "primary",
                    disabled: !has_text,
                    onclick: {
                        let link = link.clone();
                        move |_| {
                            if let Some(text) = keep.call(()) {
                                link.send(ToMain::Open(text));
                                window().close();
                            }
                        }
                    },
                    "{ui.t(Msg::QuickLookupOpenInReader)}"
                }
                button {
                    class: "ghost",
                    title: "{ui.t(Msg::Close)}",
                    aria_label: "{ui.t(Msg::Close)}",
                    onclick: move |_| window().close(),
                    "×"
                }
            }
            main { class: "ql-body reader",
                match open {
                    Some(open) => rsx! {
                        reader::ReadView { open, mode: reader::Mode::Inline }
                    },
                    None => rsx! {
                        div { class: "card ql-empty", role: "status",
                            p { strong { "{ui.t(Msg::QuickLookupEmpty)}" } }
                            p { "{ui.t(Msg::QuickLookupHowTo)}" }
                            p {
                                bdi {
                                    kbd { dir: "ltr", "{HOTKEY_LABEL}" }
                                }
                            }
                        }
                    },
                }
                if let Some(msg) = *st.toast.read() {
                    div { class: "toast", role: "status",
                        span { "{ui.t(msg)}" }
                        button {
                            class: "ghost",
                            aria_label: "{ui.t(Msg::Close)}",
                            onclick: move |_| {
                                let mut t = st.toast;
                                t.set(None);
                            },
                            "×"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 };

    #[test]
    fn opens_next_to_the_cursor() {
        assert_eq!(place(Some((100.0, 200.0)), SCREEN, (420.0, 520.0)), (116.0, 216.0));
    }

    #[test]
    fn stays_on_screen_near_the_edges() {
        assert_eq!(place(Some((1900.0, 1070.0)), SCREEN, (420.0, 520.0)), (1500.0, 560.0));
        // A second monitor to the left of the main one.
        let left = Rect { x: -1440.0, y: 0.0, w: 1440.0, h: 900.0 };
        assert_eq!(place(Some((-10.0, 10.0)), left, (840.0, 1040.0)), (-840.0, 0.0));
    }

    #[test]
    fn centered_without_a_cursor() {
        assert_eq!(place(None, SCREEN, (420.0, 520.0)), (750.0, 280.0));
    }

    #[test]
    fn hotkey_parses() {
        // `use_global_shortcut` unwraps the parse: a typo would panic at launch.
        use global_hotkey::hotkey::{Code, HotKey, Modifiers};
        let key: HotKey = HOTKEY.parse().unwrap();
        let cmd = if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CONTROL };
        assert_eq!(key, HotKey::new(Some(cmd | Modifiers::SHIFT), Code::KeyK));
    }
}
