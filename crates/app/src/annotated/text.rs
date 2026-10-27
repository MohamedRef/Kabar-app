//! Strings for annotated text in Egyptian Arabic, MSA and
//! English, typed like `crate::i18n` (a missing translation is a compile error).

use kabar_core::locale::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum T {
    ShowTranslation,
    Close,
    Breakdown,
    NotInDictionary,
    TeachIt,
}

pub fn tt(lang: UiLanguage, msg: T) -> &'static str {
    use T::*;
    use UiLanguage::*;
    match (msg, lang) {
        (ShowTranslation, ArEg) => "يعني إيه؟",
        (ShowTranslation, Ar) => "ما معناها؟",
        (ShowTranslation, En) => "What does it mean?",
        (Close, ArEg) => "اقفل",
        (Close, Ar) => "إغلاق",
        (Close, En) => "Close",
        (Breakdown, ArEg) => "تفصيل الكلمة",
        (Breakdown, Ar) => "تحليل الكلمة",
        (Breakdown, En) => "Word breakdown",
        (NotInDictionary, ArEg) => "الكلمة دي لسه مش في قاموس Kabar.",
        (NotInDictionary, Ar) => "هذه الكلمة ليست في قاموس Kabar بعد.",
        (NotInDictionary, En) => "This word isn't in Kabar's dictionary yet.",
        (TeachIt, ArEg) => "علّم Kabar الكلمة دي",
        (TeachIt, Ar) => "علِّم Kabar هذه الكلمة",
        (TeachIt, En) => "Teach Kabar this word",
    }
}
