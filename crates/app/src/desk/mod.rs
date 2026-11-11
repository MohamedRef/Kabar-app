//! Desk tools for wide windows, mostly for the Mac:
//! - panes you widen or narrow by dragging their edge (the sidebar and the
//!   Reader's word panel), kept per device;
//! - focus mode for review sessions on tablet and Mac (lessons already
//!   open full screen over the sidebar; the Reader has its own);
//! - quick search, Ctrl/⌘+K, to jump to any lesson, screen or game.
//!
//! Adapted from batna-app main: crates/app/src/desk/mod.rs (panes and focus)
//! and crates/app/src/shortcuts/palette.rs (quick search).

pub mod msg;
pub mod search;

use dioxus::prelude::*;

use crate::settings::device::{DevicePrefs, Pane};
use crate::{Db, Ui};
use msg::DeskMsg;

pub(crate) const CSS: crate::embedded::Css = crate::embedded::css!("assets/desk.css");

/// What the drag script sent: a pane and its new width in tenths of a rem
/// (zero: back to the default). Anything else is ignored.
fn parse_drag(message: &str) -> Option<(Pane, u16)> {
    let (pane, tenths) = message.split_once(':')?;
    Some((Pane::parse(pane)?, tenths.parse().ok()?))
}

/// The CSS that puts saved widths on the page root; a pane at zero keeps
/// the stylesheet's width.
fn widths_script(prefs: crate::settings::device::Panes) -> String {
    Pane::ALL
        .into_iter()
        .map(|pane| {
            let var = format!("--kb-{}-w", pane.as_str());
            match prefs.get(pane) {
                0 => format!("document.documentElement.style.removeProperty({var:?});"),
                w => format!(
                    "document.documentElement.style.setProperty({var:?}, '{}rem');",
                    f32::from(w) / 10.0
                ),
            }
        })
        .collect()
}

/// Called once by the shell: the saved widths, the drag script and the
/// ⌘K key.
pub fn use_desk() {
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    use_effect(move || {
        document::eval(&widths_script(*prefs.panes.read()));
    });
    use_future(move || {
        let db = db.clone();
        async move {
            let mut script = document::eval(include_str!("resize.js"));
            while let Ok(message) = script.recv::<String>().await {
                if let Some((pane, tenths)) = parse_drag(&message) {
                    prefs.set_pane(&db, pane, tenths);
                }
            }
        }
    });
    use_future(move || async move {
        let mut script = document::eval(include_str!("keys.js"));
        while let Ok(message) = script.recv::<String>().await {
            search::open(message == "open");
        }
    });
}

/// The drag handle on a pane's edge. `anchor` is the element the width is
/// measured from: its inline-start edge for the sidebar (`side: "start"`),
/// its inline-end edge for the Reader's panel (`side: "end"`).
#[component]
pub fn Handle(pane: Pane) -> Element {
    let ui = use_context::<Ui>();
    let prefs = use_context::<DevicePrefs>();
    let (min, max, default) = pane.range();
    let now = match prefs.panes.read().get(pane) {
        0 => default,
        w => w,
    };
    let (side, anchor, label) = match pane {
        Pane::Nav => ("start", ".shell", DeskMsg::ResizeNav),
        Pane::Panel => ("end", ".read-layout", DeskMsg::ResizePanel),
    };
    rsx! {
        div {
            class: "kb-resize",
            role: "separator",
            tabindex: "0",
            aria_orientation: "vertical",
            aria_label: "{ui.t(label)}",
            title: "{ui.t(DeskMsg::ResizeHelp)}",
            aria_valuemin: "{min}",
            aria_valuemax: "{max}",
            aria_valuenow: "{now}",
            aria_valuetext: "{f32::from(now) / 10.0:.1} rem",
            "data-pane": pane.as_str(),
            "data-side": side,
            "data-anchor": anchor,
            "data-min": "{min}",
            "data-max": "{max}",
            "data-default": "{default}",
            span { class: "kb-resize-grip", aria_hidden: "true" }
        }
    }
}

/// Focus mode for a review session on tablet and Mac: one press hides the
/// sidebar, the next brings it back; the choice stays on this device.
/// Hidden on phones, where a session already has the whole screen.
#[component]
pub fn FocusToggle() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    let on = *prefs.study_focus.read();
    let label = ui.t(if on { DeskMsg::FocusOff } else { DeskMsg::FocusOn });
    rsx! {
        div { class: "focus-bar",
            button {
                class: "focus-toggle",
                aria_pressed: if on { "true" } else { "false" },
                title: "{label}",
                onclick: move |_| prefs.set_study_focus(&db, !on),
                span { aria_hidden: "true", if on { "⤡ " } else { "⤢ " } }
                "{label}"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::device::Panes;

    #[test]
    fn drag_messages_name_a_pane_and_a_width() {
        assert_eq!(parse_drag("nav:185"), Some((Pane::Nav, 185)));
        assert_eq!(parse_drag("panel:0"), Some((Pane::Panel, 0)));
        assert_eq!(parse_drag("rail:100"), None);
        assert_eq!(parse_drag("nav:wide"), None);
        assert_eq!(parse_drag("nav"), None);
    }

    #[test]
    fn saved_widths_reach_the_page_and_zero_clears_them() {
        let js = widths_script(Panes { nav: 185, panel: 0 });
        assert!(js.contains(r#"setProperty("--kb-nav-w", '18.5rem')"#), "{js}");
        assert!(js.contains(r#"removeProperty("--kb-panel-w")"#), "{js}");
    }

    #[test]
    fn the_drag_script_handles_pointer_keys_reset_and_rtl() {
        let js = include_str!("resize.js");
        for needle in [
            "setPointerCapture",
            "pointercancel",
            "ArrowLeft",
            "Home",
            "End",
            "dblclick",
            "rtl",
            "dioxus.send",
        ] {
            assert!(js.contains(needle), "resize.js needs {needle}");
        }
        let css = CSS.text();
        for needle in [".kb-resize", "--kb-nav-w", "--kb-panel-w", "data-study-focus", ".quick-search"] {
            assert!(css.contains(needle), "desk.css needs {needle}");
        }
    }
}
