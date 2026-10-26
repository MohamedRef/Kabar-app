//! User settings. Stored and synced per key (last-writer-wins by HLC), so
//! changing the theme on the tablet never overwrites the language picked on
//! the phone.

use serde::{Deserialize, Serialize};

use crate::locale::UiLanguage;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
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
}

/// Keys as stored in the `settings` table.
pub mod keys {
    pub const UI_LANGUAGE: &str = "ui_language";
    pub const THEME: &str = "theme";
    pub const EASTERN_DIGITS: &str = "eastern_digits";
    pub const WELCOMED: &str = "welcomed";
    pub const SOUNDS_OFF: &str = "sounds_off";
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
}
