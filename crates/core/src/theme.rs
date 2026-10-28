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
    /// The word sheet (the card a tapped word opens): a raised, tinted
    /// surface that never blends into the page or its cards.
    pub sheet: &'static str,
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
    /// Journey Map: sea, its wave lines, Indonesia, its coastline, the
    /// neighbouring countries, and the bed under the railway ties.
    pub map_sea: &'static str,
    pub map_sea_ink: &'static str,
    pub map_land: &'static str,
    pub map_land_edge: &'static str,
    pub map_land_other: &'static str,
    pub map_rail_bed: &'static str,
    /// Stroke colour and opacity of the kawung background pattern.
    pub pattern: &'static str,
    pub pattern_opacity: f32,
    /// Word classes and word parts (`kabar_core::grammar::word`).
    pub words: WordColors,
}

/// The fixed colours for kinds of words and pieces of words. The same
/// colour marks a verb in the text, in its breakdown and in the grammar
/// notes. Roots take their word's class colour; clitics (-nya, -ku) share
/// the function-word colour, since they are pronouns and particles glued on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WordColors {
    pub verb: &'static str,
    pub noun: &'static str,
    pub adjective: &'static str,
    /// Pronouns, prepositions, conjunctions, particles, time words.
    pub function: &'static str,
    pub prefix: &'static str,
    pub suffix: &'static str,
    /// ke-…-an, peN-…-an: both halves.
    pub confix: &'static str,
}

/// Paper: warm off-white, the default light theme.
pub const LIGHT: Palette = Palette {
    scheme: Scheme::Light,
    bg: "#f6f1e7",
    surface: "#fffaf1",
    surface_2: "#efe7d8",
    sheet: "#e1e6f6",
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
    map_sea: "#dbe7e9",
    map_sea_ink: "#b9cfd3",
    map_land: "#d3e0bd",
    map_land_edge: "#a9bf8f",
    map_land_other: "#e9e3d6",
    map_rail_bed: "#fffaf1",
    pattern: "#2b3a70",
    pattern_opacity: 0.06,
    words: WordColors {
        verb: "#a61e4d",
        noun: "#1f5fa6",
        adjective: "#6b3fa0",
        function: "#5d574e",
        prefix: "#a84300",
        suffix: "#0b7062",
        confix: "#4d6b0f",
    },
};

/// Merapi: black volcanic sand, the default dark theme.
pub const DARK: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#171615",
    surface: "#211f1d",
    surface_2: "#2b2926",
    sheet: "#272d45",
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
    map_sea: "#172227",
    map_sea_ink: "#23343b",
    map_land: "#2b3a2b",
    map_land_edge: "#4b6446",
    map_land_other: "#2a2724",
    map_rail_bed: "#171615",
    pattern: "#a9b8ee",
    pattern_opacity: 0.05,
    words: WordColors {
        verb: "#f28fae",
        noun: "#8fbcf2",
        adjective: "#c6a6f0",
        function: "#b3aa9c",
        prefix: "#f39f6c",
        suffix: "#6fd0bd",
        confix: "#b9d47a",
    },
};

/// Gerhana (eclipse): true black for OLED screens. Every background is
/// `#000000`, so those pixels switch off: cards are told apart by thin
/// outlines (`line`) instead of grey surfaces, and the batik pattern and map
/// paper are left out. Text is a soft off-white, which smears less than pure
/// white when an OLED screen scrolls. Accents and word colours are Merapi's,
/// which read even better on black.
pub const BLACK: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#000000",
    surface: "#000000",
    surface_2: "#161616",
    sheet: "#141a30",
    text: "#ebe7e0",
    muted: "#a9a297",
    line: "#383430",
    sawah: "#7cc08a",
    dawn: "#f2a07f",
    dawn_soft: "#4d271c",
    indigo: "#a9b8ee",
    gold: "#e6be55",
    gold_soft: "#3d300c",
    focus: "#a9b8ee",
    ink: "#c3cdf2",
    paper: "#000000",
    status_new: "#f2a07f",
    status_learning: "#9fb1f0",
    status_known: "#7cc08a",
    map_sea: "#000000",
    map_sea_ink: "#14232a",
    map_land: "#13261a",
    map_land_edge: "#4b6446",
    map_land_other: "#14120f",
    map_rail_bed: "#000000",
    pattern: "#a9b8ee",
    pattern_opacity: 0.0,
    words: DARK.words,
};

