//! The phone's navigation beyond its four main tabs: the "More" tab and its
//! sheet (Games, Watch & listen, Chat, Progress, Kak Nur, Journal, Teach
//! Kabar, Settings), and the top bar, which shows a back arrow and the screen's name
//! on every screen that isn't a tab.

use dioxus::prelude::*;
use kabar_core::locale::{Direction, UiLanguage};

use crate::i18n::Msg;
use crate::settings::{GearButton, SettingsNav};
use crate::{Screen, Ui, icons};

/// The phone's bottom tabs besides More.
pub const TABS: [Screen; 4] = [Screen::Today, Screen::Journey, Screen::Reader, Screen::Review];

/// What the More sheet lists, in order.
const MORE: [Screen; 7] = [
    Screen::Play,
    Screen::Listen,
    Screen::Chat,
    Screen::Stats,
    Screen::Coach,
    Screen::Journal,
    Screen::Teach,
];

/// Whether the More tab is "where you are" on the phone.
pub fn under_more(screen: Screen) -> bool {
    !TABS.contains(&screen) && !matches!(screen, Screen::Lesson | Screen::LevelCheck)
}

#[derive(Clone, Copy)]
pub struct MoreNav(pub Signal<bool>);

impl MoreNav {
    pub fn new() -> Self {
        Self(Signal::new(false))
    }
}

fn summary(lang: UiLanguage, screen: Screen) -> &'static str {
    use UiLanguage::*;
    match (screen, lang) {
        (Screen::Play, ArEg) => "Kabar Pagi وبطاقات وألعاب قصيرة",
        (Screen::Play, Ar) => "Kabar Pagi وبطاقات وألعاب قصيرة",
        (Screen::Play, En) => "Kabar Pagi, the deck and short games",
        (Screen::Listen, ArEg) => "المسلسل وراديو Kabar",
        (Screen::Listen, Ar) => "المسلسل وراديو Kabar",
        (Screen::Listen, En) => "Sinetron and Kabar Radio",
        (Screen::Chat, ArEg) => "دردش مع شخصيات من كل مستوى",
        (Screen::Chat, Ar) => "حادث شخصيات من كل مستوى",
        (Screen::Chat, En) => "Talk with a character from each level",
        (Screen::Coach, ArEg) => "التقرير الأسبوعي والإنجازات والوارونج",
        (Screen::Coach, Ar) => "التقرير الأسبوعي والإنجازات والوارونج",
        (Screen::Coach, En) => "Weekly report, achievements, the Warung",
        (Screen::Journal, ArEg) => "كل يوم ذاكرته وتأملاتك",
        (Screen::Journal, Ar) => "كل يوم درسته وتأملاتك",
        (Screen::Journal, En) => "Every study day and your reflections",
        (Screen::Teach, ArEg) => "علّم Kabar كلمة جديدة لقيتها",
        (Screen::Teach, Ar) => "علّم Kabar كلمة جديدة وجدتها",
        (Screen::Teach, En) => "Teach Kabar a word you found",
        (Screen::Stats, ArEg) => "حفظك ونشاطك ومهاراتك واختبار المستوى",
        (Screen::Stats, Ar) => "حفظك ونشاطك ومهاراتك واختبار المستوى",
        (Screen::Stats, En) => "Retention, activity, skills and the level check",
        (_, ArEg) => "اللغة والشكل والجهاز والحساب",
        (_, Ar) => "اللغة والمظهر والجهاز والحساب",
        (_, En) => "Language, look, this device, account",
    }
}

/// Shortcuts into a screen's inner page, so they are easy to find on the
/// phone: the Reader's library and the Arabic Bridge deck in Games.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Shortcut {
    Library,
    Bridge,
}

fn shortcut_text(lang: UiLanguage, s: Shortcut) -> (&'static str, &'static str) {
    use UiLanguage::*;
    match (s, lang) {
        (Shortcut::Library, ArEg) => ("المكتبة", "النصوص اللي حفظتها أو استوردتها"),
        (Shortcut::Library, Ar) => ("المكتبة", "النصوص التي حفظتها أو استوردتها"),
        (Shortcut::Library, En) => ("Library", "Texts you saved or imported"),
        (Shortcut::Bridge, ArEg) => ("الجسر العربي", "كلمات إندونيسية جاية من العربي"),
        (Shortcut::Bridge, Ar) => ("الجسر العربي", "كلمات إندونيسية أصلها عربي"),
        (Shortcut::Bridge, En) => ("Arabic Bridge", "Indonesian words that came from Arabic"),
    }
}

fn more_label(lang: UiLanguage) -> &'static str {
    match lang {
        UiLanguage::ArEg | UiLanguage::Ar => "المزيد",
        UiLanguage::En => "More",
    }
}

/// A shorter name where the full one wraps in the narrow rail.
pub fn rail_label(lang: UiLanguage, screen: Screen) -> Option<&'static str> {
    use UiLanguage::*;
    Some(match (screen, lang) {
        (Screen::Listen, ArEg) => "اسمع",
        (Screen::Listen, Ar) => "استمع",
        (Screen::Listen, En) => "Listen",
        (Screen::Teach, ArEg | Ar) => "علّم",
        (Screen::Teach, En) => "Teach",
        _ => return None,
    })
}

