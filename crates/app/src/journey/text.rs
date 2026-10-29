//! Journey strings added with the 2026-10-29 redesign, typed like
//! `crate::i18n` (a missing translation is a compile error).

use kabar_core::locale::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum T {
    /// The current-unit card's small heading.
    NowStop,
    /// Before the lesson's can-do line.
    Goal,
    /// A lesson's state, in words.
    Continue,
    Upcoming,
    /// The milestone row: the souvenir this stop gives.
    SouvenirHere,
    /// The milestone row: the level's test, once every stop is done.
    LevelTestNext,
}

pub fn tt(lang: UiLanguage, msg: T) -> &'static str {
    use T::*;
    use UiLanguage::*;
    match (msg, lang) {
        (NowStop, ArEg) => "محطتك دلوقتي",
        (NowStop, Ar) => "محطتك الحالية",
        (NowStop, En) => "Where you are",
        (Goal, ArEg) => "هدفك:",
        (Goal, Ar) => "هدفك:",
        (Goal, En) => "Goal:",
        (Continue, ArEg) => "كمّل هنا",
        (Continue, Ar) => "تابع هنا",
        (Continue, En) => "Continue",
        (Upcoming, ArEg) => "جاي",
        (Upcoming, Ar) => "قادم",
        (Upcoming, En) => "Upcoming",
        (SouvenirHere, ArEg) => "هدية المحطة دي",
        (SouvenirHere, Ar) => "هدية هذه المحطة",
        (SouvenirHere, En) => "This stop's souvenir",
        (LevelTestNext, ArEg) => "بعدها: اختبار المستوى",
        (LevelTestNext, Ar) => "بعدها: اختبار المستوى",
        (LevelTestNext, En) => "Then: the level test",
    }
}

/// "٣ من ٨ دروس" / "3 of 8 lessons".
pub fn lessons_of(lang: UiLanguage, done: &str, total: &str, total_n: usize) -> String {
    match lang {
        UiLanguage::En => format!("{done} of {total} lessons"),
        UiLanguage::ArEg | UiLanguage::Ar if (3..=10).contains(&total_n) => format!("{done} من {total} دروس"),
        UiLanguage::ArEg | UiLanguage::Ar => format!("{done} من {total} درس"),
    }
}
