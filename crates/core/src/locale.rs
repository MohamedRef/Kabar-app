//! UI language, text direction and number formatting.

use serde::{Deserialize, Serialize};

/// The language Kabar explains things in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiLanguage {
    /// Egyptian colloquial Arabic (بالمصري).
    #[default]
    ArEg,
    /// Modern Standard Arabic.
    Ar,
    En,
}

impl UiLanguage {
    pub const ALL: [Self; 3] = [Self::ArEg, Self::Ar, Self::En];

    /// BCP 47 tag for the `lang` attribute.
    pub fn tag(self) -> &'static str {
        match self {
            Self::ArEg => "ar-EG",
            Self::Ar => "ar",
            Self::En => "en",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.tag() == tag)
    }

    pub fn dir(self) -> Direction {
        match self {
            Self::ArEg | Self::Ar => Direction::Rtl,
            Self::En => Direction::Ltr,
        }
    }

    /// The language's own name, for the language picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::ArEg => "مصري",
            Self::Ar => "العربية الفصحى",
            Self::En => "English",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        }
    }
}

/// Replaces ASCII digits with Eastern Arabic digits (٠١٢٣٤٥٦٧٨٩).
pub fn eastern_arabic_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c.to_digit(10) {
            Some(d) => char::from_u32(0x0660 + d).unwrap_or(c),
            None => c,
        })
        .collect()
}

/// Replaces Eastern Arabic (and Persian) digits with ASCII digits.
pub fn western_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c as u32 {
            d @ 0x0660..=0x0669 => char::from_u32(u32::from('0') + d - 0x0660).unwrap_or(c),
            d @ 0x06F0..=0x06F9 => char::from_u32(u32::from('0') + d - 0x06F0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// The one rule for UI digits: Eastern Arabic when the learner turned the
/// setting on and the UI is Arabic, ASCII otherwise.
pub fn uses_eastern_digits(lang: UiLanguage, eastern_digits: bool) -> bool {
    eastern_digits && lang.dir() == Direction::Rtl
}

/// Writes the digits of UI text (content included) the way the learner
/// chose (`uses_eastern_digits`).
pub fn localize_digits(s: &str, lang: UiLanguage, eastern_digits: bool) -> String {
    if uses_eastern_digits(lang, eastern_digits) { eastern_arabic_digits(s) } else { western_digits(s) }
}

/// Formats a number for the UI, honoring the Eastern-digits setting in Arabic.
pub fn format_number(n: u64, lang: UiLanguage, eastern_digits: bool) -> String {
    localize_digits(&n.to_string(), lang, eastern_digits)
}

/// "87%", or "٨٧٪" with Eastern digits (the Arabic percent sign goes with them).
pub fn format_percent(n: u64, lang: UiLanguage, eastern_digits: bool) -> String {
    let sign = if uses_eastern_digits(lang, eastern_digits) { '٪' } else { '%' };
    format!("{}{sign}", format_number(n, lang, eastern_digits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_is_rtl_english_is_ltr() {
        assert_eq!(UiLanguage::ArEg.dir(), Direction::Rtl);
        assert_eq!(UiLanguage::Ar.dir(), Direction::Rtl);
        assert_eq!(UiLanguage::En.dir(), Direction::Ltr);
    }

    #[test]
    fn tags_round_trip() {
        for l in UiLanguage::ALL {
            assert_eq!(UiLanguage::from_tag(l.tag()), Some(l));
        }
    }

    #[test]
    fn eastern_digits_only_where_asked() {
        assert_eq!(format_number(2026, UiLanguage::ArEg, true), "٢٠٢٦");
        assert_eq!(format_number(2026, UiLanguage::ArEg, false), "2026");
        assert_eq!(format_number(2026, UiLanguage::En, true), "2026");
        assert_eq!(eastern_arabic_digits("Rp26.000"), "Rp٢٦.٠٠٠");
    }

    #[test]
    fn content_digits_follow_the_setting() {
        assert_eq!(localize_digits("حوالي ١٠ دقايق", UiLanguage::ArEg, false), "حوالي 10 دقايق");
        assert_eq!(localize_digits("حوالي 10 دقايق", UiLanguage::ArEg, true), "حوالي ١٠ دقايق");
        assert_eq!(localize_digits("About ۱۰", UiLanguage::En, true), "About 10");
    }

    #[test]
    fn percent_sign_follows_the_digits() {
        assert_eq!(format_percent(87, UiLanguage::Ar, true), "٨٧٪");
        assert_eq!(format_percent(87, UiLanguage::ArEg, false), "87%");
        assert_eq!(format_percent(87, UiLanguage::En, true), "87%");
    }
}
