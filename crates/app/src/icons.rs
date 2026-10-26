//! Small stroke icons, drawn inline so they follow `currentColor`.

use dioxus::prelude::*;

use crate::Screen;
use crate::play::Route;

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
        // three bars on a baseline
        Screen::Stats => "M4 20h16M7 16v-5M12 16V6M17 16v-8",
        // a check mark in a circle
        Screen::LevelCheck => "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM8 12l3 3 5-6",
        // pen nib
        Screen::Studio => "M4 20l4-1 10-10-3-3L5 16zM13 7l3 3",
        // stylus with signal
        Screen::PenLab => "M5 19l2-6 8-8 3 3-8 8zM16 16c1.5 0 3 1 3 3",
        // phone beside a laptop
        Screen::Devices => "M3 6h12v9H3zM1 18h15M18 9h4v11h-4z",
        // speech bubble with a plus: a new word for Kabar
        Screen::Teach => "M4 5h16v11H9l-5 4zM12 8v5M9.5 10.5h5",
        // a notebook with a ribbon bookmark
        Screen::Journal => "M6 4h12v16H7a1 1 0 0 1-1-1zM6 16h12M10 4v6l2-1.5 2 1.5V4",
        // a gauge
        Screen::Cost => "M4 16a8 8 0 0 1 16 0M12 16l4-5M3 20h18",
        // sliders
        Screen::Settings => "M4 7h10M18 7h2M4 17h4M12 17h8M14 5v4M8 15v4",
        // a game controller
        Screen::Play => PLAY,
        Screen::Coach => COACH,
    };
    stroke(d)
}

/// Play: a rounded game pad with a cross and two buttons.
const PLAY: &str = "M7 8h10a4 4 0 0 1 4 4v1a4 4 0 0 1-7 2.6h-4A4 4 0 0 1 3 13v-1a4 4 0 0 1 4-4zM7 11v3M5.5 12.5h3M15.5 11.5h.01M17.5 13.5h.01";

/// Each Play game's card icon.
pub fn game(route: Route) -> Element {
    let d = match route {
        // sun rising, with a speech line
        Route::Pagi => "M3 17h18M7 17a5 5 0 0 1 10 0M12 7V5M6 10.5 4.8 9.3M18 10.5l1.2-1.2M8 21h8",
        // an arch bridge
        Route::Deck => "M3 17h18M5 17v-3a7 7 0 0 1 14 0v3M9 17v-2M15 17v-2M12 17v-3",
        // magnifying glass
        Route::Detective => "M10.5 4a6.5 6.5 0 1 0 0 13 6.5 6.5 0 1 0 0-13zM15.5 15.5 20 20",
        // a bell
        Route::Alarm => "M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15zM10 20a2 2 0 0 0 4 0M12 3v2",
        // a banknote
        Route::Rupiah => "M3 7h18v10H3zM12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 1 0 0-5zM6 10v4M18 10v4",
        // a market stall awning
        Route::Pasar => {
            "M4 9l2-5h12l2 5M4 9a2.7 2.7 0 0 0 5.3 0 2.7 2.7 0 0 0 5.4 0 2.7 2.7 0 0 0 5.3 0M5 10v10h14V10M10 20v-5h4v5"
        }
        // two speech bubbles, one bowing lower
        Route::Unggah => "M4 5h9v6H8l-3 3v-3H4zM11 13h9v5h-1v3l-3-3h-5z",
        // two waves side by side
        Route::Pairs => "M3 12c2-4 4-4 6 0s4 4 6 0M13 12c2-4 4-4 6 0M3 18h7M14 18h7",
        Route::Hub => PLAY,
    };
    stroke(d)
}

/// Kak Nur (نور, light)): a speech bubble with a small spark.
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