/// Malam: for studying at night. Very dark and warm, with no colour where
/// blue outweighs red, so the screen stays easy on the eyes before sleep.
pub const NIGHT: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#100c08",
    surface: "#18120c",
    surface_2: "#221a12",
    sheet: "#3a2a1d",
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
    map_sea: "#17120c",
    map_sea_ink: "#261e15",
    map_land: "#2b2716",
    map_land_edge: "#4f472a",
    map_land_other: "#1f1912",
    map_rail_bed: "#100c08",
    pattern: "#cfa48e",
    pattern_opacity: 0.04,
    words: WordColors {
        verb: "#f0909a",
        noun: "#a8c89a",
        adjective: "#d6a6d6",
        function: "#a8916f",
        prefix: "#eba25e",
        suffix: "#d2d27e",
        confix: "#c4a690",
    },
};

/// Batik: deep indigo, where Bridge gold glows.
pub const BATIK: Palette = Palette {
    scheme: Scheme::Dark,
    bg: "#121830",
    surface: "#1a2140",
    surface_2: "#232b4f",
    sheet: "#2a3466",
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
    map_sea: "#0f1a35",
    map_sea_ink: "#1d2a50",
    map_land: "#25392f",
    map_land_edge: "#3f5a4e",
    map_land_other: "#1f2340",
    map_rail_bed: "#121830",
    pattern: "#b8c4f5",
    pattern_opacity: 0.07,
    words: WordColors {
        verb: "#f79ab6",
        noun: "#96c4f7",
        adjective: "#d2b4f7",
        function: "#b0b3cf",
        prefix: "#f8a878",
        suffix: "#78d8c4",
        confix: "#c2dc84",
    },
};

/// Sawah: a fresh rice-terrace green light theme.
pub const SAWAH: Palette = Palette {
    scheme: Scheme::Light,
    bg: "#edf2e4",
    surface: "#f8fbf2",
    surface_2: "#e2ead5",
    sheet: "#dee5f5",
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
    map_sea: "#d6e6e4",
    map_sea_ink: "#b3cfcb",
    map_land: "#cfe0b5",
    map_land_edge: "#9fbb83",
    map_land_other: "#e6e6d6",
    map_rail_bed: "#f8fbf2",
    pattern: "#2a6334",
    pattern_opacity: 0.06,
    words: WordColors {
        verb: "#a01d4a",
        noun: "#1d5a9e",
        adjective: "#673c9a",
        function: "#4f5c4a",
        prefix: "#a84300",
        suffix: "#0a6a5d",
        confix: "#46640c",
    },
};

impl Theme {
    /// The palette for a concrete theme. `System` and `SystemBlack` have
    /// none of their own: they follow the OS (see [`Theme::follows_os`]).
    pub fn palette(self) -> Option<&'static Palette> {
        match self {
            Self::System | Self::SystemBlack => None,
            Self::Light => Some(&LIGHT),
            Self::Dark => Some(&DARK),
            Self::Black => Some(&BLACK),
            Self::Night => Some(&NIGHT),
            Self::Batik => Some(&BATIK),
            Self::Sawah => Some(&SAWAH),
        }
    }

    /// The (day, night) palettes of a theme that follows the OS.
    pub fn follows_os(self) -> Option<(&'static Palette, &'static Palette)> {
        match self {
            Self::System => Some((&LIGHT, &DARK)),
            Self::SystemBlack => Some((&LIGHT, &BLACK)),
            _ => None,
        }
    }

    /// The concrete theme to show when the platform knows the device's dark
    /// mode (`Some`): a System theme takes its day or night side. With
    /// `None` it stays as is and CSS `prefers-color-scheme` decides.
    pub fn resolved(self, device_dark: Option<bool>) -> Theme {
        match (self, device_dark) {
            (Self::System, Some(dark)) => {
                if dark {
                    Self::Dark
                } else {
                    Self::Light
                }
            }
            (Self::SystemBlack, Some(dark)) => {
                if dark {
                    Self::Black
                } else {
                    Self::Light
                }
            }
            _ => self,
        }
    }

    /// The palettes shown by day and by night: the same one for a fixed theme.
    pub fn day_and_night(self) -> (&'static Palette, &'static Palette) {
        match (self.palette(), self.follows_os()) {
            (Some(p), _) => (p, p),
            (None, Some(pair)) => pair,
            (None, None) => (&LIGHT, &DARK),
        }
    }
}

