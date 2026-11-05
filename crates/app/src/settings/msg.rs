//! UI strings for Settings (the older strings; newer ones are in `text.rs`), typed like `crate::i18n` (a missing
//! translation is a compile error). Moved out of the shared `Msg` catalog so
//! this feature's changes touch only its own file.

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsMsg {
    SettingsLanguage,
    SettingsTheme,
    SettingsDigits,
    SettingsDevice,
    SettingsShowWelcome,
    SettingsSounds,
    SettingsPron,
    SettingsRetention,
    RetentionLight,
    RetentionBalanced,
    RetentionFirm,
    RetentionTuned,
    RetentionLearning,
}
}

impl crate::i18n::Text for SettingsMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: SettingsMsg) -> &'static str {
    use SettingsMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (SettingsLanguage, ArEg) => "لغة الشرح",
        (SettingsLanguage, Ar) => "لغة الشرح",
        (SettingsLanguage, En) => "Explanation language",
        (SettingsTheme, ArEg | Ar) => "المظهر",
        (SettingsTheme, En) => "Theme",
        (SettingsDigits, ArEg) => "أرقام عربية (٠١٢٣)",
        (SettingsDigits, Ar) => "أرقام عربية مشرقية (٠١٢٣)",
        (SettingsDigits, En) => "Eastern Arabic digits (٠١٢٣) in Arabic",
        (SettingsDevice, ArEg | Ar) => "معرّف الجهاز",
        (SettingsDevice, En) => "Device ID",
        (SettingsShowWelcome, ArEg) => "اعرض شاشة الترحيب تاني",
        (SettingsShowWelcome, Ar) => "اعرض شاشة الترحيب مجددًا",
        (SettingsShowWelcome, En) => "Show the welcome screen again",
        (SettingsSounds, ArEg) => "أصوات الدروس والمراجعة",
        (SettingsSounds, Ar) => "أصوات الدروس والمراجعة",
        (SettingsSounds, En) => "Lesson and review sounds",
        (SettingsPron, ArEg) => "تعريب النطق (النطق بالحروف العربي)",
        (SettingsPron, Ar) => "النطق بالحروف العربية",
        (SettingsPron, En) => "Pronunciation in Arabic script",
        (SettingsRetention, ArEg) => "عايز تفتكر الكلمات قد إيه؟",
        (SettingsRetention, Ar) => "إلى أي حد تريد أن تتذكر الكلمات؟",
        (SettingsRetention, En) => "How firmly to remember words",
        (RetentionLight, ArEg) => "خفيف",
        (RetentionLight, Ar) => "خفيف",
        (RetentionLight, En) => "Light",
        (RetentionBalanced, ArEg) => "متوازن",
        (RetentionBalanced, Ar) => "متوازن",
        (RetentionBalanced, En) => "Balanced",
        (RetentionFirm, ArEg) => "ثابت",
        (RetentionFirm, Ar) => "ثابت",
        (RetentionFirm, En) => "Firm",
        (RetentionTuned, ArEg) => "المواعيد متظبطة على ذاكرتك انت، من {n|مراجعة|مراجعتين|مراجعات}.",
        (RetentionTuned, Ar) => "المواعيد مضبوطة على ذاكرتك أنت، من {n|مراجعة|مراجعتين|مراجعات|مراجعةً}.",
        (RetentionTuned, En) => "Timing is tuned to your own memory, from {n|review|reviews}.",
        (RetentionLearning, ArEg) => {
            "\u{2068}Kabar\u{2069} بيتعلم ذاكرتك: بعد حوالي {n|مراجعة|مراجعتين|مراجعات} هيظبط المواعيد عليك انت."
        }
        (RetentionLearning, Ar) => {
            "يتعلّم \u{2068}Kabar\u{2069} ذاكرتك: بعد نحو {n|مراجعة|مراجعتين|مراجعات|مراجعةً} سيضبط المواعيد عليك أنت."
        }
        (RetentionLearning, En) => {
            "Kabar is learning your memory: after about {n|review|reviews} it tunes the timing to you."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        crate::i18n::checks::catalog(SettingsMsg::ALL, text, |_| false);
    }
}
