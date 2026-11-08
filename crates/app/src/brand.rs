//! The Kabar mark (rebrand K2, 2026-11-08): kawung, the Javanese batik motif
//! of four petals around a seed, one petal for each skill (listening,
//! speaking, reading, writing). The same shapes as the launcher icon
//! (assets/src/icon/svg/*.svg). Colours come from the theme: the petals are
//! `--indigo`, the brand colour, so the mark reads on every palette
//! (main.css `.brand-mark`).

use dioxus::prelude::*;

/// Petal centres and their tilt on a 100×100 grid: each long axis points at
/// the seed.
pub const PETALS: [(u8, u8, i8); 4] = [(31, 31, -45), (69, 31, 45), (31, 69, 45), (69, 69, -45)];
/// The small dot inside each petal, nearer its outer tip.
pub const DOTS: [(u8, u8); 4] = [(27, 27), (73, 27), (27, 73), (73, 73)];

/// The mark alone, decorative (the name beside it is the accessible text).
#[component]
pub fn Mark() -> Element {
    rsx! {
        svg {
            class: "brand-mark",
            view_box: "4 4 92 92",
            "aria-hidden": "true",
            for (x , y , r) in PETALS {
                ellipse {
                    class: "petal",
                    cx: "{x}",
                    cy: "{y}",
                    rx: "13",
                    ry: "23",
                    transform: "rotate({r} {x} {y})",
                }
            }
            for (x , y) in DOTS {
                circle { class: "dot", cx: "{x}", cy: "{y}", r: "3.2" }
            }
            rect {
                class: "seed",
                x: "43",
                y: "43",
                width: "14",
                height: "14",
                rx: "2",
                transform: "rotate(45 50 50)",
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The in-app mark and the launcher icon must not drift apart.
    #[test]
    fn mark_matches_the_launcher_icon() {
        let icon = include_str!("../../../assets/src/icon/svg/full.svg");
        for (x, y, r) in PETALS {
            let petal =
                format!(r#"<ellipse cx="{x}" cy="{y}" rx="13" ry="23" transform="rotate({r} {x} {y})""#);
            assert!(icon.contains(&petal), "full.svg lost petal {petal}");
        }
        for (x, y) in DOTS {
            assert!(
                icon.contains(&format!(r#"<circle cx="{x}" cy="{y}" r="3.2""#)),
                "full.svg lost dot {x},{y}"
            );
        }
    }
}
