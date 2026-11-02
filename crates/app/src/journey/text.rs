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
pub fn lessons_of(lang: UiLanguage, done: &str, total: &str) -> String {
    let template = match lang {
        UiLanguage::En => "{done} of {total|lesson|lessons}",
        UiLanguage::ArEg => "{done} من {total|درس|درسين|دروس}",
        UiLanguage::Ar => "{done} من {total|درس|درسين|دروس|درسًا}",
    };
    crate::i18n::fill(lang, template, &[("done", done), ("total", total)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lesson_counts_agree_with_the_total() {
        assert_eq!(lessons_of(UiLanguage::ArEg, "3", "8"), "3 من 8\u{a0}دروس");
        assert_eq!(lessons_of(UiLanguage::ArEg, "٣", "٨"), "٣ من ٨\u{a0}دروس");
        assert_eq!(lessons_of(UiLanguage::ArEg, "3", "14"), "3 من 14\u{a0}درس");
        assert_eq!(lessons_of(UiLanguage::ArEg, "1", "2"), "1 من درسين");
        assert_eq!(lessons_of(UiLanguage::En, "0", "7"), "0 of 7\u{a0}lessons");
        assert_eq!(lessons_of(UiLanguage::En, "0", "1"), "0 of 1\u{a0}lesson");
    }
}
