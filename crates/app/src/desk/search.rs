//! Quick search (Ctrl/⌘+K, the sidebar's Search row or the phone's search
//! button): jump to any lesson, screen, game or Settings section. Offline:
//! it searches the curriculum and the catalogs built into the app.
//! Adapted from batna-app main: crates/app/src/shortcuts/palette.rs.

use dioxus::prelude::*;
use kabar_core::journey::Curriculum;
use kabar_core::locale::UiLanguage;

use super::msg::DeskMsg;
use crate::i18n::t;
use crate::play::{self, PlayNav};
use crate::settings::{Section, SettingsNav, search::matches};
use crate::{Screen, Ui};

const MAX_RESULTS: usize = 12;

static OPEN: GlobalSignal<bool> = Signal::global(|| false);

/// Opens or closes quick search. Opening twice is harmless (⌘K reaches it
/// from the Mac menu and from the page).
pub fn open(open: bool) {
    if *OPEN.peek() != open {
        *OPEN.write() = open;
    }
}

/// The screens quick search offers, in the sidebar's order.
const SCREENS: [Screen; 13] = [
    Screen::Today,
    Screen::Journey,
    Screen::Reader,
    Screen::Review,
    Screen::Play,
    Screen::Listen,
    Screen::Chat,
    Screen::Teach,
    Screen::Studio,
    Screen::PenLab,
    Screen::Stats,
    Screen::Coach,
    Screen::Journal,
];

#[derive(Clone, Debug, PartialEq, Eq)]
enum Destination {
    Screen(Screen),
    Settings(Option<Section>),
    Game(play::Route),
    Lesson(String),
}

#[derive(Clone, Debug)]
struct Hit {
    id: String,
    name: String,
    /// Indonesian or Javanese: shown left to right with its own `lang`.
    target: Option<&'static str>,
    subtitle: String,
    destination: Destination,
}

/// Every name in every UI language, so an English word finds a screen in
/// the Arabic UI and the other way round.
fn all_langs(msg: impl crate::i18n::Text) -> String {
    [UiLanguage::ArEg, UiLanguage::Ar, UiLanguage::En].map(|l| t(l, msg)).join(" ")
}

fn lessons(lang: UiLanguage, q: &str, language: &'static str, c: &Curriculum, hits: &mut Vec<Hit>) {
    for level in &c.levels {
        for unit in &level.units {
            for lesson in &unit.lessons {
                let words = format!(
                    "{} {} {} {} {} {} {} {} {} {} {}",
                    lesson.id,
                    lesson.title,
                    lesson.can_do,
                    unit.title.target.as_deref().unwrap_or_default(),
                    unit.title.en,
                    unit.title.ar,
                    unit.stop,
                    level.id,
                    level.code,
                    level.title.en,
                    level.title.ar,
                );
                if matches(q, &words) {
                    hits.push(Hit {
                        id: format!("lesson-{}", lesson.id),
                        name: lesson.title.clone(),
                        target: Some(language),
                        subtitle: format!(
                            "{} · {} · {}",
                            t(lang, DeskMsg::KindLesson),
                            crate::journey::local(lang, &level.title),
                            crate::journey::local(lang, &unit.title),
                        ),
                        destination: Destination::Lesson(lesson.id.clone()),
                    });
                }
            }
        }
    }
}

