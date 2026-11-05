//! UI strings for the games, typed like `crate::i18n` (a missing
//! translation is a compile error). Moved out of the shared `Msg` catalog so
//! this feature's changes touch only its own file.

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMsg {
    PlayContentProblem,
    PlayDetails,
    PlayTryInChat,
    PlayNotQuite,
    PlayGreat,
    PlayGood,
    PlayKeepGoing,
    Stop,
    Root,
    PagiWordsNote,
    PagiNoStory,
    DeckMarked,
    BridgeFromArabic,
    DetectiveGuessArabic,
    DetectiveGuessIndonesian,
    DetectiveNoRounds,
    AlarmQuestion,
    AlarmCaught,
    AlarmTricky,
    RupiahIntro,
    RupiahType,
    RupiahClose,
    RupiahScore,
    RupiahSeconds,
    PasarIntro,
    PasarNoItems,
    PasarBonus,
    PasarBudget,
    PasarOnTable,
    PasarSaved,
    UnggahIntro,
    UnggahNoScenes,
    UnggahMeter,
    UnggahYourTurn,
    UnggahBetter,
    UnggahFitting,
    PairsIntro,
    PairsInJavanese,
    PairsInIndonesian,
    PairsTipTitle,
    PairsTheTwoWords,
}
}

impl crate::i18n::Text for PlayMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: PlayMsg) -> &'static str {
    use PlayMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (PlayContentProblem, ArEg) => {
            "اللعبة دي محتاجة محتوى مقدرتش أفتحه دلوقتي. باقي التطبيق شغال عادي، وهنصلّحها في النسخة الجاية."
        }
        (PlayContentProblem, Ar) => {
            "تحتاج هذه اللعبة إلى محتوى تعذّر فتحه الآن. بقية التطبيق تعمل كالمعتاد، وسنصلحها في الإصدار القادم."
        }
        (PlayContentProblem, En) => {
            "This game needs content that didn't open just now. The rest of Kabar works as usual; the next build fixes it."
        }
        (PlayDetails, ArEg | Ar) => "تفاصيل",
        (PlayDetails, En) => "Details",
        (PlayTryInChat, ArEg) => "جرّب الكلام في دردشة",
        (PlayTryInChat, Ar) => "جرّبها في محادثة",
        (PlayTryInChat, En) => "Try it in a chat",
        (PlayNotQuite, ArEg) => "قربت! الإجابة:",
        (PlayNotQuite, Ar) => "اقتربت! الإجابة:",
        (PlayNotQuite, En) => "Close! The answer:",
        (PlayGreat, ArEg) => "إيه الحلاوة دي! شغل نضيف.",
        (PlayGreat, Ar) => "رائع! عمل متقن.",
        (PlayGreat, En) => "Lovely work, that was sharp.",
        (PlayGood, ArEg) => "حلو أوي، واللي فاتك هيبقى أسهل المرة الجاية.",
        (PlayGood, Ar) => "جيد جدًا، وما فاتك سيكون أسهل في المرة القادمة.",
        (PlayGood, En) => "Nicely done; the ones you missed will come easier next time.",
        (PlayKeepGoing, ArEg) => "كل جولة بتعلّمك حاجة، والغلطات دي هي خطة بكرة.",
        (PlayKeepGoing, Ar) => "كل جولة تعلّمك شيئًا، وهذه الأخطاء هي خطة الغد.",
        (PlayKeepGoing, En) => "Every round teaches something; today's misses are tomorrow's plan.",
        (Stop, ArEg) => "وقّف",
        (Stop, Ar) => "إيقاف",
        (Stop, En) => "Stop",
        (Root, ArEg | Ar) => "الجذر:",
        (Root, En) => "root:",

        (PagiWordsNote, ArEg) => "دي كلمات النهارده؛ الملوّنة بالدهبي جاية من العربي.",
        (PagiWordsNote, Ar) => "هذه كلمات اليوم؛ الملوّنة بالذهبي أصلها عربي.",
        (PagiWordsNote, En) => "Today's words; the gold ones come from Arabic.",
        (PagiNoStory, ArEg) => "مفيش حكاية جاهزة النهارده لسه. جرّب لعبة تانية وارجع بكرة.",
        (PagiNoStory, Ar) => "لا توجد قصة جاهزة اليوم بعد. جرّب لعبة أخرى وعد غدًا.",
        (PagiNoStory, En) => "No story is ready today yet. Try another game and come back tomorrow.",

        (DeckMarked, ArEg) => "اللي علّمت عليها:",
        (DeckMarked, Ar) => "ما أشرت إليه:",
        (DeckMarked, En) => "Marked as known:",
        (BridgeFromArabic, ArEg | Ar) => "من العربي للإندونيسي",
        (BridgeFromArabic, En) => "From Arabic to Indonesian",
        (DetectiveGuessArabic, ArEg) => "إيه الكلمة العربي اللي ورا دي؟",
        (DetectiveGuessArabic, Ar) => "ما الكلمة العربية وراء هذه الكلمة؟",
        (DetectiveGuessArabic, En) => "Which Arabic word is behind it?",
        (DetectiveGuessIndonesian, ArEg) => "الكلمة دي بقت إيه في الإندونيسي؟",
        (DetectiveGuessIndonesian, Ar) => "إلامَ صارت هذه الكلمة في الإندونيسية؟",
        (DetectiveGuessIndonesian, En) => "What did this word become in Indonesian?",
        (DetectiveNoRounds, ArEg) => "مفيش جولات جاهزة هنا لسه. جرّب اختيار تاني.",
        (DetectiveNoRounds, Ar) => "لا توجد جولات جاهزة هنا بعد. جرّب خيارًا آخر.",
        (DetectiveNoRounds, En) => "No rounds are ready here yet. Try another choice.",

        (AlarmQuestion, ArEg) => "معناها إيه في الإندونيسي؟",
        (AlarmQuestion, Ar) => "ما معناها في الإندونيسية؟",
        (AlarmQuestion, En) => "What does it mean in Indonesian?",
        (AlarmCaught, ArEg) => "مسكتها! مضحكتش عليك.",
        (AlarmCaught, Ar) => "كشفتها! لم تخدعك.",
        (AlarmCaught, En) => "Caught it! It didn't fool you.",
        (AlarmTricky, ArEg) => "دي خدّاعة فعلاً، وأهو بقيت عارفها:",
        (AlarmTricky, Ar) => "إنها مخادعة فعلًا، والآن صرت تعرفها:",
        (AlarmTricky, En) => "A tricky one, and now you know it:",

        (RupiahIntro, ArEg) => "هيظهرلك سعر؛ اكتب إزاي بيتقري بالإندونيسي.",
        (RupiahIntro, Ar) => "سيظهر سعر؛ اكتب كيف يُقرأ بالإندونيسية.",
        (RupiahIntro, En) => "You'll see a price; type how it's read in Indonesian.",
        (RupiahType, ArEg) => "اكتب إزاي بيتقري",
        (RupiahType, Ar) => "اكتب طريقة قراءته",
        (RupiahType, En) => "Type the reading",
        (RupiahClose, ArEg) => "صح! بس دي الطريقة المظبوطة:",
        (RupiahClose, Ar) => "صحيح! والصيغة الدقيقة هي:",
        (RupiahClose, En) => "Right! The exact reading is:",
        (RupiahScore, ArEg | Ar) => "النقاط",
        (RupiahScore, En) => "Score",
        (RupiahSeconds, ArEg | Ar) => "ثانية",
        (RupiahSeconds, En) => "Seconds",
        (PasarIntro, ArEg) => "اختار حاجة تشتريها. افضل في حدود الميزانية وخليك مؤدب، والبياع هيرد عليك.",
        (PasarIntro, Ar) => "اختر شيئًا لتشتريه. ابقَ ضمن الميزانية وكن مهذبًا، وسيرد عليك البائع.",
        (PasarIntro, En) => "Pick something to buy. Stay within budget and stay polite; the seller answers.",
        (PasarNoItems, ArEg) => "السوق لسه بيفتح. ارجع بعد التحديث الجاي.",
        (PasarNoItems, Ar) => "السوق لم يفتح بعد. عد بعد التحديث القادم.",
        (PasarNoItems, En) => "The market is still setting up. Come back after the next update.",
        (PasarBonus, ArEg) => "جولة بونص",
        (PasarBonus, Ar) => "جولة إضافية",
        (PasarBonus, En) => "Bonus round",
        (PasarBudget, ArEg) => "ميزانيتك:",
        (PasarBudget, Ar) => "ميزانيتك:",
        (PasarBudget, En) => "Budget:",
        (PasarOnTable, ArEg) => "السعر دلوقتي:",
        (PasarOnTable, Ar) => "السعر الحالي:",
        (PasarOnTable, En) => "On the table:",
        (PasarSaved, ArEg) => "وفّرت:",
        (PasarSaved, Ar) => "وفّرت:",
        (PasarSaved, En) => "You saved:",
        (UnggahIntro, ArEg) => {
            "اختار مشهد. في كل دور هتلاقي نفس الكلام بمستويات مختلفة؛ اختار اللي يليق بالشخص اللي قدامك، و«مقياس الاحترام» هيوريك وقعه عليه."
        }
        (UnggahIntro, Ar) => {
            "اختر مشهدًا. في كل دور تجد الكلام نفسه بمستويات مختلفة؛ اختر ما يليق بمن تخاطبه، وسيُريك «مقياس الاحترام» وقعه عليه."
        }
        (UnggahIntro, En) => {
            "Pick a scene. Each turn offers the same thing at different speech levels: choose what suits the person, and the Respect Meter shows how it lands."
        }
        (UnggahNoScenes, ArEg) => "المشاهد لسه بتتكتب. ارجع بعد التحديث الجاي.",
        (UnggahNoScenes, Ar) => "المشاهد قيد الكتابة. عد بعد التحديث القادم.",
        (UnggahNoScenes, En) => "The scenes are still being written. Come back after the next update.",
        (UnggahMeter, ArEg | Ar) => "مقياس الاحترام",
        (UnggahMeter, En) => "Respect Meter",
        (UnggahYourTurn, ArEg) => "هتقول إيه؟",
        (UnggahYourTurn, Ar) => "ماذا ستقول؟",
        (UnggahYourTurn, En) => "What do you say?",
        (UnggahBetter, ArEg) => "اللي يليق هنا:",
        (UnggahBetter, Ar) => "الأنسب هنا:",
        (UnggahBetter, En) => "What fits here:",
        (UnggahFitting, ArEg | Ar) => "اختيارات في محلّها",
        (UnggahFitting, En) => "fitting choices",
        (PairsIntro, ArEg) => {
            "الجاوي فيه نوعين من كل حرف: \u{2068}d\u{2069} و\u{2068}t\u{2069} زي الدال والتاء بتوعنا، و\u{2068}dh\u{2069} و\u{2068}th\u{2069} واللسان راجع لورا. في كل جولة كلمتين شبه بعض، اختار الكتابة الصح."
        }
        (PairsIntro, Ar) => {
            "في الجاوية نوعان من كل حرف: \u{2068}d\u{2069} و\u{2068}t\u{2069} كالدال والتاء العربيتين، و\u{2068}dh\u{2069} و\u{2068}th\u{2069} واللسان مرتدّ إلى الخلف. في كل جولة كلمتان متشابهتان، فاختر الكتابة الصحيحة."
        }
        (PairsIntro, En) => {
            "Javanese has two of each: d and t just like Arabic د and ت, and dh and th with the tongue pulled back. Each round has two look-alike words: pick the right spelling."
        }
        (PairsInJavanese, ArEg) => "بالجاوي:",
        (PairsInJavanese, Ar) => "بالجاوية:",
        (PairsInJavanese, En) => "In Javanese:",
        (PairsInIndonesian, ArEg) => "بالإندونيسي:",
        (PairsInIndonesian, Ar) => "بالإندونيسية:",
        (PairsInIndonesian, En) => "In Indonesian:",
        (PairsTipTitle, ArEg) => "إزاي تقولها",
        (PairsTipTitle, Ar) => "كيف تنطقها",
        (PairsTipTitle, En) => "How to say it",
        (PairsTheTwoWords, ArEg) => "الكلمتين",
        (PairsTheTwoWords, Ar) => "الكلمتان",
        (PairsTheTwoWords, En) => "The two words",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        crate::i18n::checks::catalog(PlayMsg::ALL, text, |m| matches!(m, PlayMsg::PairsIntro));
    }
}
