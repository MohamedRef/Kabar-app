//! Focused study (redesign 2026-10-29): while a lesson or a review session
//! is on screen, the phone's bottom bar and top bar step aside so the task
//! and its fixed footer (Check/Continue, the rating row) get the screen.
//! Adapted from Batna-app main: crates/app/src/parts.rs (`use_focus_mode`).

use dioxus::prelude::*;

use crate::Ui;

/// Turns focus mode on while the calling component is mounted and `on` is
/// true. The component must show its own way out, marked `"data-back":
/// "true"` so Android Back takes it. From 600dp the rail stays: there is
/// room for both (CSS keys on `data-focus` only under compact).
pub fn use_focus_mode(on: bool) {
    let mut focus = use_context::<Ui>().focus;
    use_effect(use_reactive!(|on| {
        if *focus.peek() != on {
            focus.set(on);
        }
    }));
    use_drop(move || focus.set(false));
}
