//! UI strings in Egyptian Arabic, MSA and English. A typed catalog: a missing
//! translation is a compile error, not a blank button.

use kabar_core::locale::UiLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    WelcomeKabarFrom,
    WelcomeExplain,
    WelcomeFirstWord,
    WelcomeStart,
    NavToday,
    NavJourney,
    NavReader,
    NavReview,
    NavStudio,
    NavSettings,
    NavPenLab,
    NavTeach,
    TodayTitle,
    TodayBody,
    ComingIn,
    SettingsLanguage,
    SettingsTheme,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    SettingsDigits,
    SettingsDevice,
    SettingsShowWelcome,
    PenLabIntro,
    PenLabClear,
    PenLabSave,
    PenLabCopy,
    PenLabCopied,
    PenLabSaved,
    PenLabEvents,
    PenLabSavedRuns,
    Width,
}

pub fn t(lang: UiLanguage, msg: Msg) -> &'static str {
    use Msg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (WelcomeKabarFrom, ArEg) => "كلمة \u{2068}kabar\u{2069} جاية من العربي",
        (WelcomeKabarFrom, Ar) => "كلمة \u{2068}kabar\u{2069} أصلها عربي",
        (WelcomeKabarFrom, En) => "kabar comes from Arabic",
        (WelcomeExplain, ArEg) => {
            "«\u{2068}Apa kabar?\u{2069}» يعني «إيه الأخبار؟» — وإنت أصلاً عارف الكلمة دي."
        }
        (WelcomeExplain, Ar) => "«\u{2068}Apa kabar?\u{2069}» تعني «ما الأخبار؟» — وأنت تعرف هذه الكلمة أصلًا.",
        (WelcomeExplain, En) => "“Apa kabar?” means “what's the news?”, and you already know that word.",
        (WelcomeFirstWord, ArEg) => "أول كلمة ليك: كلمة إنت عارفها.",
        (WelcomeFirstWord, Ar) => "كلمتك الأولى: كلمة تعرفها بالفعل.",
        (WelcomeFirstWord, En) => "Your first word is one you already know.",
        (WelcomeStart, ArEg) => "يلا نبدأ",
        (WelcomeStart, Ar) => "لنبدأ",
        (WelcomeStart, En) => "Let's begin",
        (NavToday, ArEg | Ar) => "اليوم",
        (NavToday, En) => "Today",
        (NavJourney, ArEg | Ar) => "الرحلة",
        (NavJourney, En) => "Journey",
        (NavReader, ArEg | Ar) => "القارئ",
        (NavReader, En) => "Reader",
        (NavReview, ArEg | Ar) => "المراجعة",
        (NavReview, En) => "Review",
        (NavStudio, ArEg | Ar) => "الاستوديو",
        (NavStudio, En) => "Studio",
        (NavSettings, ArEg | Ar) => "الإعدادات",
        (NavSettings, En) => "Settings",
        (NavPenLab, _) => "Pen Lab",
        (NavTeach, ArEg | Ar) => "علّم كبر",
        (NavTeach, En) => "Teach Kabar",
        (TodayTitle, _) => "Rencana Hari Ini",
        (TodayBody, ArEg) => "\u{2068}Kabar\u{2069} لسه بيتبني. الخطة اليومية والمراجعات هتظهر هنا قريب.",
        (TodayBody, Ar) => "\u{2068}Kabar\u{2069} قيد البناء. ستظهر هنا الخطة اليومية والمراجعات قريبًا.",
        (TodayBody, En) => "Kabar is still being built. Your daily plan and reviews will appear here soon.",
        (ComingIn, ArEg) => "جاي في",
        (ComingIn, Ar) => "قادم في",
        (ComingIn, En) => "Coming in",
        (SettingsLanguage, ArEg) => "لغة الشرح",
        (SettingsLanguage, Ar) => "لغة الشرح",
        (SettingsLanguage, En) => "Explanation language",
        (SettingsTheme, ArEg | Ar) => "المظهر",
        (SettingsTheme, En) => "Theme",
        (ThemeSystem, ArEg) => "زي الجهاز",
        (ThemeSystem, Ar) => "حسب النظام",
        (ThemeSystem, En) => "System",
        (ThemeLight, ArEg | Ar) => "فاتح",
        (ThemeLight, En) => "Light",
        (ThemeDark, ArEg | Ar) => "داكن",
        (ThemeDark, En) => "Dark",
        (SettingsDigits, ArEg) => "أرقام عربية (٠١٢٣)",
        (SettingsDigits, Ar) => "أرقام عربية مشرقية (٠١٢٣)",
        (SettingsDigits, En) => "Eastern Arabic digits (٠١٢٣) in Arabic",
        (SettingsDevice, ArEg | Ar) => "معرّف الجهاز",
        (SettingsDevice, En) => "Device ID",
        (SettingsShowWelcome, ArEg) => "اعرض شاشة الترحيب تاني",
        (SettingsShowWelcome, Ar) => "اعرض شاشة الترحيب مجددًا",
        (SettingsShowWelcome, En) => "Show the welcome screen again",
        (PenLabIntro, ArEg) => {
            "اكتب وارسم هنا بالقلم وبصباعك. جرّب تقرّب القلم من غير ما تلمس، وزرار القلم، والممحاة. وبعدين احفظ التقرير وابعته."
        }
        (PenLabIntro, Ar) => {
            "اكتب وارسم هنا بالقلم وبإصبعك. جرّب تقريب القلم دون لمس، وزر القلم، والممحاة. ثم احفظ التقرير وأرسله."
        }
        (PenLabIntro, En) => {
            "Write and draw here with the pen and your finger. Try hovering, the pen button and the eraser. Then save the report and send it."
        }
        (PenLabClear, ArEg) => "امسح",
        (PenLabClear, Ar) => "امسح",
        (PenLabClear, En) => "Clear",
        (PenLabSave, ArEg | Ar) => "احفظ التقرير",
        (PenLabSave, En) => "Save report",
        (PenLabCopy, ArEg | Ar) => "انسخ JSON",
        (PenLabCopy, En) => "Copy JSON",
        (PenLabCopied, ArEg) => "اتنسخ",
        (PenLabCopied, Ar) => "تم النسخ",
        (PenLabCopied, En) => "Copied",
        (PenLabSaved, ArEg) => "اتحفظ",
        (PenLabSaved, Ar) => "تم الحفظ",
        (PenLabSaved, En) => "Saved",
        (PenLabEvents, ArEg | Ar) => "الأحداث",
        (PenLabEvents, En) => "Events",
        (PenLabSavedRuns, ArEg | Ar) => "التقارير المحفوظة",
        (PenLabSavedRuns, En) => "Saved reports",
        (Width, ArEg | Ar) => "عرض الشاشة",
        (Width, En) => "Window width",
    }
}
