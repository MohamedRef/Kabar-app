//! The Mac menu bar: Kabar › Settings… (⌘,), the standard Edit and Window
//! menus (so ⌘C, ⌘V and ⌘W work as everywhere else), Go with ⌘1–⌘7, and
//! Help › Keyboard Shortcuts (also the "?" key).
//! Built on every desktop so it is type-checked here; only macOS installs it.

use dioxus::desktop::muda::accelerator::{Accelerator, Code, Modifiers};
use dioxus::desktop::muda::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use dioxus::prelude::*;

use super::{Section, SettingsNav};
use crate::{Screen, Ui};

const SETTINGS: &str = "kabar.settings";
const SHORTCUTS: &str = "kabar.shortcuts";

/// Go menu: (id, title, key, screen).
const GO: [(&str, &str, Code, Screen); 7] = [
    ("kabar.go.today", "Today", Code::Digit1, Screen::Today),
    ("kabar.go.journey", "Journey", Code::Digit2, Screen::Journey),
    ("kabar.go.reader", "Reader", Code::Digit3, Screen::Reader),
    ("kabar.go.review", "Review", Code::Digit4, Screen::Review),
    ("kabar.go.play", "Play", Code::Digit5, Screen::Play),
    ("kabar.go.listen", "Watch & listen", Code::Digit6, Screen::Listen),
    ("kabar.go.chat", "Chat", Code::Digit7, Screen::Chat),
];

fn cmd(key: Code) -> Option<Accelerator> {
    Some(Accelerator::new(Some(Modifiers::SUPER), key))
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn menu_bar() -> Menu {
    let about = AboutMetadata {
        name: Some("Kabar".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        comments: Some("Indonesian and Javanese, from Arabic.".into()),
        ..Default::default()
    };
    // On macOS the first submenu is the app menu, titled with the app's name.
    let app = Submenu::with_items(
        "Kabar",
        true,
        &[
            &PredefinedMenuItem::about(Some("About Kabar"), Some(about)),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(SETTINGS, "Settings…", true, cmd(Code::Comma)),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(Some("Hide Kabar")),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(Some("Quit Kabar")),
        ],
    )
    .expect("app menu");
    let edit = Submenu::with_items(
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ],
    )
    .expect("edit menu");
    let go = Submenu::new("Go", true);
    for (id, title, key, _) in GO {
        go.append(&MenuItem::with_id(id, title, true, cmd(key))).expect("go item");
    }
    let window = Submenu::with_items(
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::maximize(Some("Zoom")),
            &PredefinedMenuItem::fullscreen(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::close_window(None),
        ],
    )
    .expect("window menu");
    let help =
        Submenu::with_items("Help", true, &[&MenuItem::with_id(SHORTCUTS, "Keyboard Shortcuts", true, None)])
            .expect("help menu");
    let menu = Menu::new();
    menu.append_items(&[&app, &edit, &go, &window, &help]).expect("menu bar");
    #[cfg(target_os = "macos")]
    {
        window.set_as_windows_menu_for_nsapp();
        help.set_as_help_menu_for_nsapp();
    }
    menu
}

/// Answers the menu's own items, and "?" (outside text fields) opens the
/// shortcut list like Help › Keyboard Shortcuts.
pub fn use_menu_events(ui: Ui, nav: SettingsNav) {
    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            if (window.__kabarHelpKey) document.removeEventListener("keydown", window.__kabarHelpKey);
            window.__kabarHelpKey = (e) => {
                const t = e.target;
                const typing = t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
                if (e.key === "?" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
                    e.preventDefault();
                    dioxus.send("help");
                }
            };
            document.addEventListener("keydown", window.__kabarHelpKey);
            await new Promise(() => {});
            "#,
        );
        while eval.recv::<String>().await.is_ok() {
            nav.show(Some(Section::Mac));
        }
    });
    dioxus::desktop::use_muda_event_handler(move |event| {
        let id = event.id().0.as_str();
        match id {
            SETTINGS => nav.show(None),
            SHORTCUTS => nav.show(Some(Section::Mac)),
            _ => {
                if let Some((_, _, _, screen)) = GO.iter().find(|(go, ..)| *go == id) {
                    nav.close();
                    let mut s = ui.screen;
                    s.set(*screen);
                }
            }
        }
    });
}
