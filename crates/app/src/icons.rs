//! Small stroke icons, drawn inline so they follow `currentColor`.
//!
//! Most are single paths written here; the ones from Kabar's own icon set
//! (assets/icons/custom, reviewed 2026-10) are embedded from their SVG files.

use dioxus::prelude::*;

use crate::Screen;
use crate::play::Route;

pub fn nav(screen: Screen) -> Element {
    let d = match screen {
        // sun over a horizon
        Screen::Today => "M4 18h16M7 18a5 5 0 0 1 10 0M12 6V4M5.6 9.6 4.2 8.2M18.4 9.6l1.4-1.4",
        Screen::Journey | Screen::Lesson => return custom(JOURNEY_ISLAND),
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
        // headphones: Sinetron and Kabar Radio
        Screen::Listen => "M4 15v-3a8 8 0 0 1 16 0v3M4 15h3v5H4zM17 15h3v5h-3z",
        // two speech bubbles
        Screen::Chat => "M4 5h11v8H9l-3 3v-3H4zM15 9h5v8h-2v3l-3-3h-4v-2",
        Screen::Settings => GEAR,
        // a game controller
        Screen::Play => PLAY,
        Screen::Coach => return custom(COACH),
    };
    stroke(d)
}

/// Settings: a cog with eight teeth.
const GEAR: &str = "M12 9a3 3 0 1 0 0 6 3 3 0 1 0 0-6zM10.6 3h2.8l.5 2.3 1.7.9 2.2-.8 1.4 2.4-1.7 1.6v1.9l1.7 1.6-1.4 2.4-2.2-.8-1.7.9-.5 2.3h-2.8l-.5-2.3-1.7-.9-2.2.8-1.4-2.4 1.7-1.6v-1.9L4.8 7.8l1.4-2.4 2.2.8 1.7-.9z";

/// More: four tiles.
pub fn more() -> Element {
    stroke("M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z")
}

/// The gear that opens Settings.
pub fn gear() -> Element {
    stroke(GEAR)
}

/// Each Settings section's icon.
pub fn settings_section(section: crate::settings::Section) -> Element {
    use crate::settings::Section;
    let d = match section {
        // a speech bubble with a check
        Section::Learning => "M4 5h16v11H9l-5 4zM8.5 10.5l2.5 2.5 4.5-5",
        // a palette
        Section::Look => {
            "M12 3a9 9 0 1 0 0 18c1.5 0 2-1 2-2s-1-2 0-3 3 0 4-1 3-2 3-4a9 9 0 0 0-9-8zM7.5 11h.01M10 7.5h.01M14.5 7.5h.01"
        }
        Section::Coach => return custom(COACH),
        // a phone
        Section::Android => "M8 3h8a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM11 18h2",
        // a tablet held landscape
        Section::Tablet => "M3 6h18v12H3zM19 11v2",
        // a laptop
        Section::Mac => "M5 5h14v10H5zM2 18h20",
        // a download arrow into a tray
        Section::Storage => "M12 4v10M8 10l4 4 4-4M4 16v3h16v-3",
        Section::Account => return nav(Screen::Devices),
        // an i in a circle
        Section::About => "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v5M12 8h.01",
    };
    stroke(d)
}

/// Play: a rounded game pad with a cross and two buttons.
const PLAY: &str = "M7 8h10a4 4 0 0 1 4 4v1a4 4 0 0 1-7 2.6h-4A4 4 0 0 1 3 13v-1a4 4 0 0 1 4-4zM7 11v3M5.5 12.5h3M15.5 11.5h.01M17.5 13.5h.01";

/// Each Play game's card icon.
pub fn game(route: Route) -> Element {
    let d = match route {
        Route::Pagi => return custom(KABAR_PAGI),
        Route::Deck => return custom(BRIDGE_WORD),
        // magnifying glass
        Route::Detective => "M10.5 4a6.5 6.5 0 1 0 0 13 6.5 6.5 0 1 0 0-13zM15.5 15.5 20 20",
        Route::Alarm => return custom(FALSE_FRIEND),
        Route::Rupiah => return custom(RUPIAH),
        Route::Pasar => return custom(PASAR),
        Route::Unggah => return custom(RESPECT_METER),
        // two waves side by side
        Route::Pairs => "M3 12c2-4 4-4 6 0s4 4 6 0M13 12c2-4 4-4 6 0M3 18h7M14 18h7",
        Route::Hub => PLAY,
    };
    stroke(d)
}

