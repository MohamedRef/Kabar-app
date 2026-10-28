//! Supporting illustrations (docs/VISUAL-ASSETS.md, prompt 7), drawn in code
//! by `scripts/assets/draw_misc.py`: the first-launch hero, the empty states,
//! the lesson-end gong and notebook paper. Embedded as `data:` URLs like the
//! map art, so they need no file access on any platform.

use std::collections::HashMap;
use std::sync::LazyLock;

use base64::Engine as _;
use dioxus::prelude::*;
use kabar_core::theme::{self as palette, Palette, Scheme};

macro_rules! art {
    ($name:literal) => {
        include_bytes!(concat!("../../../assets/export/misc/", $name, ".webp"))
    };
}

/// Every picture and where it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Art {
    /// Onboarding, wide windows.
    HeroLandscape,
    /// Onboarding, phones.
    HeroPortrait,
    /// The Suitcase before the first souvenir.
    EmptySuitcase,
    /// Review, when nothing is due.
    AllReviewed,
    /// Account, while sync is offline.
    Offline,
    /// The end of a lesson.
    Gong,
}

const ARTS: [(Art, &[u8]); 6] = [
    (Art::HeroLandscape, art!("hero-landscape-half")),
    (Art::HeroPortrait, art!("hero-portrait-half")),
    (Art::EmptySuitcase, art!("empty-suitcase-sm")),
    (Art::AllReviewed, art!("all-reviewed-sm")),
    (Art::Offline, art!("offline-sm")),
    (Art::Gong, art!("celebrate-gong-sm")),
];

const NOTEBOOK_PAPER: &[u8] = art!("notebook-paper-512");

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/webp;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

static URLS: LazyLock<HashMap<Art, String>> =
    LazyLock::new(|| ARTS.iter().map(|&(a, bytes)| (a, data_url(bytes))).collect());

pub fn url(art: Art) -> &'static str {
    &URLS[&art]
}

/// A decorative picture; the words next to it carry the meaning.
#[component]
pub fn Picture(art: Art, class: &'static str) -> Element {
    rsx! {
        img { class: "misc-art {class}", src: "{url(art)}", alt: "", aria_hidden: "true", draggable: "false" }
    }
}

/// `--notebook-paper` for light themes; dark themes keep their own surface.
pub fn paper_css() -> &'static str {
    static CSS: LazyLock<String> = LazyLock::new(|| {
        let paper = data_url(NOTEBOOK_PAPER);
        let rule = |id: &str, p: &Palette| {
            let value = if p.scheme == Scheme::Dark { "none".to_string() } else { format!("url({paper})") };
            format!(".app[data-theme=\"{id}\"] {{ --notebook-paper: {value}; }}\n")
        };
        palette::rules_for_all(rule)
    });
    &CSS
}

#[cfg(test)]
mod tests {
    use super::*;
    use kabar_core::settings::Theme;

    #[test]
    fn every_picture_is_webp() {
        for (art, _) in ARTS {
            assert!(url(art).starts_with("data:image/webp;base64,UklGR"), "{art:?}");
        }
    }

    #[test]
    fn only_light_themes_get_notebook_paper() {
        let css = paper_css();
        for theme in Theme::ALL {
            let Some(p) = theme.palette() else { continue };
            let rule = format!(".app[data-theme=\"{}\"] {{ --notebook-paper: none; }}", theme.as_str());
            assert_eq!(css.contains(&rule), p.scheme == Scheme::Dark, "{}", theme.as_str());
        }
    }
}