/// CSS for every theme from one rule maker: `rule(id, palette)` returns the
/// rules for `.app[data-theme="<id>"]`. Themes that follow the OS get their
/// day rule plus their night rule inside `prefers-color-scheme: dark`. Every
/// per-theme stylesheet goes through here, so a new theme can't be missed.
pub fn rules_for_all(rule: impl Fn(&str, &Palette) -> String) -> String {
    let mut css = String::new();
    for theme in Theme::ALL {
        let id = theme.as_str();
        match (theme.palette(), theme.follows_os()) {
            (Some(p), _) => css.push_str(&rule(id, p)),
            (None, Some((day, night))) => {
                css.push_str(&rule(id, day));
                css.push_str(&format!("@media (prefers-color-scheme: dark) {{\n  {}}}\n", rule(id, night)));
            }
            (None, None) => unreachable!("{id} has neither a palette nor an OS pair"),
        }
    }
    css
}

impl Palette {
    /// True black (OLED): textures and patterns stay off so the background
    /// pixels stay off too.
    pub fn is_true_black(&self) -> bool {
        self.bg == "#000000"
    }

    /// The tokens as CSS custom-property declarations.
    pub fn css_vars(&self) -> String {
        let vars = [
            ("bg", self.bg),
            ("surface", self.surface),
            ("surface-2", self.surface_2),
            ("sheet", self.sheet),
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
            ("sea", self.map_sea),
            ("sea-ink", self.map_sea_ink),
            ("land", self.map_land),
            ("land-edge", self.map_land_edge),
            ("land-other", self.map_land_other),
            ("rail-bed", self.map_rail_bed),
            ("wc-verb", self.words.verb),
            ("wc-noun", self.words.noun),
            ("wc-adj", self.words.adjective),
            ("wc-func", self.words.function),
            ("wp-prefix", self.words.prefix),
            ("wp-suffix", self.words.suffix),
            ("wp-confix", self.words.confix),
            ("wp-clitic", self.words.function),
            ("wp-root", self.text),
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
/// and the ink canvas match. `system` and `system-black` follow
/// `prefers-color-scheme`.
pub fn stylesheet() -> String {
    rules_for_all(|id, p| {
        format!(":root:has(.app[data-theme=\"{id}\"]), .app[data-theme=\"{id}\"] {{ {} }}\n", p.css_vars())
    })
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
                ("verb", p.words.verb, p.surface_2, 4.5),
                ("noun", p.words.noun, p.surface_2, 4.5),
                ("adjective", p.words.adjective, p.surface_2, 4.5),
                ("function word", p.words.function, p.surface_2, 4.5),
                ("prefix", p.words.prefix, p.surface_2, 4.5),
                ("suffix", p.words.suffix, p.surface_2, 4.5),
                ("confix", p.words.confix, p.surface_2, 4.5),
                ("text on sheet", p.text, p.sheet, 7.0),
                ("muted on sheet", p.muted, p.sheet, 4.5),
                ("headword on sheet", p.indigo, p.sheet, 4.5),
                ("Bridge gold on sheet", p.gold, p.sheet, 4.5),
                ("verb on sheet", p.words.verb, p.sheet, 4.5),
                ("noun on sheet", p.words.noun, p.sheet, 4.5),
                ("adjective on sheet", p.words.adjective, p.sheet, 4.5),
                ("function word on sheet", p.words.function, p.sheet, 4.5),
                ("prefix on sheet", p.words.prefix, p.sheet, 4.5),
                ("suffix on sheet", p.words.suffix, p.sheet, 4.5),
                ("confix on sheet", p.words.confix, p.sheet, 4.5),
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
    fn word_colours_are_told_apart() {
        // Euclidean RGB distance: far enough to name the colour at a glance.
        let dist = |a: &str, b: &str| {
            let (a, b) = (rgb(a), rgb(b));
            (0..3).map(|i| (f64::from(a[i]) - f64::from(b[i])).powi(2)).sum::<f64>().sqrt()
        };
        for (theme, p) in all() {
            let w = &p.words;
            let colours = [
                ("verb", w.verb),
                ("noun", w.noun),
                ("adjective", w.adjective),
                ("function", w.function),
                ("prefix", w.prefix),
                ("suffix", w.suffix),
                ("confix", w.confix),
                ("Bridge gold", p.gold),
            ];
            for (i, (an, a)) in colours.iter().enumerate() {
                for (bn, b) in &colours[i + 1..] {
                    let d = dist(a, b);
                    assert!(d >= 38.0, "{}: {an} and {bn} are too close ({d:.0})", theme.as_str());
                }
            }
        }
    }

    #[test]
    fn word_sheet_stands_out() {
        // The sheet floats over the page and its cards: it must read as a
        // different layer at a glance, never as one more card.
        let dist = |a: &str, b: &str| {
            let (a, b) = (rgb(a), rgb(b));
            (0..3).map(|i| (f64::from(a[i]) - f64::from(b[i])).powi(2)).sum::<f64>().sqrt()
        };
        for (theme, p) in all() {
            for (what, under) in [("page", p.bg), ("card", p.surface), ("chip", p.surface_2)] {
                let d = dist(p.sheet, under);
                assert!(d >= 24.0, "{}: sheet vs {what} too close ({d:.0})", theme.as_str());
            }
            if p.scheme == Scheme::Dark {
                assert!(
                    luminance(p.sheet) > luminance(p.surface),
                    "{}: a dark sheet sits above its cards",
                    theme.as_str()
                );
            }
        }
    }

    #[test]
    fn map_keeps_land_and_sea_apart() {
        for (theme, p) in all() {
            // Land and sea differ mostly in hue (green vs blue), so the
            // coastline carries the shape: it must stand out from the sea.
            assert_ne!(p.map_land, p.map_sea, "{}: land vs sea", theme.as_str());
            assert!(contrast(p.map_land_edge, p.map_sea) >= 1.5, "{}: coastline", theme.as_str());
            assert_ne!(p.map_land, p.map_land_other, "{}: Indonesia vs neighbours", theme.as_str());
        }
    }

    #[test]
    fn night_has_no_blue_heavy_colour() {
        let p = &NIGHT;
        let colours = [
            p.bg,
            p.surface,
            p.surface_2,
            p.sheet,
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
            p.map_sea,
            p.map_sea_ink,
            p.map_land,
            p.map_land_edge,
            p.map_land_other,
            p.map_rail_bed,
            p.words.verb,
            p.words.noun,
            p.words.adjective,
            p.words.function,
            p.words.prefix,
            p.words.suffix,
            p.words.confix,
        ];
        for c in colours {
            let [r, _, b] = rgb(c);
            assert!(b <= r, "night colour {c} is blue-heavy");
        }
    }

    #[test]
    fn black_switches_oled_pixels_off() {
        let p = &BLACK;
        for (what, c) in [
            ("bg", p.bg),
            ("surface", p.surface),
            ("paper", p.paper),
            ("sea", p.map_sea),
            ("rail bed", p.map_rail_bed),
        ] {
            assert_eq!(c, "#000000", "black: {what} must be true black");
        }
        assert_eq!(p.pattern_opacity, 0.0, "black: no batik pattern on the background");
        // With surface == bg, cards are drawn by their outline alone.
        assert!(contrast(p.line, p.bg) >= 1.5, "black: card outlines must show");
        assert_eq!(Theme::Black.palette(), Some(&BLACK));
        assert_eq!(Theme::SystemBlack.follows_os(), Some((&LIGHT, &BLACK)));
        assert_eq!(Theme::Black.day_and_night(), (&BLACK, &BLACK));
    }

    #[test]
    fn os_themes_resolve_to_their_side_when_the_device_says() {
        assert_eq!(Theme::System.resolved(Some(true)), Theme::Dark);
        assert_eq!(Theme::SystemBlack.resolved(Some(true)), Theme::Black);
        assert_eq!(Theme::SystemBlack.resolved(Some(false)), Theme::Light);
        assert_eq!(Theme::System.resolved(None), Theme::System);
        assert_eq!(Theme::Night.resolved(Some(false)), Theme::Night);
        for theme in Theme::ALL {
            for dark in [true, false] {
                let (day, night) = theme.day_and_night();
                let want = if dark { night } else { day };
                assert_eq!(theme.resolved(Some(dark)).palette(), Some(want), "{}", theme.as_str());
            }
        }
    }

    #[test]
    fn every_theme_has_a_palette_or_follows_the_os() {
        for theme in Theme::ALL {
            assert!(theme.palette().is_some() != theme.follows_os().is_some(), "{}", theme.as_str());
        }
    }

    #[test]
    fn stylesheet_has_a_rule_per_theme_and_system_follows_the_os() {
        let css = stylesheet();
        for theme in Theme::ALL {
            assert!(css.contains(&format!(".app[data-theme=\"{}\"]", theme.as_str())), "{}", theme.as_str());
        }
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert_eq!(css.matches("@media (prefers-color-scheme: dark)").count(), 2);
        assert!(css.contains("--status-known"));
        assert!(css.contains("--wc-verb") && css.contains("--wp-suffix"));
        assert!(!css.contains("{{"));
    }
}
