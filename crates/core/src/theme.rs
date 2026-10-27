//! Colour themes. Each theme is a fixed set of tokens that the app turns into
//! CSS custom properties. Contrast is checked in tests against WCAG AA, so a
//! new theme can't ship unreadable text.
// adapted from kabar-app main: crates/core/src/theme.rs

/// Light or dark, for the CSS `color-scheme` property (form controls,
/// scrollbars).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub scheme: Scheme,
    pub bg: &'static str,
    pub surface: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub line: &'static str,
    /// Bile ochre: brand, links, the current nav item.
    pub accent: &'static str,
    pub accent_soft: &'static str,
    /// Liver-tissue green: right answers, "ready".
    pub good: &'static str,
    pub good_soft: &'static str,
    pub bad: &'static str,
    pub bad_soft: &'static str,
}

impl Palette {
    /// `(css variable, value)` for every token.
    pub fn tokens(&self) -> [(&'static str, &'static str); 11] {
        [
            ("--bg", self.bg),
            ("--surface", self.surface),
            ("--text", self.text),
            ("--muted", self.muted),
            ("--line", self.line),
            ("--accent", self.accent),
            ("--accent-soft", self.accent_soft),
            ("--good", self.good),
            ("--good-soft", self.good_soft),
            ("--bad", self.bad),
            ("--bad-soft", self.bad_soft),
        ]
    }

    /// The tokens as CSS declarations, plus `color-scheme`.
    pub fn css(&self) -> String {
        let mut out = String::new();
        for (name, value) in self.tokens() {
            out.push_str(name);
            out.push(':');
            out.push_str(value);
            out.push(';');
        }
        out.push_str("color-scheme:");
        out.push_str(self.scheme.as_str());
        out.push(';');
        out
    }
}

/// Paper: a faint green-grey page, the default light theme.
pub const PAPER: Palette = Palette {
    scheme: Scheme::Light,
    bg: "#f6f7f4",
    surface: "#ffffff",
    text: "#1d2420",
    muted: "#5b665f",
    line: "#d9ded8",
    accent: "#8a5e0f",
    accent_soft: "#f3e7cc",
    good: "#2f6b4f",
    good_soft: "#e1efe7",
    bad: "#a13a2c",
    bad_soft: "#f6e1dc",
};

/// Night: the default dark theme.
pub const NIGHT: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#141917",
    surface: "#1b211e",
    text: "#e6ebe7",
    muted: "#9aa69f",
    line: "#2f3833",
    accent: "#e0b25a",
    accent_soft: "#3a2f18",
    good: "#7cc7a0",
    good_soft: "#1d3329",
    bad: "#ee8f80",
    bad_soft: "#3d2320",
};

/// Night shift: for studying late. Very dark and warm, with no colour where
/// blue outweighs red, so the screen stays easy on the eyes before sleep.
pub const NIGHT_SHIFT: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#110d0a",
    surface: "#1a1511",
    text: "#e8dccb",
    muted: "#a8998a",
    line: "#3a3029",
    accent: "#e3a552",
    accent_soft: "#3a2a16",
    good: "#c2b56c",
    good_soft: "#2a2614",
    bad: "#ef9373",
    bad_soft: "#3d1f17",
};

/// What the learner picked in Settings › Look.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    /// Paper by day, Night when the device is dark.
    #[default]
    System,
    Paper,
    Night,
    NightShift,
}

impl Theme {
    pub const ALL: [Self; 4] = [Self::System, Self::Paper, Self::Night, Self::NightShift];

    /// Stored value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Paper => "paper",
            Self::Night => "night",
            Self::NightShift => "night-shift",
        }
    }

    pub fn parse(s: &str) -> Self {
        Self::ALL.into_iter().find(|t| t.as_str() == s).unwrap_or_default()
    }

    /// The fixed palette, or `None` for System (it follows the device).
    pub fn palette(self) -> Option<&'static Palette> {
        match self {
            Self::System => None,
            Self::Paper => Some(&PAPER),
            Self::Night => Some(&NIGHT),
            Self::NightShift => Some(&NIGHT_SHIFT),
        }
    }

    /// The CSS that applies this theme to the whole page.
    pub fn css(self) -> String {
        match self.palette() {
            Some(p) => format!(":root{{{}}}", p.css()),
            None => format!(
                ":root{{{}}}@media (prefers-color-scheme: dark){{:root{{{}}}}}",
                PAPER.css(),
                NIGHT.css()
            ),
        }
    }
}

/// WCAG relative luminance of `#rrggbb`.
pub fn luminance(hex: &str) -> f64 {
    let (r, g, b) = rgb(hex);
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio between two `#rrggbb` colours (1 to 21).
pub fn contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn rgb(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim_start_matches('#');
    let p = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(0);
    (p(0), p(2), p(4))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, &Palette); 3] = [("paper", &PAPER), ("night", &NIGHT), ("night-shift", &NIGHT_SHIFT)];

    #[test]
    fn every_theme_meets_wcag_aa_for_text() {
        for (name, p) in ALL {
            let pairs = [
                ("text on bg", p.text, p.bg),
                ("text on surface", p.text, p.surface),
                ("muted on bg", p.muted, p.bg),
                ("muted on surface", p.muted, p.surface),
                ("accent on surface", p.accent, p.surface),
                ("accent on accent-soft", p.accent, p.accent_soft),
                ("good on good-soft", p.good, p.good_soft),
                ("bad on bad-soft", p.bad, p.bad_soft),
                ("text on accent-soft", p.text, p.accent_soft),
            ];
            for (what, fg, bg) in pairs {
                let c = contrast(fg, bg);
                assert!(c >= 4.5, "{name}: {what} is {c:.2}:1, below 4.5:1");
            }
        }
    }

    #[test]
    fn night_shift_has_no_blue_light() {
        for (name, hex) in NIGHT_SHIFT.tokens() {
            let (r, _, b) = rgb(hex);
            assert!(b <= r, "night shift {name} {hex} has more blue than red");
        }
    }

    #[test]
    fn themes_round_trip_and_unknown_is_system() {
        for t in Theme::ALL {
            assert_eq!(Theme::parse(t.as_str()), t);
        }
        assert_eq!(Theme::parse("neon"), Theme::System);
        assert!(Theme::System.css().contains("prefers-color-scheme"));
        assert!(Theme::NightShift.css().starts_with(":root{--bg:#110d0a;"));
    }

    #[test]
    fn contrast_matches_known_values() {
        assert!((contrast("#000000", "#ffffff") - 21.0).abs() < 1e-9);
        assert!((contrast("#777777", "#777777") - 1.0).abs() < 1e-9);
    }
}