/// Kak Nur: a speech bubble holding a heart.
pub fn coach() -> Element {
    custom(COACH)
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

/// Send: a paper plane, pointing forward (mirrored in Arabic by CSS).
pub fn send() -> Element {
    stroke("M4 12 20 4l-4 16-4-6zM12 14l8-10")
}

/// Listen: a speaker with two waves.
pub fn speaker() -> Element {
    stroke("M4 9h4l5-4v14l-5-4H4zM16 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11")
}

/// Replay as a dialogue: a play triangle in a circle.
pub fn play() -> Element {
    stroke("M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM10 8.5v7l6-3.5z")
}

/// Delete: a bin.
pub fn trash() -> Element {
    stroke("M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6")
}

/// An idea: a light bulb.
pub fn bulb() -> Element {
    stroke("M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2.1h5c0-.9.4-1.6 1-2.1A6 6 0 0 0 12 3z")
}

/// Private: a padlock.
pub fn lock() -> Element {
    stroke("M6 11h12v9H6zM8.5 11V8a3.5 3.5 0 0 1 7 0v3")
}

// Kabar's own icons (assets/icons/custom/README.md has what each one means).
const JOURNEY_ISLAND: &str = include_str!("../../../assets/icons/custom/journey-island.svg");
const COACH: &str = include_str!("../../../assets/icons/custom/coach.svg");
const KABAR_PAGI: &str = include_str!("../../../assets/icons/custom/kabar-pagi.svg");
const BRIDGE_WORD: &str = include_str!("../../../assets/icons/custom/bridge-word.svg");
const FALSE_FRIEND: &str = include_str!("../../../assets/icons/custom/false-friend.svg");
const RUPIAH: &str = include_str!("../../../assets/icons/custom/rupiah.svg");
const PASAR: &str = include_str!("../../../assets/icons/custom/pasar.svg");
const RESPECT_METER: &str = include_str!("../../../assets/icons/custom/respect-meter.svg");

/// The drawing inside one of the custom SVG files, without its `<svg>` wrapper.
fn svg_body(file: &'static str) -> &'static str {
    let open = file.find("<svg").and_then(|i| file[i..].find('>').map(|j| i + j + 1));
    let close = file.rfind("</svg>");
    match (open, close) {
        (Some(start), Some(end)) if start <= end => file[start..end].trim(),
        _ => "",
    }
}

/// A custom icon at the same size and stroke as the rest, so it sits in line with them.
fn custom(file: &'static str) -> Element {
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
            dangerous_inner_html: svg_body(file),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, &str); 8] = [
        ("journey-island", JOURNEY_ISLAND),
        ("coach", COACH),
        ("kabar-pagi", KABAR_PAGI),
        ("bridge-word", BRIDGE_WORD),
        ("false-friend", FALSE_FRIEND),
        ("rupiah", RUPIAH),
        ("pasar", PASAR),
        ("respect-meter", RESPECT_METER),
    ];

    #[test]
    fn custom_icons_are_24px_strokes_in_current_colour() {
        for (name, file) in ALL {
            assert!(file.contains(r#"viewBox="0 0 24 24""#), "{name}: not on the 24 grid");
            assert!(file.contains(r#"stroke="currentColor""#), "{name}: stroke is not currentColor");
            assert!(!file.contains('#'), "{name}: hard-coded colour");
            let body = svg_body(file);
            assert!(!body.is_empty(), "{name}: empty drawing");
            assert!(!body.contains("<svg") && !body.contains("</svg>"), "{name}: wrapper left in");
            assert!(!body.to_lowercase().contains("<script"), "{name}: script in an icon");
        }
    }

    #[test]
    fn svg_body_strips_only_the_wrapper() {
        let file =
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\">\n<path d=\"M1 1\"/>\n</svg>\n";
        assert_eq!(svg_body(file), "<path d=\"M1 1\"/>");
        assert_eq!(svg_body("not an svg"), "");
    }
}
