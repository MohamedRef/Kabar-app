//! The word sheet: the card a tapped word opens, everywhere a word can be
//! tapped (the Reader, and annotated text in lessons, review, chat, stories,
//! Sinetron and Radio). On phones and tablets it is a bottom sheet in its
//! own raised colour (`--sheet`, tested per theme in `kabar_core::theme`)
//! that the learner resizes by its handle: free dragging, with a pull
//! towards a peek, half and near-full stop, a fling to the next stop, and
//! a drag (or fling) down to close. It reopens at the size it was left.
//! The Reader keeps it as a side column on wide windows (`side`).
//!
//! The gestures live in `assets/sheet.js`, installed once by [`use_gestures`]:
//! a drag must follow the finger frame by frame, which a round trip to Rust
//! per pointer move can't do on a phone. It closes the sheet by clicking its
//! close button, so Rust stays the one owner of what is open.

use dioxus::prelude::*;

use crate::Ui;
use crate::embedded::{Css, css};
use crate::i18n::Msg;

pub const CSS: Css = css!("assets/sheet.css");
const SHEET_JS: &str = include_str!("../assets/sheet.js");

/// Installs the sheet gestures for the whole app (listeners are delegated
/// from `document`, so sheets opened later need nothing).
pub fn use_gestures() {
    use_hook(|| {
        document::eval(SHEET_JS);
    });
}

/// A word sheet around `children`; `side` keeps it as a side column on
/// wide windows. System Back closes it (`data-back`).
#[component]
pub fn Sheet(
    label: String,
    on_close: EventHandler,
    #[props(default)] side: bool,
    children: Element,
) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div {
            class: if side { "bsheet side" } else { "bsheet" },
            role: "dialog",
            aria_label: "{label}",
            // Taps inside must not reach a button around the sentence.
            onclick: move |e| e.stop_propagation(),
            div {
                class: "bsheet-grip",
                role: "button",
                tabindex: "0",
                aria_label: "{ui.t(Msg::SheetResize)}",
                span { class: "bsheet-bar" }
            }
            button {
                class: "bsheet-close",
                "data-back": "true",
                "data-sheet-close": "true",
                aria_label: "{ui.t(Msg::Close)}",
                onclick: move |_| on_close.call(()),
                "×"
            }
            div { class: "bsheet-body", {children} }
        }
    }
}
