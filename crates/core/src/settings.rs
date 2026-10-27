//! User settings. Stored and synced per key (last-writer-wins by HLC), so
//! changing the theme on the tablet never overwrites the language picked on
//! the phone.

use serde::{Deserialize, Serialize};

use crate::locale::UiLanguage;
use crate::srs::FsrsParams;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follows the OS: [`Theme::Light`] by day, [`Theme::Dark`] by night.
    #[default]
    System,
    /// Paper: warm off-white.
    Light,
    /// Merapi: black volcanic sand.
    Dark,
    /// Malam: very dark and warm, low blue light, for studying at night.
    Night,
    /// Batik: deep indigo.
    Batik,
    /// Sawah: rice-terrace green.
    Sawah,
}

impl Theme {
    pub const ALL: [Self; 6] = [Self::System, Self::Light, Self::Dark, Self::Night, Self::Batik, Self::Sawah];

    /// Stable id: stored in settings, synced, and used as `data-theme`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Night => "night",
            Self::Batik => "batik",
            Self::Sawah => "sawah",
        }
    }
}

/// How the Arabic-script pronunciation (تعريب النطق) shows next to
/// Indonesian and Javanese text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PronMode {
    /// Always under the word.
    #[default]
    On,
    /// Behind a tap, so reading the Latin spelling comes first.
    Tap,
    Off,
}

impl PronMode {
    pub const ALL: [Self; 3] = [Self::On, Self::Tap, Self::Off];
}

/// The share of reviews FSRS aims to have remembered, in percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RetentionPct(pub u8);

impl RetentionPct {
    /// Beyond these, FSRS either schedules absurdly often or lets most
    /// cards slip; a stored value outside is clamped.
    pub const MIN: u8 = 70;
    pub const MAX: u8 = 97;

    pub fn as_f32(self) -> f32 {
        f32::from(self.0.clamp(Self::MIN, Self::MAX)) / 100.0
    }
}

impl Default for RetentionPct {
    fn default() -> Self {
        Self(90)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub ui_language: UiLanguage,
    pub theme: Theme,
    /// Show ٠١٢٣ instead of 0123 in the Arabic UI.
    pub eastern_digits: bool,
    /// The first-launch greeting has been seen.
    pub welcomed: bool,
    /// Lesson and review sounds are off (SPEC §9: toggleable, on by default).
    pub sounds_off: bool,
    /// Arabic-script pronunciation next to target-language text.
    pub pronunciation: PronMode,
    /// FSRS desired retention (no UI yet; 90%).
    pub desired_retention: RetentionPct,
    /// FSRS parameters fitted to this learner's reviews, once there are enough.
    pub fsrs: Option<FsrsParams>,
}

/// Keys as stored in the `settings` table.
pub mod keys {
    pub const UI_LANGUAGE: &str = "ui_language";
    pub const THEME: &str = "theme";
    pub const EASTERN_DIGITS: &str = "eastern_digits";
    pub const WELCOMED: &str = "welcomed";
    pub const SOUNDS_OFF: &str = "sounds_off";
    pub const PRONUNCIATION: &str = "pronunciation";
    pub const DESIRED_RETENTION: &str = "desired_retention";
    pub const FSRS_PARAMS: &str = "fsrs_params";
}

impl Settings {
    /// Builds settings from stored `(key, json value)` pairs. Unknown keys and
    /// unreadable values fall back to defaults instead of failing.
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut s = Self::default();
        for (key, value) in pairs {
            match key {
                keys::UI_LANGUAGE => s.ui_language = serde_json::from_str(value).unwrap_or_default(),
                keys::THEME => s.theme = serde_json::from_str(value).unwrap_or_default(),
                keys::EASTERN_DIGITS => s.eastern_digits = serde_json::from_str(value).unwrap_or_default(),
                keys::WELCOMED => s.welcomed = serde_json::from_str(value).unwrap_or_default(),
                keys::SOUNDS_OFF => s.sounds_off = serde_json::from_str(value).unwrap_or_default(),
                keys::PRONUNCIATION => s.pronunciation = serde_json::from_str(value).unwrap_or_default(),
                keys::DESIRED_RETENTION => {
                    s.desired_retention = serde_json::from_str(value).unwrap_or_default()
                }
                keys::FSRS_PARAMS => s.fsrs = serde_json::from_str(value).unwrap_or_default(),
                _ => {}
            }
        }
        s
    }