/// The phone's fifth tab.
#[component]
pub fn MoreTab(current: Screen) -> Element {
    let ui = use_context::<Ui>();
    let MoreNav(mut open) = use_context::<MoreNav>();
    let here = under_more(current) || open();
    rsx! {
        button {
            class: "nav-item nav-more",
            aria_current: if here { "page" } else { "false" },
            aria_haspopup: "dialog",
            onclick: move |_| open.toggle(),
            span { class: "nav-icon", aria_hidden: "true", {icons::more()} }
            span { "{more_label(ui.lang())}" }
        }
    }
}

/// The sheet the More tab opens.
#[component]
pub fn MoreSheet() -> Element {
    let ui = use_context::<Ui>();
    let MoreNav(mut open) = use_context::<MoreNav>();
    let settings = use_context::<SettingsNav>();
    if !open() {
        return rsx! {};
    }
    let lang = ui.lang();
    let current = *ui.screen.read();
    let close_label = match lang {
        UiLanguage::ArEg => "اقفل",
        UiLanguage::Ar => "إغلاق",
        UiLanguage::En => "Close",
    };
    rsx! {
        div { class: "more-backdrop", onclick: move |_| open.set(false) }
        div { class: "more-sheet", role: "dialog", aria_modal: "true", aria_label: "{more_label(lang)}",
            div { class: "more-head",
                h2 { "{more_label(lang)}" }
                button {
                    class: "icon-button",
                    "data-back": "true",
                    aria_label: "{close_label}",
                    onclick: move |_| open.set(false),
                    span { aria_hidden: "true", "✕" }
                }
            }
            ul { class: "more-list",
                for screen in MORE {
                    li { key: "{screen:?}",
                        button {
                            aria_current: if screen == current { "page" } else { "false" },
                            onclick: move |_| {
                                open.set(false);
                                if screen == Screen::Play {
                                    let crate::play::PlayNav(mut nav) = consume_context::<crate::play::PlayNav>();
                                    nav.set(crate::play::Route::Hub);
                                }
                                // Stats remembers where Back returns to.
                                if screen == Screen::Stats {
                                    crate::stats::open(ui, screen);
                                } else {
                                    let mut s = ui.screen;
                                    s.set(screen);
                                }
                            },
                            span { class: "nav-icon", aria_hidden: "true", {icons::nav(screen)} }
                            span { class: "more-title", "{ui.t(screen.label())}" }
                            span { class: "more-sub", "{summary(lang, screen)}" }
                        }
                    }
                }
                for shortcut in [Shortcut::Library, Shortcut::Bridge] {
                    li { key: "{shortcut:?}",
                        button {
                            onclick: move |_| {
                                open.set(false);
                                let screen = match shortcut {
                                    Shortcut::Library => {
                                        consume_context::<crate::reader::ReaderState>().close();
                                        Screen::Reader
                                    }
                                    Shortcut::Bridge => {
                                        let crate::play::PlayNav(mut nav) = consume_context::<crate::play::PlayNav>();
                                        nav.set(crate::play::Route::Deck);
                                        Screen::Play
                                    }
                                };
                                let mut s = ui.screen;
                                s.set(screen);
                            },
                            span { class: "nav-icon", aria_hidden: "true",
                                match shortcut {
                                    Shortcut::Library => icons::nav(Screen::Reader),
                                    Shortcut::Bridge => icons::game(crate::play::Route::Deck),
                                }
                            }
                            span { class: "more-title", "{shortcut_text(lang, shortcut).0}" }
                            span { class: "more-sub", "{shortcut_text(lang, shortcut).1}" }
                        }
                    }
                }
                li { key: "settings",
                    button {
                        onclick: move |_| {
                            open.set(false);
                            settings.show(None);
                        },
                        span { class: "nav-icon", aria_hidden: "true", {icons::gear()} }
                        span { class: "more-title", "{ui.t(Msg::NavSettings)}" }
                        span { class: "more-sub", "{summary(lang, Screen::Settings)}" }
                    }
                }
            }
        }
    }
}

/// The phone's top bar: the brand on a tab, or a back arrow and the screen's
/// name elsewhere; the gear on every screen.
#[component]
pub fn TopBar(current: Screen) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let tab = TABS.contains(&current);
    let back_label = match lang {
        UiLanguage::ArEg | UiLanguage::Ar => "رجوع",
        UiLanguage::En => "Back",
    };
    rsx! {
        header { class: "top-bar",
            if tab {
                span { class: "brand-small latin", "Kabar" }
            } else {
                div { class: "top-title",
                    button {
                        class: "icon-button gear-button",
                        aria_label: "{back_label}",
                        title: "{back_label}",
                        // The same path as the system Back (back.rs).
                        onclick: move |_| {
                            document::eval("history.back();");
                        },
                        span { aria_hidden: "true", if lang.dir() == Direction::Rtl { "→" } else { "←" } }
                    }
                    h1 { "{ui.t(current.label())}" }
                }
            }
            GearButton {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_screen_has_a_home_on_the_phone() {
        for s in [
            Screen::Play,
            Screen::Listen,
            Screen::Chat,
            Screen::Stats,
            Screen::Coach,
            Screen::Journal,
            Screen::Teach,
        ] {
            assert!(MORE.contains(&s) && under_more(s), "{s:?} must be in More");
        }
        for s in TABS {
            assert!(!under_more(s));
        }
        assert!(!under_more(Screen::Lesson));
    }
}
