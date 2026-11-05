//! UI strings for the Reader, typed like `crate::i18n` (a missing
//! translation is a compile error). Moved out of the shared `Msg` catalog so
//! this feature's changes touch only its own file.

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderMsg {
    Voice,
    LangSlang,
    AddToReview,
    EnglishInChat,
    InThisSentence,
    Breakdown,
    MorphMore,
    MorphSoundChange,
    ThanksFeedback,
    ExplainThinking,
    RegisterLabel,
    NoGrammarMatched,
    FoundBy,
    GrammarLearnedIn,
    GrammarTaughtIn,
    GrammarOpenLesson,
    GrammarReadMore,
    CommonMistake,
    ExplainWord,
    ReaderBusyHint,
    Delete,
    Original,
    Explanation,
    Example,
    TranslationWordByWord,
    ReadingSettings,
    ReadSize,
    ReadSpacing,
}
}

impl crate::i18n::Text for ReaderMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: ReaderMsg) -> &'static str {
    use ReaderMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (Example, ArEg | Ar) => "مثال",
        (Example, En) => "Example",
        (TranslationWordByWord, ArEg) => "كلمة بكلمة من قاموس \u{2068}Kabar\u{2069}",
        (TranslationWordByWord, Ar) => "كلمة بكلمة من قاموس \u{2068}Kabar\u{2069}",
        (TranslationWordByWord, En) => "Word by word, from Kabar's dictionary",
        (ReadingSettings, ArEg | Ar) => "إعدادات القراءة",
        (ReadingSettings, En) => "Reading settings",
        (ReadSize, ArEg | Ar) => "حجم النص",
        (ReadSize, En) => "Text size",
        (ReadSpacing, ArEg | Ar) => "المسافة بين السطور",
        (ReadSpacing, En) => "Line spacing",
        (Voice, ArEg) => "الشرح",
        (Voice, Ar) => "لغة الشرح",
        (Voice, En) => "Explain in",
        (LangSlang, ArEg) => "اختصار شات",
        (LangSlang, Ar) => "اختصار محادثة",
        (LangSlang, En) => "Chat slang",
        (AddToReview, ArEg) => "ضيفها للمراجعة",
        (AddToReview, Ar) => "أضفها إلى المراجعة",
        (AddToReview, En) => "Add to review",
        (EnglishInChat, ArEg) => {
            "كلمة إنجليزي داخلة في الكلام، زي ما بنعمل إحنا كمان. إنت عارفها، فمش هتتحسب في كلماتك الجديدة."
        }
        (EnglishInChat, Ar) => "كلمة إنجليزية ممزوجة بالكلام. أنت تعرفها، فلن تُحسب ضمن كلماتك الجديدة.",
        (EnglishInChat, En) => {
            "An English word mixed into the chat. You know it, so it doesn't count as a new word."
        }
        (InThisSentence, ArEg) => "في الجملة دي",
        (InThisSentence, Ar) => "في هذه الجملة",
        (InThisSentence, En) => "In this sentence",
        (Breakdown, ArEg | Ar) => "التركيب",
        (Breakdown, En) => "Built from",
        (MorphMore, ArEg) => "تفصيل الكلمة",
        (MorphMore, Ar) => "تفصيل الكلمة",
        (MorphMore, En) => "Word breakdown",
        (MorphSoundChange, ArEg) => "أول حرف في الأصل بيتغيّر لما السابقة تلزق فيه",
        (MorphSoundChange, Ar) => "يتغيّر أول حرف في الجذر عند اتصال السابقة به",
        (MorphSoundChange, En) => "the root's first sound changes when the prefix joins it",
        (ThanksFeedback, ArEg) => "شكرًا، ده بيساعدنا نحسّن الشرح",
        (ThanksFeedback, Ar) => "شكرًا، هذا يساعد على تحسين الشرح",
        (ThanksFeedback, En) => "Thanks, that helps improve the explanations",
        (ExplainThinking, ArEg) => "بحضّرلك الشرح…",
        (ExplainThinking, Ar) => "جارٍ تحضير الشرح…",
        (ExplainThinking, En) => "Writing the explanation…",
        (RegisterLabel, ArEg) => "الأسلوب:",
        (RegisterLabel, Ar) => "الأسلوب:",
        (RegisterLabel, En) => "Register:",
        (NoGrammarMatched, ArEg) => "مفيش قاعدة من المكتبة باينة في الجملة دي، معاني الكلمات كفاية هنا.",
        (NoGrammarMatched, Ar) => "لا تظهر قاعدة من المكتبة في هذه الجملة، ومعاني الكلمات تكفي هنا.",
        (NoGrammarMatched, En) => {
            "No library grammar point shows up in this sentence; the word meanings cover it."
        }
        (FoundBy, ArEg) => "لقيتها من:",
        (FoundBy, Ar) => "وُجدت عبر:",
        (FoundBy, En) => "Spotted by:",
        (GrammarLearnedIn, ArEg) => "اتعلمتها في درس",
        (GrammarLearnedIn, Ar) => "تعلّمتَها في درس",
        (GrammarLearnedIn, En) => "Learned in",
        (GrammarTaughtIn, ArEg) => "هتيجي في درس",
        (GrammarTaughtIn, Ar) => "تأتي في درس",
        (GrammarTaughtIn, En) => "Taught in",
        (GrammarOpenLesson, ArEg) => "افتح الدرس",
        (GrammarOpenLesson, Ar) => "افتح الدرس",
        (GrammarOpenLesson, En) => "Open lesson",
        (GrammarReadMore, ArEg) => "الشرح كله",
        (GrammarReadMore, Ar) => "الشرح الكامل",
        (GrammarReadMore, En) => "Full explanation",
        (CommonMistake, ArEg) => "غلطة شائعة",
        (CommonMistake, Ar) => "خطأ شائع",
        (CommonMistake, En) => "Common mistake",
        (ExplainWord, ArEg) => "اشرح الجملة دي",
        (ExplainWord, Ar) => "اشرح هذه الجملة",
        (ExplainWord, En) => "Explain the sentence",
        (ReaderBusyHint, ArEg) => "هيتفتح لوحده أول ما يجهز. لحد ما يخلص تقدر تتفرج على مكتبتك.",
        (ReaderBusyHint, Ar) => "سيُفتح وحده حين يجهز. حتى ذلك الحين تصفّح مكتبتك.",
        (ReaderBusyHint, En) => "It opens by itself when ready. Browse your library meanwhile.",
        (Delete, ArEg) => "امسح",
        (Delete, Ar) => "احذف",
        (Delete, En) => "Delete",
        (Original, ArEg | Ar) => "الأصل",
        (Original, En) => "Original",
        (Explanation, ArEg | Ar) => "الشرح",
        (Explanation, En) => "Explanation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        crate::i18n::checks::catalog(ReaderMsg::ALL, text, |_| false);
    }
}
