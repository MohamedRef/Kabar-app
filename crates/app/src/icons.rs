//! Small stroke icons, drawn inline so they follow `currentColor`.

use dioxus::prelude::*;

use crate::Screen;

pub fn nav(screen: Screen) -> Element {
    let d = match screen {
        // sun over a horizon
        Screen::Today => "M4 18h16M7 18a5 5 0 0 1 10 0M12 6V4M5.6 9.6 4.2 8.2M18.4 9.6l1.4-1.4",
        // route between two islands
        Screen::Journey => "M5 19c3-1 3-5 7-6s4-5 7-6M5 19a2 2 0 1 0 0-.01M19 7a2 2 0 1 0 0-.01",
        // open book
        Screen::Reader => "M3 6c3-1 6-1 9 1 3-2 6-2 9-1v12c-3-1-6-1-9 1-3-2-6-2-9-1zM12 7v12",
        // stacked cards
        Screen::Review => "M7 4h11v13H7zM4 7v13h11",
        // pen nib
        Screen::Studio => "M4 20l4-1 10-10-3-3L5 16zM13 7l3 3",
        // stylus with signal
        Screen::PenLab => "M5 19l2-6 8-8 3 3-8 8zM16 16c1.5 0 3 1 3 3",
        // phone beside a laptop
        Screen::Devices => "M3 6h12v9H3zM1 18h15M18 9h4v11h-4z",
        // sliders
        Screen::Settings => "M4 7h10M18 7h2M4 17h4M12 17h8M14 5v4M8 15v4",
    };
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
