//! The Mac menu bar. On macOS WKWebView's editing shortcuts (Cmd+C/V/X/A/Z)
//! only work through an Edit menu, and Cmd+Q/W/H/M through the app and
//! Window menus, so Kabar needs one even though it draws its own navigation.
//! Other desktops keep no menu bar: GTK handles those keys without one.

use dioxus::desktop::muda::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use dioxus::prelude::*;

use crate::{Screen, Ui};

const SETTINGS_ID: &str = "kabar-settings";

/// Kabar (About, Settings…, Hide, Quit), Edit and Window. No developer
/// tools entry, unlike Dioxus's default menu.
pub fn mac_menu() -> Menu {
    let about = AboutMetadata {
        name: Some("Kabar".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        ..Default::default()
    };
    let app = Submenu::with_items(
        "Kabar",
        true,
        &[
            &PredefinedMenuItem::about(Some("About Kabar"), Some(about)),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(SETTINGS_ID, "Settings…", true, "CmdOrCtrl+Comma".parse().ok()),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(Some("Hide Kabar")),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(Some("Quit Kabar")),
        ],
    );
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
    );
    let window = Submenu::with_items(
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::maximize(None),
            &PredefinedMenuItem::fullscreen(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::close_window(None),
        ],
    );
    let menu = Menu::new();
    // A menu that fails to build is left out; the window still opens.
    for sub in [&app, &edit, &window].into_iter().flatten() {
        let _ = menu.append(sub);
    }
    #[cfg(target_os = "macos")]
    if let Ok(window) = &window {
        window.set_as_windows_menu_for_nsapp();
    }
    menu
}

/// Settings… (Cmd+,) opens the Settings screen.
pub fn use_menu_events(ui: Ui) {
    dioxus::desktop::use_muda_event_handler(move |e: &MenuEvent| {
        if e.id() == SETTINGS_ID {
            let mut screen = ui.screen;
            screen.set(Screen::Settings);
        }
    });
}
