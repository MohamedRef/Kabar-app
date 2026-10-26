//! Colour themes from the Java palette (SPEC §9). Each theme is a fixed set
//! of tokens that the app turns into CSS custom properties. Contrast is
//! checked in tests against WCAG AA, so a new theme can't ship unreadable
//! text, Bridge gold or word-status colours.

use crate::settings::Theme;

/// Light or dark, for the CSS `color-scheme` property (form controls,
/// scrollbars).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Light,
    Dark,
}

impl Scheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// One theme's colours, as `#rrggbb`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub scheme: Scheme,
    pub bg: &'static str,
    pub surface: &'static str,
    pub surface_2: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub line: &'static str,
    /// Primary action (sawah green).
    pub sawah: &'static str,
    pub dawn: &'static str,
    pub dawn_soft: &'static str,
    /// Selected state, headings, brand. Batik indigo in most themes.
    pub indigo: &'static str,
    /// Bridge gold: Arabic-origin words only.
    pub gold: &'static str,
    pub gold_soft: &'static str,
    pub focus: &'static str,
    /// Default pen colour and the Notebook paper.
    pub ink: &'static str,
    pub paper: &'static str,
    /// Word statuses in the Reader (new / learning / known).
    pub status_new: &'static str,
    pub status_learning: &'static str,
    pub status_known: &'static str,
    /// Stroke colour and opacity of the kawung background pattern.
    pub pattern: &'static str,
    pub pattern_opacity: f32,
}

/// Paper: warm off-white, the default light theme.
pub const LIGHT: Palette = Palette {
    scheme: Scheme::Light,
    bg: "#f6f1e7",
    surface: "#fffaf1",
    surface_2: "#efe7d8",
    text: "#23211f",
    muted: "#5d574e",
    line: "#ddd2bf",
    sawah: "#2f6b3a",
    dawn: "#b04a2f",
    dawn_soft: "#f4c9b5",
    indigo: "#2b3a70",
    gold: "#7a5600",
    gold_soft: "#f3e2b3",
    focus: "#2b3a70",
    ink: "#2b3a70",
    paper: "#fffdf8",
    status_new: "#c0502f",
    status_learning: "#3d5bb0",
    status_known: "#2f7a3e",
    pattern: "#2b3a70",
    pattern_opacity: 0.06,
};

/// Merapi: black volcanic sand, the default dark theme.
pub const DARK: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#171615",
    surface: "#211f1d",
    surface_2: "#2b2926",
    text: "#efe9df",
    muted: "#b3aa9c",
    line: "#3a3632",
    sawah: "#7cc08a",
    dawn: "#f2a07f",
    dawn_soft: "#5a2e22",
    indigo: "#a9b8ee",
    gold: "#e6be55",
    gold_soft: "#4a3b12",
    focus: "#a9b8ee",
    ink: "#c3cdf2",
    paper: "#1c1b19",
    status_new: "#f2a07f",
    status_learning: "#9fb1f0",
    status_known: "#7cc08a",
    pattern: "#a9b8ee",
    pattern_opacity: 0.05,
};

/// Malam: for studying at night. Very dark and warm, with no colour where
/// blue outweighs red, so the screen stays easy on the eyes before sleep.
pub const NIGHT: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#100c08",
    surface: "#18120c",
    surface_2: "#221a12",
    text: "#e3cfb2",
    muted: "#a8916f",
    line: "#33281c",
    sawah: "#b5b86a",
    dawn: "#e0956a",
    dawn_soft: "#4a2a18",
    indigo: "#cfa48e",
    gold: "#e8b83a",
    gold_soft: "#3d2e0a",
    focus: "#cfa48e",
    ink: "#d9b48a",
    paper: "#140f0a",
    status_new: "#e0956a",
    status_learning: "#c98f9a",
    status_known: "#b5b86a",
    pattern: "#cfa48e",
    pattern_opacity: 0.04,
};