    /// The `(key, json value)` pairs that differ between `self` and `other`.
    pub fn changed_pairs(&self, other: &Self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let mut push = |changed: bool, key, value: String| {
            if changed {
                out.push((key, value));
            }
        };
        push(self.ui_language != other.ui_language, keys::UI_LANGUAGE, json(&other.ui_language));
        push(self.theme != other.theme, keys::THEME, json(&other.theme));
        push(self.eastern_digits != other.eastern_digits, keys::EASTERN_DIGITS, json(&other.eastern_digits));
        push(self.welcomed != other.welcomed, keys::WELCOMED, json(&other.welcomed));
        push(self.sounds_off != other.sounds_off, keys::SOUNDS_OFF, json(&other.sounds_off));
        push(self.pronunciation != other.pronunciation, keys::PRONUNCIATION, json(&other.pronunciation));
        push(
            self.desired_retention != other.desired_retention,
            keys::DESIRED_RETENTION,
            json(&other.desired_retention),
        );
        push(self.fsrs != other.fsrs, keys::FSRS_PARAMS, json(&other.fsrs));
        out
    }
}

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("settings values always serialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_egyptian_arabic_and_system_theme() {
        let s = Settings::default();
        assert_eq!(s.ui_language, UiLanguage::ArEg);
        assert_eq!(s.theme, Theme::System);
        assert!(!s.welcomed);
    }

    #[test]
    fn round_trips_only_changed_keys() {
        let old = Settings::default();
        let new = Settings { theme: Theme::Dark, welcomed: true, ..old.clone() };
        let pairs = old.changed_pairs(&new);
        assert_eq!(pairs.len(), 2);
        let restored = Settings::from_pairs(pairs.iter().map(|(k, v)| (*k, v.as_str())));
        assert_eq!(restored, new);
    }

    #[test]
    fn bad_values_fall_back_to_defaults() {
        let s = Settings::from_pairs([("theme", "\"neon\""), ("eastern_digits", "true"), ("nope", "1")]);
        assert_eq!(s.theme, Theme::System);
        assert!(s.eastern_digits);
    }

    #[test]
    fn theme_ids_round_trip() {
        for theme in Theme::ALL {
            let s = Settings::from_pairs([("theme", format!("\"{}\"", theme.as_str()).as_str())]);
            assert_eq!(s.theme, theme);
        }
    }

    #[test]
    fn fsrs_settings_round_trip_and_default_to_90_percent() {
        let old = Settings::default();
        assert_eq!(old.desired_retention.as_f32(), 0.9);
        assert_eq!(old.fsrs, None);
        let params =
            FsrsParams { weights: vec![0.4, 1.2, 3.1], reviews: 512, trained_at_ms: 1_761_472_800_000 };
        let new = Settings { fsrs: Some(params), desired_retention: RetentionPct(85), ..old.clone() };
        let pairs = old.changed_pairs(&new);
        assert_eq!(pairs.len(), 2);
        assert_eq!(Settings::from_pairs(pairs.iter().map(|(k, v)| (*k, v.as_str()))), new);
        assert_eq!(RetentionPct(99).as_f32(), 0.97, "clamped");
    }

    #[test]
    fn pronunciation_defaults_on_and_round_trips() {
        assert_eq!(Settings::default().pronunciation, PronMode::On);
        let new = Settings { pronunciation: PronMode::Tap, ..Settings::default() };
        let pairs = Settings::default().changed_pairs(&new);
        assert_eq!(pairs, vec![("pronunciation", "\"tap\"".to_string())]);
        assert_eq!(Settings::from_pairs(pairs.iter().map(|(k, v)| (*k, v.as_str()))), new);
    }
}
