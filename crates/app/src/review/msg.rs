//! UI strings for Review, typed like `crate::i18n` (a missing
//! translation is a compile error). Moved out of the shared `Msg` catalog so
//! this feature's changes touch only its own file.

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewMsg {
    RevSeedBody,
    RevSeedButton,
    RevForecast,
    RevToday,
    RevTomorrow,
    RevIn,
    RevClassic,
    RevClassicHint,
    RevWall,
    RevWallHint,
    RevWallPhone,
    RevWallHintTouch,
    RevWallHintMouse,
    RevBurst,
    RevBurstHint,
    RevStart,
    RevNextReview,
    RevLearnNew,
    RevShowAnswer,
    RevRateHint,
    RevContentOk,
    RevKeysHint,
    RevAnswerPlaceholder,
    RevCheck,
    RevExact,
    RevClose,
    RevWrong,
    RevYouWrote,
    RevPlay,
    RevUndo,
    RevFinish,
    RevNotIndonesian,
    RevShifted,
    RevLeft,
    RevThisSession,
    RevAnswered,
    RevRemaining,
    RevJustReviewed,
    RevNoneYet,
    RevTahu,
    RevBelum,
    RevWallHelp,
    RevKnow,
    RevNotYet,
    RevSecondsLeft,
    RevAllRemembered,
    MonShort,
    TueShort,
    WedShort,
    ThuShort,
    FriShort,
    SatShort,
    SunShort,
}
}

