//! UI strings for the desk tools (drag-to-resize panes, focus mode, quick
//! search), typed like `crate::i18n` (a missing translation is a compile
//! error).

use kabar_core::locale::UiLanguage;

crate::i18n::catalog! {
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeskMsg {
    Search,
    SearchTitle,
    SearchField,
    SearchPlaceholder,
    SearchResults,
    SearchEmpty,
    SearchHint,
    SearchClose,
    KindScreen,
    KindSettings,
    KindGame,
    KindLesson,
    FocusOn,
    FocusOff,
    ResizeNav,
    ResizePanel,
    ResizeHelp,
}
}

impl crate::i18n::Text for DeskMsg {
    fn text(self, lang: UiLanguage) -> &'static str {
        text(lang, self)
    }
}

fn text(lang: UiLanguage, msg: DeskMsg) -> &'static str {
    use DeskMsg::*;
    use UiLanguage::*;
    match (msg, lang) {
        (Search, ArEg | Ar) => "بحث سريع",
        (Search, En) => "Quick search",
        (SearchTitle, ArEg) => "روح لأي حتة",
        (SearchTitle, Ar) => "انتقل إلى أي مكان",
        (SearchTitle, En) => "Go anywhere",
        (SearchField, ArEg) => "دوّر على درس أو شاشة أو لعبة",
        (SearchField, Ar) => "ابحث عن درس أو شاشة أو لعبة",
        (SearchField, En) => "Search lessons, screens and games",
        (SearchPlaceholder, ArEg) => "اكتب اسم درس أو شاشة أو لعبة…",
        (SearchPlaceholder, Ar) => "اكتب اسم درس أو شاشة أو لعبة…",
        (SearchPlaceholder, En) => "Type a lesson, screen or game…",
        (SearchResults, ArEg | Ar) => "النتائج",
        (SearchResults, En) => "Results",
        (SearchEmpty, ArEg) => "مفيش حاجة بالاسم ده. جرّب كلمة تانية، بالعربي أو الإندونيسي.",
        (SearchEmpty, Ar) => "لا شيء بهذا الاسم. جرّب كلمة أخرى، بالعربية أو الإندونيسية.",
        (SearchEmpty, En) => "Nothing by that name. Try another word, in English or Indonesian.",
        (SearchHint, ArEg) => "↑ ↓ تتنقّل · Enter تفتح · Esc تقفل",
        (SearchHint, Ar) => "↑ ↓ للتنقل · Enter للفتح · Esc للإغلاق",
        (SearchHint, En) => "↑ ↓ to move · Enter to open · Esc to close",
        (SearchClose, ArEg) => "اقفل البحث",
        (SearchClose, Ar) => "أغلق البحث",
        (SearchClose, En) => "Close search",
        (KindScreen, ArEg | Ar) => "شاشة",
        (KindScreen, En) => "Screen",
        (KindSettings, ArEg | Ar) => "الإعدادات",
        (KindSettings, En) => "Settings",
        (KindGame, ArEg | Ar) => "لعبة",
        (KindGame, En) => "Game",
        (KindLesson, ArEg | Ar) => "درس",
        (KindLesson, En) => "Lesson",
        (FocusOn, ArEg | Ar) => "وضع التركيز",
        (FocusOn, En) => "Focus mode",
        (FocusOff, ArEg) => "اخرج من التركيز",
        (FocusOff, Ar) => "اخرج من وضع التركيز",
        (FocusOff, En) => "Leave focus mode",
        (ResizeNav, ArEg) => "عرض القايمة الجانبية",
        (ResizeNav, Ar) => "عرض الشريط الجانبي",
        (ResizeNav, En) => "Sidebar width",
        (ResizePanel, ArEg) => "عرض كارت الكلمة",
        (ResizePanel, Ar) => "عرض بطاقة الكلمة",
        (ResizePanel, En) => "Word panel width",
        (ResizeHelp, ArEg) => "اسحب تعرّضها أو تضيّقها، أو استعمل الأسهم. دبل كليك ترجّعها زي ما كانت.",
        (ResizeHelp, Ar) => "اسحب لتوسيعها أو تضييقها، أو استخدم الأسهم. انقر مرتين لإعادتها كما كانت.",
        (ResizeHelp, En) => "Drag to widen or narrow it, or use the arrow keys. Double-click to reset.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_right_in_every_language() {
        // The hint names the keys as printed on them (Enter, Esc).
        crate::i18n::checks::catalog(DeskMsg::ALL, text, |m| m == DeskMsg::SearchHint);
    }
}
