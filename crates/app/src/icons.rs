//! Small stroke icons, drawn inline so they follow `currentColor`.

use dioxus::prelude::*;

use crate::Screen;

pub fn nav(screen: Screen) -> Element {
    let d = match screen {
        // sun over a horizon
        Screen::Today => "M4 18h16M7 18a5 5 0 0 1 10 0M12 6V4M5.6 9.6 4.2 8.2M18.4 9.6l1.4-1.4",
        // route between two islands
        Screen::Journey | Screen::Lesson => {
            "M5 19c3-1 3-5 7-6s4-5 7-6M5 19a2 2 0 1 0 0-.01M19 7a2 2 0 1 0 0-.01"
        }
        // open book
        Screen::Reader => "M3 6c3-1 6-1 9 1 3-2 6-2 9-1v12c-3-1-6-1-9 1-3-2-6-2-9-1zM12 7v12",
        // stacked cards
        Screen::Review => "M7 4h11v13H7zM4 7v13h11",
        // pen nib
        Screen::Studio => "M4 20l4-1 10-10-3-3L5 16zM13 7l3 3",
        // stylus with signal
        Screen::PenLab => "M5 19l2-6 8-8 3 3-8 8zM16 16c1.5 0 3 1 3 3",
        // speech bubble with a plus: a new word for Kabar
        Screen::Teach => "M4 5h16v11H9l-5 4zM12 8v5M9.5 10.5h5",
        // sliders
        Screen::Settings => "M4 7h10M18 7h2M4 17h4M12 17h8M14 5v4M8 15v4",
        Screen::Coach => COACH,
    };
    stroke(d)
}

/// Kak Nur (نور, light): a speech bubble with a small spark.
const COACH: &str =
    "M5 5h14a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-7l-4 3v-3H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zM12 7.5v5M9.5 10h5";

pub fn coach() -> Element {
    stroke(COACH)
}

/// Pelita: an oil lamp with its flame.
pub fn lamp() -> Element {
    stroke("M4 15h13a3 3 0 0 0 3-3M6 15c0 2 2 4 6 4s6-2 6-4M12 4c2 2 2 4 0 6-2-2-2-4 0-6z")
}

/// Koin.
pub fn coin() -> Element {
    stroke("M12 3a9 9 0 1 0 0 18 9 9 0 1 0 0-18zM12 7a5 5 0 1 0 0 10 5 5 0 1 0 0-10z")
}

/// An achievement.
pub fn star() -> Element {
    stroke("M12 3l2.6 5.6 6.1.7-4.5 4.2 1.2 6L12 16.6 6.6 19.5l1.2-6-4.5-4.2 6.1-.7z")
}

fn stroke(d: &'static str) -> Element {
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d }
        }
    }
}