/// An empty query lists the screens and Settings; a typed one also searches
/// every lesson of both routes, the games and the Settings sections.
fn find(lang: UiLanguage, tablet: bool, query: &str) -> Vec<Hit> {
    let q = query.trim();
    let mut hits = Vec::new();
    for screen in SCREENS.into_iter().filter(|s| tablet || !s.pen_only()) {
        if q.is_empty() || matches(q, &format!("{screen:?} {}", all_langs(screen.label()))) {
            hits.push(Hit {
                id: format!("screen-{screen:?}").to_lowercase(),
                name: t(lang, screen.label()).into(),
                target: None,
                subtitle: t(lang, DeskMsg::KindScreen).into(),
                destination: Destination::Screen(screen),
            });
        }
    }
    let settings = crate::i18n::Msg::NavSettings;
    if q.is_empty() || matches(q, &format!("settings preferences {}", all_langs(settings))) {
        hits.push(Hit {
            id: "settings".into(),
            name: t(lang, settings).into(),
            target: None,
            subtitle: t(lang, DeskMsg::KindSettings).into(),
            destination: Destination::Settings(None),
        });
    }
    if q.is_empty() {
        return hits;
    }
    for game in &play::GAMES {
        if matches(q, &format!("{:?} {} {}", game.route, all_langs(game.title), all_langs(game.blurb))) {
            hits.push(Hit {
                id: format!("game-{:?}", game.route).to_lowercase(),
                name: t(lang, game.title).into(),
                target: game.route.latin_title().then_some("id"),
                subtitle: t(lang, DeskMsg::KindGame).into(),
                destination: Destination::Game(game.route),
            });
        }
    }
    for section in Section::ALL.into_iter().filter(|s| s.shown(tablet)) {
        if section.matches(q) {
            hits.push(Hit {
                id: format!("settings-{}", section.id()),
                name: section.label(lang).into(),
                target: None,
                subtitle: t(lang, DeskMsg::KindSettings).into(),
                destination: Destination::Settings(Some(section)),
            });
        }
    }
    for (language, code) in [("id", "id"), ("jv", "jv")] {
        if let Some(c) = crate::journey::curriculum(code) {
            lessons(lang, q, language, c, &mut hits);
        }
    }
    // Exact names and ids first, then names that start with the query, then
    // the rest; a stable sort keeps the curriculum's order inside each band.
    let lower = q.to_lowercase();
    hits.sort_by_key(|h| {
        let name = h.name.to_lowercase();
        let id = h.id.to_lowercase();
        if name == lower || id.ends_with(&format!("-{lower}")) {
            0
        } else if name.starts_with(&lower) {
            1
        } else {
            2
        }
    });
    hits.truncate(MAX_RESULTS);
    hits
}

/// Goes where a result points, along the same paths as the sidebar, the
/// Play hub and the Journey Map; Settings stays a sheet over the screen.
fn activate(target: Destination, ui: Ui, settings: SettingsNav, play_nav: PlayNav) {
    open(false);
    if !matches!(target, Destination::Settings(_)) {
        settings.close();
    }
    match target {
        Destination::Screen(Screen::Stats) => crate::stats::open_from_nav(ui),
        Destination::Screen(screen) => {
            if screen == Screen::Play {
                let PlayNav(mut nav) = play_nav;
                nav.set(play::Route::Hub);
            }
            let mut s = ui.screen;
            s.set(screen);
        }
        Destination::Settings(section) => settings.show(section),
        Destination::Game(route) => play::open(ui, play_nav, route),
        Destination::Lesson(id) => ui.open_lesson(id),
    }
}

/// Closing puts the keyboard back on the control that opened search.
fn dismiss() {
    open(false);
    document::eval(
        "requestAnimationFrame(() => { const el = window.__kabarSearchOpener; if (el?.isConnected && el.getClientRects().length) el.focus({ preventScroll: true }); });",
    );
}

/// The search button: a row in the sidebar and rail, an icon in the
/// phone's top bar.
#[component]
pub fn SearchButton(#[props(default)] compact: bool) -> Element {
    let ui = use_context::<Ui>();
    let label = ui.t(DeskMsg::Search);
    if compact {
        return rsx! {
            button {
                class: "icon-button gear-button qs-open",
                aria_label: "{label}",
                title: "{label}",
                aria_haspopup: "dialog",
                onclick: move |_| open(true),
                {crate::icons::search()}
            }
        };
    }
    rsx! {
        button {
            class: "nav-item secondary nav-search qs-open",
            aria_haspopup: "dialog",
            aria_keyshortcuts: "Meta+K Control+K",
            title: "{label} (⌘K)",
            onclick: move |_| open(true),
            span { class: "nav-icon", aria_hidden: "true", {crate::icons::search()} }
            span { class: "nav-label", "{label}" }
            kbd { class: "nav-kbd", dir: "ltr", "⌘K" }
        }
    }
}