/// Batik: deep indigo, where Bridge gold glows.
pub const BATIK: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#121830",
    surface: "#1a2140",
    surface_2: "#232b4f",
    text: "#ece9f4",
    muted: "#b0b3cf",
    line: "#323b63",
    sawah: "#86c995",
    dawn: "#f3a585",
    dawn_soft: "#55303a",
    indigo: "#b8c4f5",
    gold: "#f0c75e",
    gold_soft: "#4a3c18",
    focus: "#f0c75e",
    ink: "#d6ddfa",
    paper: "#161c38",
    status_new: "#f3a585",
    status_learning: "#b8c4f5",
    status_known: "#86c995",
    pattern: "#b8c4f5",
    pattern_opacity: 0.07,
};

/// Sawah: a fresh rice-terrace green light theme.
pub const SAWAH: Palette = Palette {
    scheme: Scheme::Light,
    bg: "#edf2e4",
    surface: "#f8fbf2",
    surface_2: "#e2ead5",
    text: "#1d271b",
    muted: "#4f5c4a",
    line: "#cbd8bb",
    sawah: "#2a6334",
    dawn: "#a8452b",
    dawn_soft: "#f2cdb6",
    indigo: "#2b3a70",
    gold: "#735100",
    gold_soft: "#f1e1b0",
    focus: "#2a6334",
    ink: "#25502c",
    paper: "#fafcf5",
    status_new: "#b8492b",
    status_learning: "#3b56a6",
    status_known: "#2a7038",
    pattern: "#2a6334",
    pattern_opacity: 0.06,
};

impl Theme {
    /// The palette for a concrete theme. `System` has none of its own: it
    /// follows the OS between [`LIGHT`] and [`DARK`] (see [`stylesheet`]).
    pub fn palette(self) -> Option<&'static Palette> {
        match self {
            Self::System => None,
            Self::Light => Some(&LIGHT),
            Self::Dark => Some(&DARK),
            Self::Night => Some(&NIGHT),
            Self::Batik => Some(&BATIK),
            Self::Sawah => Some(&SAWAH),
        }
    }
}

impl Palette {
    /// The tokens as CSS custom-property declarations.
    pub fn css_vars(&self) -> String {
        let vars = [
            ("bg", self.bg),
            ("surface", self.surface),
            ("surface-2", self.surface_2),
            ("text", self.text),
            ("muted", self.muted),
            ("line", self.line),
            ("sawah", self.sawah),
            ("dawn", self.dawn),
            ("dawn-soft", self.dawn_soft),
            ("indigo", self.indigo),
            ("gold", self.gold),
            ("gold-soft", self.gold_soft),
            ("focus", self.focus),
            ("ink", self.ink),
            ("paper", self.paper),
            ("status-new", self.status_new),
            ("status-learning", self.status_learning),
            ("status-known", self.status_known),
        ];
        let mut out = String::new();
        for (name, value) in vars {
            out.push_str(&format!("--{name}: {value}; "));
        }
        out.push_str(&format!("color-scheme: {};", self.scheme.as_str()));
        out
    }
}

/// CSS for every theme. The app root carries `data-theme="<id>"`; the
/// variables are also set on `:root` (via `:has`) so the page background
/// and the ink canvas match. `system` follows `prefers-color-scheme`.
pub fn stylesheet() -> String {
    let rule = |id: &str, p: &Palette| {
        format!(":root:has(.app[data-theme=\"{id}\"]), .app[data-theme=\"{id}\"] {{ {} }}\n", p.css_vars())
    };
    let mut css = rule("system", &LIGHT);
    css.push_str(&format!("@media (prefers-color-scheme: dark) {{\n  {}}}\n", rule("system", &DARK)));
    for theme in Theme::ALL {
        if let Some(p) = theme.palette() {
            css.push_str(&rule(theme.as_str(), p));
        }
    }
    css
}

