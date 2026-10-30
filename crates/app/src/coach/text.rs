//! Today, Kak Nur and onboarding strings added with the 2026-10-29 redesign,
//! typed like `crate::i18n` (a missing translation is a compile error).

use kabar_core::locale::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum T {
    /// The hero's small heading.
    PlanEyebrow,
    /// Above the duration chips.
    HowLong,
    StartPlan,
    ContinuePlan,
    /// Hero's quiet second action (ticks a step that has no lesson record).
    MarkStepDone,
    /// The rest of the plan, below the hero.
    ThenRows,
    AllDoneTitle,
    /// Hero action once the plan is done: the Journey's next lesson.
    OneMoreLesson,
    /// The hero step's kind, for a lesson step.
    LessonIn,
    /// Kak Nur's card on Today opens her page.
    NurOpens,
    /// Kak Nur's page: the next-step card.
    CoachNextTitle,
    CoachDoneTitle,
    EditPlan,
    EditPlanHint,
    Reminders,
    RemindersOn,
    RemindersOff,
    /// Onboarding's fast start, once language and starting point are picked.
    StartNow,
    StartNowHint,
    TargetEstimateHint,
}

pub fn tt(lang: UiLanguage, msg: T) -> &'static str {
    use T::*;
    use UiLanguage::*;
    match (msg, lang) {
        (PlanEyebrow, ArEg) => "خطتك النهارده",
        (PlanEyebrow, Ar) => "خطتك اليوم",
        (PlanEyebrow, En) => "Today's plan",
        (HowLong, ArEg) => "معاك قد إيه؟",
        (HowLong, Ar) => "كم معك من الوقت؟",
        (HowLong, En) => "How long today?",
        (StartPlan, ArEg) => "ابدأ خطتك",
        (StartPlan, Ar) => "ابدأ خطتك",
        (StartPlan, En) => "Start your plan",
        (ContinuePlan, ArEg) => "كمّل خطتك",
        (ContinuePlan, Ar) => "تابع خطتك",
        (ContinuePlan, En) => "Continue your plan",
        (MarkStepDone, ArEg) => "عملتها من برّه؟ علّمها خلصت",
        (MarkStepDone, Ar) => "أنجزتها؟ علّمها منجزة",
        (MarkStepDone, En) => "Did it already? Mark it done",
        (ThenRows, ArEg) => "وبعدها",
        (ThenRows, Ar) => "وبعد ذلك",
        (ThenRows, En) => "Then",
        (AllDoneTitle, ArEg) => "خلّصت خطة النهارده",
        (AllDoneTitle, Ar) => "أنهيت خطة اليوم",
        (AllDoneTitle, En) => "Today's plan is done",
        (OneMoreLesson, ArEg) => "عايز درس كمان؟",
        (OneMoreLesson, Ar) => "درس إضافي؟",
        (OneMoreLesson, En) => "One more lesson?",
        (LessonIn, ArEg) => "درس في",
        (LessonIn, Ar) => "درس في",
        (LessonIn, En) => "Lesson in",
        (NurOpens, ArEg) => "صفحة Kak Nur: تقريرك وإنجازاتك",
        (NurOpens, Ar) => "صفحة Kak Nur: تقريرك وإنجازاتك",
        (NurOpens, En) => "Kak Nur's page: your report and achievements",
        (CoachNextTitle, ArEg) => "خطوتك الجاية",
        (CoachNextTitle, Ar) => "خطوتك التالية",
        (CoachNextTitle, En) => "Your next step",
        (CoachDoneTitle, ArEg) => "يومك كمل",
        (CoachDoneTitle, Ar) => "اكتمل يومك",
        (CoachDoneTitle, En) => "Your day is done",
        (EditPlan, ArEg) => "عدّل الخطة",
        (EditPlan, Ar) => "عدّل الخطة",
        (EditPlan, En) => "Change the plan",
        (EditPlanHint, ArEg) => "المدة، الهدف، ومواعيد المذاكرة",
        (EditPlanHint, Ar) => "المدة والهدف ومواعيد الدراسة",
        (EditPlanHint, En) => "Length, goal and study times",
        (Reminders, ArEg) => "التذكير",
        (Reminders, Ar) => "التذكير",
        (Reminders, En) => "Reminders",
        (RemindersOn, ArEg) => "شغّال على مواعيدك",
        (RemindersOn, Ar) => "مفعّل على مواعيدك",
        (RemindersOn, En) => "On, at your study times",
        (RemindersOff, ArEg) => "مقفول. تقدر تشغّله لو حابب",
        (RemindersOff, Ar) => "متوقف. يمكنك تفعيله متى شئت",
        (RemindersOff, En) => "Off. Turn it on whenever you like",
        (StartNow, ArEg) => "ابدأ دلوقتي، والباقي بعدين",
        (StartNow, Ar) => "ابدأ الآن، والباقي لاحقًا",
        (StartNow, En) => "Start now, the rest later",
        (StartNowHint, ArEg) => {
            "الهدف والمواعيد والتذكير تقدر تظبطهم بعدين من صفحة Kak Nur. التذكير هيفضل مقفول لحد ما تشغّله."
        }
        (StartNowHint, Ar) => {
            "الهدف والمواعيد والتذكير يمكنك ضبطها لاحقًا من صفحة Kak Nur. سيبقى التذكير متوقفًا حتى تفعّله."
        }
        (StartNowHint, En) => {
            "Set a goal, study times and reminders later on Kak Nur's page. Reminders stay off until you turn them on."
        }
        (TargetEstimateHint, ArEg) => {
            "التاريخ ده تقدير على حسب وقتك اليومي، مش ميعاد مضمون. تقدر تعدّله أو تكمل من غير هدف."
        }
        (TargetEstimateHint, Ar) => {
            "هذا التاريخ تقدير مبني على وقتك اليومي، وليس موعدًا مضمونًا. يمكنك تعديله أو المتابعة دون هدف."
        }
        (TargetEstimateHint, En) => {
            "This date estimates progress from your daily study time, not a guaranteed deadline. You can change it or continue without a target."
        }
    }
}

/// "كمّل خطتك · فاضل ٧ دقايق" / "Continue your plan · 7 min left".
pub fn left(lang: UiLanguage, minutes: &str) -> String {
    match lang {
        UiLanguage::ArEg => format!("فاضل {minutes}"),
        UiLanguage::Ar => format!("متبقٍّ {minutes}"),
        UiLanguage::En => format!("{minutes} left"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_left_reads_in_order() {
        assert_eq!(left(UiLanguage::ArEg, "7 دقايق"), "فاضل 7 دقايق");
        assert_eq!(left(UiLanguage::En, "7 min"), "7 min left");
    }
}
