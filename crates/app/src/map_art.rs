//! Journey Map artwork (docs/VISUAL-ASSETS.md, prompt 5): the paper under
//! the sea and one landmark vignette per level. Exports come from
//! `scripts/assets/export_map.py`; they are embedded as `data:` URLs like
//! the Naskh font, so the map needs no file access on any platform.

use std::collections::HashMap;
use std::sync::LazyLock;

use base64::Engine as _;
use kabar_core::settings::Theme;
use kabar_core::theme::{self as palette, Palette, Scheme};

macro_rules! art {
    ($name:literal) => {
        include_bytes!(concat!("../../../assets/export/map/", $name, ".webp"))
    };
}

/// Level id → (512px vignette, 256px vignette).
const VIGNETTES: [(&str, &[u8], &[u8]); 13] = [
    ("bipa1", art!("jakarta"), art!("jakarta-sm")),
    ("bipa2", art!("bali"), art!("bali-sm")),
    ("bipa3", art!("lombok"), art!("lombok-sm")),
    ("bipa4", art!("sulawesi"), art!("sulawesi-sm")),
    ("bipa5", art!("kalimantan"), art!("kalimantan-sm")),
    ("bipa6", art!("sumatra"), art!("sumatra-sm")),
    ("bipa7", art!("raja-ampat"), art!("raja-ampat-sm")),
    ("j1", art!("surabaya"), art!("surabaya-sm")),
    ("j2", art!("madiun"), art!("madiun-sm")),
    ("j3", art!("solo"), art!("solo-sm")),
    ("j4", art!("yogyakarta"), art!("yogyakarta-sm")),
    ("j5", art!("semarang"), art!("semarang-sm")),
    ("j6", art!("kotagede"), art!("kotagede-sm")),
];

/// The 1024px tiles, drawn at 512 CSS px so the fibres stay sharp on
/// high-density screens.
const PAPER_LIGHT: &[u8] = art!("paper-light-1024");
const PAPER_DARK: &[u8] = art!("paper-dark-1024");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    /// 512px: the level header on tablets and the Mac.
    Large,
    /// 256px: map stickers and the phone route.
    Small,
}

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/webp;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

static URLS: LazyLock<HashMap<(&'static str, Size), String>> = LazyLock::new(|| {
    VIGNETTES
        .iter()
        .flat_map(|&(id, large, small)| {
            [((id, Size::Large), data_url(large)), ((id, Size::Small), data_url(small))]
        })
        .collect()
});

/// The landmark vignette of a level, as a `data:` URL.
pub fn vignette(level_id: &str, size: Size) -> Option<&'static str> {
    let id = VIGNETTES.iter().find(|v| v.0 == level_id)?.0;
    URLS.get(&(id, size)).map(String::as_str)
}

/// `--map-paper` for every theme: light paper for light themes, Merapi
/// charcoal paper for dark ones. `system` follows the OS like the colours.
pub fn paper_css() -> &'static str {
    static CSS: LazyLock<String> = LazyLock::new(|| {
        let (light, dark) = (data_url(PAPER_LIGHT), data_url(PAPER_DARK));
        let rule = |id: &str, p: &Palette| {
            let url = if p.scheme == Scheme::Dark { &dark } else { &light };
            format!(".app[data-theme=\"{id}\"] {{ --map-paper: url({url}); }}\n")
        };
        let mut css = rule("system", &palette::LIGHT);
        css.push_str(&format!(
            "@media (prefers-color-scheme: dark) {{ {}}}\n",
            rule("system", &palette::DARK)
        ));
        for theme in Theme::ALL {
            if let Some(p) = theme.palette() {
                css.push_str(&rule(theme.as_str(), p));
            }
        }
        css
    });
    &CSS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_of_both_routes_has_a_vignette() {
        for lang in ["id", "jv"] {
            for level in &crate::journey::curriculum(lang).unwrap().levels {
                for size in [Size::Large, Size::Small] {
                    let url = vignette(&level.id, size).unwrap_or_else(|| panic!("{} has no art", level.id));
                    assert!(url.starts_with("data:image/webp;base64,UklGR"), "{} is not WebP", level.id);
                }
            }
        }
        assert_eq!(vignette("nope", Size::Small), None);
    }

    #[test]
    fn dark_themes_get_dark_paper() {
        let css = paper_css();
        let light = data_url(PAPER_LIGHT);
        let dark = data_url(PAPER_DARK);
        for theme in Theme::ALL {
            let Some(p) = theme.palette() else { continue };
            let want = if p.scheme == Scheme::Dark { &dark } else { &light };
            assert!(css.contains(&format!(
                ".app[data-theme=\"{}\"] {{ --map-paper: url({want}); }}",
                theme.as_str()
            )));
        }
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
    }
}
