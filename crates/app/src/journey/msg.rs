//! UI strings for the Journey map (the older strings; newer ones are in `text.rs`), typed like `crate::i18n` (a missing
//! translation is a compile error). Moved out of the shared `Msg` catalog so
//! this feature's changes touch only its own file.

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Names kept from the shared catalog they moved out of.
#[allow(clippy::enum_variant_names)]
pub enum JourneyMsg {
    JourneyMap,
    JourneyMapOfIndonesia,
    JourneyMapOfJava,
    JourneyContinue,
    JourneyTakeFlight,
    JourneyTakeExpress,
    JourneyBestAfter,
    JourneyOpenAnyway,
    JourneySuitcase,
    JourneySuitcaseEmpty,
    JourneyUnverified,
    JourneyDone,
    JourneyAllDone,
    JourneyZoomIn,
    JourneyZoomOut,
    JourneyZoomReset,
    JourneyClose,
    JourneyLoadError,
}
}

impl crate::i18n::Text for JourneyMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: JourneyMsg) -> &'static str {
    use JourneyMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (JourneyMap, ArEg | Ar) => "الخريطة",
        (JourneyMap, En) => "Map",
        (JourneyMapOfIndonesia, ArEg | Ar) => "خريطة إندونيسيا",
        (JourneyMapOfIndonesia, En) => "Map of Indonesia",
        (JourneyMapOfJava, ArEg | Ar) => "خريطة جاوة",
        (JourneyMapOfJava, En) => "Map of Java",
        (JourneyContinue, ArEg) => "كمّل",
        (JourneyContinue, Ar) => "تابِع",
        (JourneyContinue, En) => "Continue",
        (JourneyTakeFlight, ArEg) => "اركب الطيارة: اختبار تحديد مستوى",
        (JourneyTakeFlight, Ar) => "سافر بالطائرة: اختبار تحديد المستوى",
        (JourneyTakeFlight, En) => "Take a flight: placement test",
        (JourneyTakeExpress, ArEg) => "اركب القطر السريع: اختبار تحديد مستوى",
        (JourneyTakeExpress, Ar) => "اركب القطار السريع: اختبار تحديد المستوى",
        (JourneyTakeExpress, En) => "Take the express: placement test",
        (JourneyBestAfter, ArEg) => "أحسن تبدأه بعد",
        (JourneyBestAfter, Ar) => "يُفضَّل أن تبدأه بعد",
        (JourneyBestAfter, En) => "Best started after",
        (JourneyOpenAnyway, ArEg) => "بس هو مفتوح لو حابب تبص عليه دلوقتي.",
        (JourneyOpenAnyway, Ar) => "لكنه مفتوح إن أحببت أن تلقي نظرة الآن.",
        (JourneyOpenAnyway, En) => "It's open now if you'd like a look.",
        (JourneySuitcase, ArEg) => "الشنطة",
        (JourneySuitcase, Ar) => "حقيبة السفر",
        (JourneySuitcase, En) => "Suitcase",
        (JourneySuitcaseEmpty, ArEg) => "خلّص أول محطة، وهتلاقي أول تذكار مستنيك هنا.",
        (JourneySuitcaseEmpty, Ar) => "أنهِ أول محطة، وستجد تذكارك الأول بانتظارك هنا.",
        (JourneySuitcaseEmpty, En) => "Finish your first stop and your first souvenir will be waiting here.",
        (JourneyUnverified, ArEg) => "لسه متراجعش",
        (JourneyUnverified, Ar) => "لم يُراجَع بعد",
        (JourneyUnverified, En) => "Not yet verified",
        (JourneyDone, ArEg) => "خلصته",
        (JourneyDone, Ar) => "مكتمل",
        (JourneyDone, En) => "Done",
        (JourneyAllDone, ArEg) => "خلصت المسار كله. \u{2068}Selamat!\u{2069}",
        (JourneyAllDone, Ar) => "أنهيت المسار كله. \u{2068}Selamat!\u{2069}",
        (JourneyAllDone, En) => "You've finished the whole route. Selamat!",
        (JourneyZoomIn, ArEg) => "قرّب",
        (JourneyZoomIn, Ar) => "تكبير",
        (JourneyZoomIn, En) => "Zoom in",
        (JourneyZoomOut, ArEg) => "بعّد",
        (JourneyZoomOut, Ar) => "تصغير",
        (JourneyZoomOut, En) => "Zoom out",
        (JourneyZoomReset, ArEg) => "رجّع الخريطة زي ما كانت",
        (JourneyZoomReset, Ar) => "إعادة الخريطة كما كانت",
        (JourneyZoomReset, En) => "Reset the map",
        (JourneyClose, ArEg) => "اقفل",
        (JourneyClose, Ar) => "إغلاق",
        (JourneyClose, En) => "Close",
        (JourneyLoadError, ArEg) => "معرفناش نقرا المنهج. ده غلط عندنا مش عندك.",
        (JourneyLoadError, Ar) => "تعذّرت قراءة المنهج. الخطأ من جهتنا لا من جهتك.",
        (JourneyLoadError, En) => "Kabar couldn't read the curriculum. That's on us, not you.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        crate::i18n::checks::catalog(JourneyMsg::ALL, text, |_| false);
    }
}
