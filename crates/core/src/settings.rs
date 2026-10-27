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

/// The sound for a right answer. Each is a synthesized file,
/// `assets/sounds/<id>.wav` (`scripts/assets/synth_sounds.py`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrectSound {
    /// Two soft rising notes (picked by ear, 2026-10-27).
    #[default]
    #[serde(rename = "ding-1")]
    Ding1,
    /// A bright bell, a fifth up.
    #[serde(rename = "ding-2")]
    Ding2,
    /// Three quick saron notes.
    #[serde(rename = "ding-3")]
    Ding3,
    /// A round kendang "dhung".
    #[serde(rename = "kendang-1")]
    Kendang1,
    /// A lower kendang with more bend.
    #[serde(rename = "kendang-2")]
    Kendang2,
    /// A tight kendang "tak".
    #[serde(rename = "kendang-3")]
    Kendang3,
}

impl CorrectSound {
    pub const ALL: [Self; 6] =
        [Self::Ding1, Self::Ding2, Self::Ding3, Self::Kendang1, Self::Kendang2, Self::Kendang3];

    /// The sound file's stem, also its stored value.
    pub fn id(self) -> &'static str {
        match self {
            Self::Ding1 => "ding-1",
            Self::Ding2 => "ding-2",
            Self::Ding3 => "ding-3",
            Self::Kendang1 => "kendang-1",
            Self::Kendang2 => "kendang-2",
            Self::Kendang3 => "kendang-3",
        }
    }
}

/// The gong when a lesson ends (`assets/sounds/<id>.wav`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndSound {
    /// Warm, the classic.
    #[serde(rename = "gong-1")]
    Gong1,
    /// Deeper and darker.
    #[serde(rename = "gong-2")]
    Gong2,
    /// Smaller and brighter (picked by ear, 2026-10-27).
    #[default]
    #[serde(rename = "gong-3")]
    Gong3,
}

impl EndSound {
    pub const ALL: [Self; 3] = [Self::Gong1, Self::Gong2, Self::Gong3];

    /// The sound file's stem, also its stored value.
    pub fn id(self) -> &'static str {
        match self {
            Self::Gong1 => "gong-1",
            Self::Gong2 => "gong-2",
            Self::Gong3 => "gong-3",
        }
    }
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

/// How the translation shows under Indonesian and Javanese text outside an
/// exercise (slides, dialogue, feedback, review backs, transcripts, chats).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranslationMode {
    /// Always under the sentence.
    #[default]
    On,
    /// Behind a tap, to try reading first.
    Tap,
    Off,
}

impl TranslationMode {
    pub const ALL: [Self; 3] = [Self::On, Self::Tap, Self::Off];
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
    /// The sound for a right answer.
    pub correct_sound: CorrectSound,
    /// The gong when a lesson ends.
    pub end_sound: EndSound,
    /// A light vibration on answers and at the end of a lesson is off
    /// (phones and tablets that have a motor; on by default).
    pub haptics_off: bool,
    /// Arabic-script pronunciation next to target-language text.
    pub pronunciation: PronMode,
    /// FSRS desired retention (Settings → "How firmly to remember words"; 90%).
    pub desired_retention: RetentionPct,
    /// FSRS parameters fitted to this learner's reviews, once there are enough.
    pub fsrs: Option<FsrsParams>,
    /// Translation under target-language text.
    pub translation: TranslationMode,
    /// Word-class and word-part colours are off (on by default).
    pub word_colours_off: bool,
    /// Grammar hints (rule chips under Reader lines, "The rule here" in word
    /// cards) are off (on by default).
    pub grammar_hints_off: bool,
}