impl crate::i18n::Text for ReviewMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: ReviewMsg) -> &'static str {
    use ReviewMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (RevSeedBody, ArEg) => {
            "{n|كلمة إندونيسي|كلمتين إندونيسي|كلمات إندونيسي|كلمة إندونيسي} أصلها عربي. هتعرف أغلبهم من أول نظرة، وفيهم اتنين بيعملوا مقالب."
        }
        (RevSeedBody, Ar) => {
            "{n|كلمة إندونيسية|كلمتان إندونيسيتان|كلمات إندونيسية|كلمةً إندونيسية} أصلها عربي. ستعرف أغلبها من النظرة الأولى، واثنتان منها تخدعانك."
        }
        (RevSeedBody, En) => {
            "{n|Indonesian word|Indonesian words} that came from Arabic. You'll recognize most at first sight, and two of them play tricks."
        }
        (RevSeedButton, ArEg) => "ضيف الكلمات دي",
        (RevSeedButton, Ar) => "أضف هذه الكلمات",
        (RevSeedButton, En) => "Add these words",
        (RevForecast, ArEg) => "الأيام السبعة الجاية",
        (RevForecast, Ar) => "الأيام السبعة القادمة",
        (RevForecast, En) => "The next 7 days",
        (RevToday, ArEg) => "النهارده",
        (RevToday, Ar) => "اليوم",
        (RevToday, En) => "Today",
        (RevTomorrow, ArEg) => "بكرة",
        (RevTomorrow, Ar) => "غدًا",
        (RevTomorrow, En) => "tomorrow",
        (RevIn, ArEg | Ar) => "بعد {span}",
        (RevIn, En) => "in {span}",
        (RevClassic, ArEg) => "كروت عادية",
        (RevClassic, Ar) => "البطاقات التقليدية",
        (RevClassic, En) => "Classic cards",
        (RevClassicHint, ArEg) => "اكشف الكارت وقول عرفته قد إيه.",
        (RevClassicHint, Ar) => "اكشف البطاقة وقيّم مدى معرفتك بها.",
        (RevClassicHint, En) => "Reveal each card, then say how well you knew it.",
        (RevWall, ArEg) => "طاولة الكروت",
        (RevWall, Ar) => "طاولة البطاقات",
        (RevWall, En) => "Card Wall",
        (RevWallHint, ArEg) => {
            "زقّ الكروت بالقلم أو بصباعك لكومة \u{2068}tahu\u{2069} (عارفها) أو \u{2068}belum\u{2069} (لسه)."
        }
        (RevWallHint, Ar) => {
            "ادفع البطاقات بالقلم أو بإصبعك إلى كومة \u{2068}tahu\u{2069} (أعرفها) أو \u{2068}belum\u{2069} (ليس بعد)."
        }
        (RevWallHint, En) => {
            "Flick cards with the pen or your finger into tahu (I know it) or belum (not yet)."
        }
        (RevWallHintTouch, ArEg) => {
            "زقّ الكروت بصباعك لكومة \u{2068}tahu\u{2069} (عارفها) أو \u{2068}belum\u{2069} (لسه)."
        }
        (RevWallHintTouch, Ar) => {
            "ادفع البطاقات بإصبعك إلى كومة \u{2068}tahu\u{2069} (أعرفها) أو \u{2068}belum\u{2069} (ليس بعد)."
        }
        (RevWallHintTouch, En) => "Flick cards with your finger into tahu (I know it) or belum (not yet).",
        (RevWallHintMouse, ArEg) => {
            "اسحب الكروت بالماوس لكومة \u{2068}tahu\u{2069} (عارفها) أو \u{2068}belum\u{2069} (لسه)."
        }
        (RevWallHintMouse, Ar) => {
            "اسحب البطاقات بالفأرة إلى كومة \u{2068}tahu\u{2069} (أعرفها) أو \u{2068}belum\u{2069} (ليس بعد)."
        }
        (RevWallHintMouse, En) => "Drag cards with the mouse onto tahu (I know it) or belum (not yet).",
        (RevWallPhone, ArEg) => "على الموبايل كارت كارت؛ الطاولة الكاملة على التابلت.",
        (RevWallPhone, Ar) => "على الهاتف بطاقة واحدة في كل مرة؛ والطاولة الكاملة على الجهاز اللوحي.",
        (RevWallPhone, En) => "One card at a time on the phone; the full table is on the tablet.",
        (RevBurst, ArEg | Ar) => "دقيقة سريعة",
        (RevBurst, En) => "1-minute burst",
        (RevBurstHint, ArEg) => "أكتر عدد كروت تقدر عليه في {n|ثانية|ثانيتين|ثواني}.",
        (RevBurstHint, Ar) => "أكبر عدد من البطاقات في {n|ثانية|ثانيتين|ثوانٍ|ثانيةً}.",
        (RevBurstHint, En) => "As many quick cards as fit in {n|second|seconds}.",
        (RevStart, ArEg) => "يلا",
        (RevStart, Ar) => "ابدأ",
        (RevStart, En) => "Start",
        (RevNextReview, ArEg) => "المراجعة الجاية: {when}",
        (RevNextReview, Ar) => "المراجعة القادمة: {when}",
        (RevNextReview, En) => "Next review: {when}",
        (RevLearnNew, ArEg) => "عايز كلمات جديدة؟ كمّل رحلتك",
        (RevLearnNew, Ar) => "تريد كلمات جديدة؟ تابع رحلتك",
        (RevLearnNew, En) => "Want new words? Continue your journey",
        (RevRateHint, ArEg) => "قيّم اللي افتكرته، مش سهولة السؤال.",
        (RevRateHint, Ar) => "قيّم ما تذكّرته، لا سهولة السؤال.",
        (RevRateHint, En) => "Rate what you remembered, not how easy it looked.",
        (RevContentOk, ArEg) => "المحتوى سليم",
        (RevContentOk, Ar) => "المحتوى سليم",
        (RevContentOk, En) => "The content is right",
        (RevShowAnswer, ArEg) => "وريني الإجابة",
        (RevShowAnswer, Ar) => "أظهر الإجابة",
        (RevShowAnswer, En) => "Show answer",
        (RevKeysHint, ArEg) => "المسافة تكشف، و1–4 للتقييم",
        (RevKeysHint, Ar) => "المسافة للكشف، و1–4 للتقييم",
        (RevKeysHint, En) => "Space reveals, 1–4 rate",
        (RevAnswerPlaceholder, ArEg) => "اكتب هنا، بالكيبورد أو بالقلم",
        (RevAnswerPlaceholder, Ar) => "اكتب هنا، بلوحة المفاتيح أو بالقلم",
        (RevAnswerPlaceholder, En) => "Type here, or write with the pen",
        (RevCheck, ArEg) => "شوف",
        (RevCheck, Ar) => "تحقّق",
        (RevCheck, En) => "Check",
        (RevExact, ArEg) => "بالظبط!",
        (RevExact, Ar) => "تمامًا!",
        (RevExact, En) => "Spot on!",
        (RevClose, ArEg) => "قربت أوي، حرف واحد بس.",
        (RevClose, Ar) => "قريب جدًّا، حرف واحد فقط.",
        (RevClose, En) => "So close: one letter off.",
        (RevWrong, ArEg) => "دي الكلمة. المرة الجاية هتفتكرها.",
        (RevWrong, Ar) => "هذه هي الكلمة، وستتذكرها في المرة القادمة.",
        (RevWrong, En) => "Here's the word. It'll come easier next time.",
        (RevYouWrote, ArEg) => "إنت كتبت",
        (RevYouWrote, Ar) => "كتبتَ",
        (RevYouWrote, En) => "You wrote",
        (RevPlay, ArEg | Ar) => "شغّل النطق",
        (RevPlay, En) => "Play pronunciation",
        (RevUndo, ArEg) => "رجّع آخر إجابة",
        (RevUndo, Ar) => "تراجع عن آخر إجابة",
        (RevUndo, En) => "Undo last answer",
        (RevFinish, ArEg) => "كفاية كده",
        (RevFinish, Ar) => "إنهاء الجلسة",
        (RevFinish, En) => "Finish",
        (RevNotIndonesian, ArEg) => "مش زي الإندونيسي",
        (RevNotIndonesian, Ar) => "ليست كالإندونيسية",
        (RevNotIndonesian, En) => "not like Indonesian",
        (RevShifted, ArEg) => "المعنى اتغيّر شوية",
        (RevShifted, Ar) => "تغيّر المعنى قليلًا",
        (RevShifted, En) => "The meaning shifted",
        (RevLeft, ArEg) => "فاضل {n}",
        (RevLeft, Ar) => "المتبقي {n}",
        (RevLeft, En) => "{n} left",
        (RevThisSession, ArEg) => "الجلسة دي",
        (RevThisSession, Ar) => "هذه الجلسة",
        (RevThisSession, En) => "This session",
        (RevAnswered, ArEg) => "جاوبت",
        (RevAnswered, Ar) => "أجبت",
        (RevAnswered, En) => "Answered",
        (RevRemaining, ArEg) => "فاضل",
        (RevRemaining, Ar) => "متبقٍّ",
        (RevRemaining, En) => "To go",
        (RevJustReviewed, ArEg) => "لسه راجعتها",
        (RevJustReviewed, Ar) => "راجعتها للتو",
        (RevJustReviewed, En) => "Just reviewed",
        (RevNoneYet, ArEg) => "أول كلمة بتراجعها هتظهر هنا.",
        (RevNoneYet, Ar) => "ستظهر هنا أول كلمة تراجعها.",
        (RevNoneYet, En) => "The words you review show up here.",
        (RevTahu, ArEg) => "عارفها",
        (RevTahu, Ar) => "أعرفها",
        (RevTahu, En) => "I know it",
        (RevBelum, ArEg) => "لسه",
        (RevBelum, Ar) => "ليس بعد",
        (RevBelum, En) => "Not yet",
        (RevWallHelp, ArEg) => "دوس على الكارت تشوف ضهره، واسحبه أو ازّقه لكومة.",
        (RevWallHelp, Ar) => "انقر البطاقة لترى ظهرها، ثم اسحبها أو ادفعها إلى كومة.",
        (RevWallHelp, En) => "Tap a card to peek at its back; drag or flick it onto a pile.",
        (RevKnow, ArEg) => "عارفها",
        (RevKnow, Ar) => "أعرفها",
        (RevKnow, En) => "I know it",
        (RevNotYet, ArEg) => "لسه",
        (RevNotYet, Ar) => "ليس بعد",
        (RevNotYet, En) => "Not yet",
        (RevSecondsLeft, ArEg) => "ثانية فاضلة",
        (RevSecondsLeft, Ar) => "ثانية متبقية",
        (RevSecondsLeft, En) => "seconds left",
        (RevAllRemembered, ArEg) => "افتكرت كله، ولا غلطة.",
        (RevAllRemembered, Ar) => "تذكّرت كل شيء، دون خطأ واحد.",
        (RevAllRemembered, En) => "You remembered every single one.",
        // The Review forecast strip.
        // One spelling of each day across the app (#587): Progress says
        // «الأحد، الثلاثاء» too.
        (MonShort, ArEg | Ar) => "الاثنين",
        (MonShort, En) => "Mon",
        (TueShort, ArEg | Ar) => "الثلاثاء",
        (TueShort, En) => "Tue",
        (WedShort, ArEg | Ar) => "الأربعاء",
        (WedShort, En) => "Wed",
        (ThuShort, ArEg | Ar) => "الخميس",
        (ThuShort, En) => "Thu",
        (FriShort, ArEg | Ar) => "الجمعة",
        (FriShort, En) => "Fri",
        (SatShort, ArEg | Ar) => "السبت",
        (SatShort, En) => "Sat",
        (SunShort, ArEg | Ar) => "الأحد",
        (SunShort, En) => "Sun",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        crate::i18n::checks::catalog(ReviewMsg::ALL, text, |_| false);
    }
}