/// WCAG 2.x contrast ratio between two `#rrggbb` colours (1.0 to 21.0).
pub fn contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn rgb(hex: &str) -> [u8; 3] {
    let hex = hex.trim_start_matches('#');
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("palette colours are #rrggbb");
    [channel(0), channel(2), channel(4)]
}

fn luminance(hex: &str) -> f64 {
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let [r, g, b] = rgb(hex);
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<(Theme, &'static Palette)> {
        Theme::ALL.into_iter().filter_map(|t| t.palette().map(|p| (t, p))).collect()
    }

    #[test]
    fn contrast_matches_known_values() {
        assert!((contrast("#000000", "#ffffff") - 21.0).abs() < 1e-9);
        assert!((contrast("#777777", "#ffffff") - 4.48).abs() < 0.01);
    }

    #[test]
    fn every_theme_meets_wcag_aa() {
        for (theme, p) in all() {
            // (what, foreground, background, minimum ratio)
            let checks = [
                ("text on bg", p.text, p.bg, 7.0),
                ("text on surface", p.text, p.surface, 7.0),
                ("text on surface-2", p.text, p.surface_2, 4.5),
                ("muted on bg", p.muted, p.bg, 4.5),
                ("muted on surface", p.muted, p.surface, 4.5),
                ("primary button label", p.surface, p.sawah, 4.5),
                ("selected button label", p.surface, p.indigo, 4.5),
                ("current nav item", p.indigo, p.surface_2, 4.5),
                ("Bridge badge", p.gold, p.gold_soft, 4.5),
                ("Bridge gold on surface", p.gold, p.surface, 4.5),
                ("pill text", p.text, p.dawn_soft, 4.5),
                ("ok text", p.sawah, p.surface, 4.5),
                ("warning text", p.dawn, p.surface, 4.5),
                ("focus ring", p.focus, p.bg, 3.0),
                ("ink on paper", p.ink, p.paper, 4.5),
                ("status new", p.status_new, p.surface, 3.0),
                ("status learning", p.status_learning, p.surface, 3.0),
                ("status known", p.status_known, p.surface, 3.0),
            ];
            for (what, fg, bg, min) in checks {
                let ratio = contrast(fg, bg);
                assert!(ratio >= min, "{}: {what} is {ratio:.2}:1, needs {min}:1", theme.as_str());
            }
        }
    }

    #[test]
    fn bridge_gold_and_statuses_stay_distinct() {
        for (theme, p) in all() {
            let marks = [p.gold, p.status_new, p.status_learning, p.status_known];
            for (i, a) in marks.iter().enumerate() {
                for b in &marks[i + 1..] {
                    assert_ne!(a, b, "{}: two reader marks share a colour", theme.as_str());
                }
            }
            assert_ne!(p.gold, p.indigo, "{}: Bridge gold must not double as the accent", theme.as_str());
        }
    }

    #[test]
    fn night_has_no_blue_heavy_colour() {
        let p = &NIGHT;
        let colours = [
            p.bg,
            p.surface,
            p.surface_2,
            p.text,
            p.muted,
            p.line,
            p.sawah,
            p.dawn,
            p.dawn_soft,
            p.indigo,
            p.gold,
            p.gold_soft,
            p.focus,
            p.ink,
            p.paper,
            p.status_new,
            p.status_learning,
            p.status_known,
            p.pattern,
        ];
        for c in colours {
            let [r, _, b] = rgb(c);
            assert!(b <= r, "night colour {c} is blue-heavy");
        }
    }

    #[test]
    fn stylesheet_has_a_rule_per_theme_and_system_follows_the_os() {
        let css = stylesheet();
        for theme in Theme::ALL {
            assert!(css.contains(&format!(".app[data-theme=\"{}\"]", theme.as_str())), "{}", theme.as_str());
        }
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert!(css.contains("--status-known"));
        assert!(!css.contains("{{"));
    }
}