/// Keys as stored in the `settings` table.
pub mod keys {
    pub const UI_LANGUAGE: &str = "ui_language";
    pub const THEME: &str = "theme";
    pub const EASTERN_DIGITS: &str = "eastern_digits";
    pub const WELCOMED: &str = "welcomed";
    pub const SOUNDS_OFF: &str = "sounds_off";
    pub const CORRECT_SOUND: &str = "correct_sound";
    pub const END_SOUND: &str = "end_sound";
    pub const HAPTICS_OFF: &str = "haptics_off";
    pub const PRONUNCIATION: &str = "pronunciation";
    pub const DESIRED_RETENTION: &str = "desired_retention";
    pub const FSRS_PARAMS: &str = "fsrs_params";
    pub const TRANSLATION: &str = "translation";
    pub const WORD_COLOURS_OFF: &str = "word_colours_off";
    pub const GRAMMAR_HINTS_OFF: &str = "grammar_hints_off";
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
                keys::CORRECT_SOUND => s.correct_sound = serde_json::from_str(value).unwrap_or_default(),
                keys::END_SOUND => s.end_sound = serde_json::from_str(value).unwrap_or_default(),
                keys::HAPTICS_OFF => s.haptics_off = serde_json::from_str(value).unwrap_or_default(),
                keys::PRONUNCIATION => s.pronunciation = serde_json::from_str(value).unwrap_or_default(),
                keys::DESIRED_RETENTION => {
                    s.desired_retention = serde_json::from_str(value).unwrap_or_default()
                }
                keys::FSRS_PARAMS => s.fsrs = serde_json::from_str(value).unwrap_or_default(),
                keys::TRANSLATION => s.translation = serde_json::from_str(value).unwrap_or_default(),
                keys::WORD_COLOURS_OFF => {
                    s.word_colours_off = serde_json::from_str(value).unwrap_or_default()
                }
                keys::GRAMMAR_HINTS_OFF => {
                    s.grammar_hints_off = serde_json::from_str(value).unwrap_or_default()
                }
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
        push(self.correct_sound != other.correct_sound, keys::CORRECT_SOUND, json(&other.correct_sound));
        push(self.end_sound != other.end_sound, keys::END_SOUND, json(&other.end_sound));
        push(self.haptics_off != other.haptics_off, keys::HAPTICS_OFF, json(&other.haptics_off));
        push(self.pronunciation != other.pronunciation, keys::PRONUNCIATION, json(&other.pronunciation));
        push(
            self.desired_retention != other.desired_retention,
            keys::DESIRED_RETENTION,
            json(&other.desired_retention),
        );
        push(self.fsrs != other.fsrs, keys::FSRS_PARAMS, json(&other.fsrs));
        push(self.translation != other.translation, keys::TRANSLATION, json(&other.translation));
        push(
            self.word_colours_off != other.word_colours_off,
            keys::WORD_COLOURS_OFF,
            json(&other.word_colours_off),
        );
        push(
            self.grammar_hints_off != other.grammar_hints_off,
            keys::GRAMMAR_HINTS_OFF,
            json(&other.grammar_hints_off),
        );
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

    #[test]
    fn grammar_hints_default_on_and_round_trip() {
        assert!(!Settings::default().grammar_hints_off);
        let new = Settings { grammar_hints_off: true, ..Settings::default() };
        let pairs = Settings::default().changed_pairs(&new);
        assert_eq!(pairs, vec![("grammar_hints_off", "true".to_string())]);
        assert_eq!(Settings::from_pairs(pairs.iter().map(|(k, v)| (*k, v.as_str()))), new);
    }

    #[test]
    fn sound_choices_store_their_file_names() {
        let s = Settings::default();
        assert_eq!(
            (s.correct_sound, s.end_sound, s.haptics_off),
            (CorrectSound::Ding1, EndSound::Gong3, false)
        );
        let new = Settings { correct_sound: CorrectSound::Kendang2, end_sound: EndSound::Gong1, ..s.clone() };
        let pairs = s.changed_pairs(&new);
        assert_eq!(
            pairs,
            vec![("correct_sound", "\"kendang-2\"".to_string()), ("end_sound", "\"gong-1\"".to_string())]
        );
        assert_eq!(Settings::from_pairs(pairs.iter().map(|(k, v)| (*k, v.as_str()))), new);
        for c in CorrectSound::ALL {
            assert_eq!(json(&c), format!("\"{}\"", c.id()));
        }
        for e in EndSound::ALL {
            assert_eq!(json(&e), format!("\"{}\"", e.id()));
        }
    }

    #[test]
    fn translation_defaults_on_and_round_trips() {
        assert_eq!(Settings::default().translation, TranslationMode::On);
        let new = Settings { translation: TranslationMode::Tap, ..Settings::default() };
        let pairs = Settings::default().changed_pairs(&new);
        assert_eq!(pairs, vec![("translation", "\"tap\"".to_string())]);
        assert_eq!(Settings::from_pairs(pairs.iter().map(|(k, v)| (*k, v.as_str()))), new);
    }
}