/// Mounted once by the shell; draws nothing until search is opened.
#[component]
pub fn QuickSearch() -> Element {
    let ui = use_context::<Ui>();
    let settings = use_context::<SettingsNav>();
    let play_nav = use_context::<PlayNav>();
    let mut query = use_signal(String::new);
    // Each opening starts with an empty field.
    use_effect(move || {
        if !*OPEN.read() {
            query.set(String::new());
        }
    });
    if !*OPEN.read() {
        return rsx! {};
    }
    let lang = ui.lang();
    let tablet = *ui.tablet.read();
    let results = find(lang, tablet, &query.read());
    rsx! {
        div { class: "qs-backdrop", onclick: move |_| dismiss() }
        section {
            class: "quick-search",
            role: "dialog",
            aria_modal: "true",
            aria_labelledby: "qs-title",
            onmounted: move |_| {
                document::eval(
                    "window.__kabarSearchOpener = document.activeElement; requestAnimationFrame(() => document.querySelector('.qs-field')?.focus());",
                );
            },
            div { class: "qs-head",
                h2 { id: "qs-title", "{ui.t(DeskMsg::SearchTitle)}" }
                button {
                    class: "icon-button",
                    "data-back": "true",
                    aria_label: "{ui.t(DeskMsg::SearchClose)}",
                    title: "{ui.t(DeskMsg::SearchClose)}",
                    onclick: move |_| dismiss(),
                    span { aria_hidden: "true", "✕" }
                }
            }
            div { class: "qs-row",
                span { class: "qs-glass", aria_hidden: "true", {crate::icons::search()} }
                input {
                    class: "qs-field",
                    r#type: "search",
                    aria_label: "{ui.t(DeskMsg::SearchField)}",
                    placeholder: "{ui.t(DeskMsg::SearchPlaceholder)}",
                    autocomplete: "off",
                    spellcheck: "false",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                }
                kbd { class: "qs-kbd", dir: "ltr", "⌘K" }
            }
            nav { class: "qs-results", aria_label: "{ui.t(DeskMsg::SearchResults)}",
                if results.is_empty() {
                    p { class: "qs-empty", role: "status", "{ui.t(DeskMsg::SearchEmpty)}" }
                }
                for hit in results {
                    button {
                        key: "{hit.id}",
                        class: "qs-result",
                        onclick: {
                            let to = hit.destination.clone();
                            move |_| activate(to.clone(), ui, settings, play_nav)
                        },
                        if let Some(code) = hit.target {
                            span { class: "qs-name latin", lang: code, bdi { dir: "ltr", "{hit.name}" } }
                        } else {
                            span { class: "qs-name", "{hit.name}" }
                        }
                        span { class: "qs-sub", "{hit.subtitle}" }
                    }
                }
            }
            p { class: "qs-hint", dir: "auto", "{ui.t(DeskMsg::SearchHint)}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dests(lang: UiLanguage, q: &str) -> Vec<Destination> {
        find(lang, false, q).into_iter().map(|h| h.destination).collect()
    }

    #[test]
    fn an_empty_query_lists_the_screens_and_settings_only() {
        let hits = dests(UiLanguage::ArEg, "");
        assert!(hits.contains(&Destination::Screen(Screen::Today)));
        assert!(hits.contains(&Destination::Settings(None)));
        assert!(hits.iter().all(|d| matches!(d, Destination::Screen(_) | Destination::Settings(None))));
        // Pen tools only on a tablet.
        assert!(!hits.contains(&Destination::Screen(Screen::Studio)));
        let tablet: Vec<_> = find(UiLanguage::ArEg, true, "").into_iter().map(|h| h.destination).collect();
        assert!(tablet.contains(&Destination::Screen(Screen::Studio)));
    }

    #[test]
    fn lessons_are_found_by_indonesian_title_english_goal_and_arabic_unit() {
        let first = Destination::Lesson("bipa1.u01.l01".into());
        assert!(dests(UiLanguage::ArEg, "selamat pagi").contains(&first));
        assert!(dests(UiLanguage::En, "greet by time").contains(&first));
        assert!(dests(UiLanguage::ArEg, "التحية").contains(&first), "the unit's Arabic title");
        assert!(dests(UiLanguage::En, "bipa1.u01.l01")[0] == first, "an exact id comes first");
        // Javanese lessons too.
        let jv =
            crate::journey::curriculum("jv").and_then(|c| c.lessons().next()).expect("a Javanese lesson");
        assert!(dests(UiLanguage::En, &jv.id).contains(&Destination::Lesson(jv.id.clone())));
    }

    #[test]
    fn screens_games_and_settings_answer_in_any_ui_language() {
        assert!(dests(UiLanguage::ArEg, "review").contains(&Destination::Screen(Screen::Review)));
        assert!(
            dests(UiLanguage::En, t(UiLanguage::ArEg, Screen::Reader.label()))
                .contains(&Destination::Screen(Screen::Reader))
        );
        assert!(dests(UiLanguage::ArEg, "pasar").contains(&Destination::Game(play::Route::Pasar)));
        assert!(
            dests(UiLanguage::En, "keyboard shortcuts").contains(&Destination::Settings(Some(Section::Mac)))
        );
        assert!(dests(UiLanguage::En, "zzzz-nothing").is_empty());
    }

    #[test]
    fn results_are_capped() {
        assert!(find(UiLanguage::En, false, "a").len() <= MAX_RESULTS);
    }
}
