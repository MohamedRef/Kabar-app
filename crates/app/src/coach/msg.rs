//! UI strings for Kak Nur, Today, onboarding, reminders, report and journal, typed like `crate::i18n` (a missing
//! translation is a compile error). Moved out of the shared `Msg` catalog so
//! this feature's changes touch only its own file.

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoachMsg {
    // --- coach: onboarding ---
    OnbProgress,
    OnbNameLabel,
    OnbWhy,
    OnbWhyNote,
    OnbFocus,
    OnbTarget,
    OnbTargetLevel,
    OnbTargetDate,
    OnbStart,
    OnbPlacementLater,
    // --- coach: settings shared with onboarding ---
    DailySize,
    StudyTimes,
    ReminderStyle,
    QuietHours,
    RemindersElsewhere,
    RemindersBlocked,
    RemindersAllow,
    RemindHere,
    RemindersAsleep,
    RemindersBattery,
    QuietTo,
    // --- coach: today ---
    TodayProgress,
    WelcomeBackTitle,
    WelcomeBackPlan,
    WelcomeBackStart,
    WelcomeBackBigger,
    PelitaReady,
    PlanBreakTitle,
    Freezes,
    MarkDone,
    RecapRemember,
    RecapTomorrow,
    WeekOf,
    Strongest,
    Weakest,
    NextFocus,
    AchievementsTitle,
    Locked,
    WarungExplain,
    BatikClassic,
    InUse,
    NotEnoughKoin,
    KoinToGo,
    JournalTitle,
    JournalReflectionPlaceholder,
    JournalReflectionEdited,
    JournalReflectionLabel,
    JournalSouvenir,
    JournalTaught,
    JournalDraft,
    EvNewWords,
    EvPurchase,
}
}

