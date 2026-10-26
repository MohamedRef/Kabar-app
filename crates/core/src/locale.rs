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

/// Formats a number for the UI, honoring the Eastern-digits setting in Arabic.
pub fn format_number(n: u64, lang: UiLanguage, eastern_digits: bool) -> String {
    let s = n.to_string();
    if eastern_digits && lang.dir() == Direction::Rtl { eastern_arabic_digits(&s) } else { s }
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
}