impl crate::i18n::Text for CoachMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: CoachMsg) -> &'static str {
    use CoachMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        // --- coach: onboarding ---
        (OnbProgress, ArEg | Ar) => "خطوات التعارف",
        (OnbProgress, En) => "Getting to know you",
        (OnbNameLabel, ArEg) => "أناديك بإيه؟",
        (OnbNameLabel, Ar) => "بماذا أناديك؟",
        (OnbNameLabel, En) => "What should I call you?",
        (OnbWhy, ArEg) => "ليه بتتعلم",
        (OnbWhy, Ar) => "لماذا تتعلم",
        (OnbWhy, En) => "Why you're learning",
        (OnbWhyNote, ArEg) => "عايز تحكيلي أكتر؟ (اختياري)",
        (OnbWhyNote, Ar) => "هل تريد أن تخبرني بالمزيد؟ (اختياري)",
        (OnbWhyNote, En) => "Want to tell me more? (optional)",
        (OnbFocus, ArEg | Ar) => "اللغة",
        (OnbFocus, En) => "Language",
        (OnbTarget, ArEg | Ar) => "الهدف",
        (OnbTarget, En) => "Target",
        (OnbTargetLevel, ArEg | Ar) => "المستوى",
        (OnbTargetLevel, En) => "Level",
        (OnbTargetDate, ArEg) => "لحد إمتى",
        (OnbTargetDate, Ar) => "بحلول",
        (OnbTargetDate, En) => "By",
        (OnbStart, ArEg) => "نقطة البداية",
        (OnbStart, Ar) => "نقطة الانطلاق",
        (OnbStart, En) => "Starting point",
        (OnbPlacementLater, ArEg) => "اختبار تحديد المستوى الصغير هيظبط نقطة البداية بعدين.",
        (OnbPlacementLater, Ar) => "سيضبط اختبار تحديد المستوى القصير نقطة البداية لاحقًا.",
        (OnbPlacementLater, En) => "A short placement check will fine-tune this later.",
        // --- coach: settings ---
        (DailySize, ArEg) => "وقتك المعتاد في اليوم",
        (DailySize, Ar) => "وقتك المعتاد يوميًا",
        (DailySize, En) => "Usual time per day",
        (StudyTimes, ArEg) => "مواعيد المذاكرة",
        (StudyTimes, Ar) => "مواعيد الدراسة",
        (StudyTimes, En) => "Study times",
        (ReminderStyle, ArEg) => "التذكيرات",
        (ReminderStyle, Ar) => "أسلوب التذكير",
        (ReminderStyle, En) => "Reminders",
        (QuietHours, ArEg) => "ساعات الهدوء",
        (QuietHours, Ar) => "ساعات الهدوء",
        (QuietHours, En) => "Quiet hours",
        (RemindersElsewhere, ArEg) => {
            "التذكيرات بتوصلك على الموبايل. هنا بتشتغل في تطبيق \u{2068}Kabar\u{2069} المتسطّب بس."
        }
        (RemindersElsewhere, Ar) => {
            "تصلك التذكيرات على الهاتف، وهنا تعمل في تطبيق \u{2068}Kabar\u{2069} المثبّت فقط."
        }
        (RemindersElsewhere, En) => {
            "Reminders reach you on your phone. Here they work in the installed Kabar app."
        }
        (RemindersBlocked, ArEg) => {
            "الإشعارات مقفولة لـ\u{2068}Kabar\u{2069} على الجهاز ده، فمش هقدر أفكّرك هنا."
        }
        (RemindersBlocked, Ar) => {
            "الإشعارات مغلقة لـ\u{2068}Kabar\u{2069} على هذا الجهاز، فلن أستطيع تذكيرك هنا."
        }
        (RemindersBlocked, En) => {
            "Notifications are off for Kabar on this device, so I can't remind you here."
        }
        (RemindersAllow, ArEg) => "اسمح بالإشعارات",
        (RemindersAllow, Ar) => "اسمح بالإشعارات",
        (RemindersAllow, En) => "Allow notifications",
        (RemindHere, ArEg) => "فكّرني على الجهاز ده",
        (RemindHere, Ar) => "ذكّرني على هذا الجهاز",
        (RemindHere, En) => "Remind me on this device",
        (RemindersAsleep, ArEg) => {
            "سامسونج ممكن تنيّم \u{2068}Kabar\u{2069} لو مافتحتهوش كام يوم، وساعتها التذكيرات بتقف. من البطارية اختار «بدون قيود» عشان أفضل أفكّرك."
        }
        (RemindersAsleep, Ar) => {
            "قد يُنيم سامسونج \u{2068}Kabar\u{2069} إن لم تفتحه أيامًا فتتوقف التذكيرات. اختر «غير مقيّد» من إعدادات البطارية لأبقى أذكّرك."
        }
        (RemindersAsleep, En) => {
            "Samsung may put Kabar to sleep after a few days unopened, and reminders stop. Set its battery use to Unrestricted so I can keep reminding you."
        }
        (RemindersBattery, ArEg) => "افتح إعدادات البطارية",
        (RemindersBattery, Ar) => "افتح إعدادات البطارية",
        (RemindersBattery, En) => "Open battery settings",
        (QuietTo, ArEg) => "لحد",
        (QuietTo, Ar) => "إلى",
        (QuietTo, En) => "to",
        // --- coach: today ---
        (TodayProgress, ArEg) => "إنجاز النهارده",
        (TodayProgress, Ar) => "إنجاز اليوم",
        (TodayProgress, En) => "Today's progress",
        (WelcomeBackTitle, ArEg) => "أهلاً بيك تاني",
        (WelcomeBackTitle, Ar) => "مرحبًا بعودتك",
        (WelcomeBackTitle, En) => "Welcome back",
        (WelcomeBackPlan, ArEg) => "خطة النهارده الخفيفة",
        (WelcomeBackPlan, Ar) => "خطة اليوم الخفيفة",
        (WelcomeBackPlan, En) => "Today's light plan",
        (WelcomeBackStart, ArEg) => "يلا نبدأ",
        (WelcomeBackStart, Ar) => "لنبدأ",
        (WelcomeBackStart, En) => "Let's start",
        (WelcomeBackBigger, ArEg) => "لو ليك مزاج لأكتر، اختار خطة أكبر",
        (WelcomeBackBigger, Ar) => "إن رغبت في المزيد، اختر خطة أكبر",
        (WelcomeBackBigger, En) => "In the mood for more? Pick a bigger plan",
        (PelitaReady, ArEg) => "جاهزة تنوّر",
        (PelitaReady, Ar) => "جاهز ليضيء",
        (PelitaReady, En) => "ready to glow",
        (PlanBreakTitle, ArEg) => "استراحة صغيرة",
        (PlanBreakTitle, Ar) => "استراحة قصيرة",
        (PlanBreakTitle, En) => "A short break",
        (Freezes, ArEg) => "تجميد:",
        (Freezes, Ar) => "تجميد:",
        (Freezes, En) => "freezes:",
        (MarkDone, ArEg) => "خلصت",
        (MarkDone, Ar) => "أنجزت",
        (MarkDone, En) => "Done",
        (RecapRemember, ArEg) => "حاجة واحدة افتكرها",
        (RecapRemember, Ar) => "شيء واحد تتذكره",
        (RecapRemember, En) => "One thing to remember",
        (RecapTomorrow, ArEg) => "أول خطوة بكرة",
        (RecapTomorrow, Ar) => "أول خطوة غدًا",
        (RecapTomorrow, En) => "Tomorrow's first step",
        (WeekOf, ArEg) => "أسبوع",
        (WeekOf, Ar) => "أسبوع",
        (WeekOf, En) => "Week of",
        (Strongest, ArEg) => "أقوى مهارة",
        (Strongest, Ar) => "أقوى مهارة",
        (Strongest, En) => "Strongest skill",
        (Weakest, ArEg) => "محتاجة شوية حب",
        (Weakest, Ar) => "تحتاج إلى اهتمام",
        (Weakest, En) => "Needs some love",
        (NextFocus, ArEg) => "تركيز الأسبوع الجاي",
        (NextFocus, Ar) => "تركيز الأسبوع القادم",
        (NextFocus, En) => "Next week's focus",
        (AchievementsTitle, ArEg | Ar) => "الإنجازات",
        (AchievementsTitle, En) => "Achievements",
        (Locked, ArEg) => "لسه جاية",
        (Locked, Ar) => "لم يُفتح بعد",
        (Locked, En) => "Still ahead",
        (WarungExplain, ArEg) => "ثيمات باتيك للزينة بس، بتتشري بالكوين اللي كسبتها",
        (WarungExplain, Ar) => "سمات باتيك تجميلية فقط، تُشترى بالعملات التي كسبتها",
        (WarungExplain, En) => "Batik themes, purely cosmetic, bought with the Koin you earned",
        (BatikClassic, ArEg | Ar) => "الباتيك النيلي الأصلي",
        (BatikClassic, En) => "Classic indigo batik",
        (InUse, ArEg) => "مستخدم دلوقتي",
        (InUse, Ar) => "قيد الاستخدام",
        (InUse, En) => "In use",
        (NotEnoughKoin, ArEg) => "محتاج كوين أكتر شوية",
        (NotEnoughKoin, Ar) => "تحتاج إلى مزيد من العملات",
        (NotEnoughKoin, En) => "A few more Koin needed",
        (KoinToGo, ArEg) => "فاضلك {n}",
        (KoinToGo, Ar) => "يتبقّى لك {n}",
        (KoinToGo, En) => "{n} more",
        (JournalTitle, ArEg) => "يومياتك في التعلم",
        (JournalTitle, Ar) => "سجل التعلّم",
        (JournalTitle, En) => "Learning journal",
        (JournalReflectionPlaceholder, ArEg) => "اتعلمت إيه؟ إيه اللي كان صعب؟ وإيه اللي عجبك؟",
        (JournalReflectionPlaceholder, Ar) => "ماذا تعلّمت؟ ما الذي كان صعبًا؟ وما الذي أعجبك؟",
        (JournalReflectionPlaceholder, En) => "What did you learn? What was hard? What did you enjoy?",
        (JournalReflectionEdited, ArEg) => "اتعدّلت",
        (JournalReflectionEdited, Ar) => "عُدّلت",
        (JournalReflectionEdited, En) => "edited",
        (JournalReflectionLabel, ArEg) => "خاطرتك",
        (JournalReflectionLabel, Ar) => "خاطرتك",
        (JournalReflectionLabel, En) => "Your reflection",
        (JournalSouvenir, ArEg) => "تذكار جديد في الشنطة",
        (JournalSouvenir, Ar) => "تذكار جديد في الحقيبة",
        (JournalSouvenir, En) => "New souvenir in your suitcase",
        (JournalTaught, ArEg | Ar) => "علّمت \u{2068}Kabar\u{2069} كلمة",
        (JournalTaught, En) => "Taught Kabar a word",
        (JournalDraft, ArEg) => "لسه مسودة",
        (JournalDraft, Ar) => "مسودة",
        (JournalDraft, En) => "draft",
        (EvNewWords, ArEg | Ar) => "كلمات جديدة",
        (EvNewWords, En) => "New words",
        (EvPurchase, ArEg) => "اشتريت",
        (EvPurchase, Ar) => "اشتريت",
        (EvPurchase, En) => "Bought",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        crate::i18n::checks::catalog(CoachMsg::ALL, text, |_| false);
    }
}
