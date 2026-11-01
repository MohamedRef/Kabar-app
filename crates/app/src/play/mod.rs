//! Play (spec §12, §14): Kabar Pagi, the Arabic Bridge games (the "you
//! already know N words" deck, Sound-Shift Detective, False Friends Alarm),
//! Rupiah Reader and Pasar!. The game logic lives in `kabar_core::games` and
//! `kabar_core::pagi`; these screens draw it and log what was practised.

pub mod alarm;
pub mod bridge_deck;
pub mod content;
pub mod detective;
pub mod logic;
pub mod pagi;
pub mod pairs;
pub mod pasar;
pub mod rupiah;
pub mod speech;
pub mod unggah;

use dioxus::prelude::*;
use kabar_core::games::bridge;

use crate::embedded::{Css, css};
use crate::i18n::Msg;
use crate::{Screen, Ui, icons};
use logic::deck_headline;

pub use pagi::PagiTodayCard;

/// Which Play screen is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Hub,
    Pagi,
    Deck,
    Detective,
    Alarm,
    Rupiah,
    Pasar,
    Unggah,
    Pairs,
}

impl Route {
    /// Whether this game's title is Indonesian/Latin in every UI language
    /// ("Pasar!", "Kabar Pagi"...). Such titles are bidi-isolated as LTR so
    /// trailing punctuation stays put in the Arabic UI. The hub cards and
    /// the game headers both read this one flag.
    pub fn latin_title(self) -> bool {
        matches!(self, Route::Pagi | Route::Pasar | Route::Unggah | Route::Pairs)
    }
}

/// The open Play screen, shared through context so Today can open Kabar Pagi.
#[derive(Clone, Copy)]
pub struct PlayNav(pub Signal<Route>);

impl PlayNav {
    pub fn new() -> Self {
        Self(Signal::new(Route::Hub))
    }
}

/// Opens a Play screen from anywhere.
pub fn open(ui: Ui, nav: PlayNav, route: Route) {
    let PlayNav(mut nav) = nav;
    nav.set(route);
    let mut screen = ui.screen;
    screen.set(Screen::Play);
}

const PLAY_CSS: Css = css!("assets/play.css");

#[component]
pub fn Play() -> Element {
    let PlayNav(nav) = use_context::<PlayNav>();
    let route = *nav.read();
    // Bridge games keep Bridge gold for a right answer; the others use the
    // success colours.
    let bridge = matches!(route, Route::Deck | Route::Detective | Route::Alarm);
    rsx! {
        style { {PLAY_CSS.text()} }
        div { class: if bridge { "play bridge-game" } else { "play" }, {screen(route)} }
    }
}

fn screen(route: Route) -> Element {
    match route {
        Route::Hub => rsx! { Hub {} },
        Route::Pagi => rsx! { pagi::PagiPlayer {} },
        Route::Deck => rsx! { bridge_deck::BridgeDeck {} },
        Route::Detective => rsx! { detective::Detective {} },
        Route::Alarm => rsx! { alarm::Alarm {} },
        Route::Rupiah => rsx! { rupiah::RupiahReader {} },
        Route::Pasar => rsx! { pasar::Pasar {} },
        Route::Unggah => rsx! { unggah::Unggah {} },
        Route::Pairs => rsx! { pairs::Pairs {} },
    }
}

struct GameCard {
    route: Route,
    title: Msg,
    blurb: Msg,
    /// `gold` for Arabic Bridge games, `dawn` for the alarm.
    accent: &'static str,
}

const GAMES: [GameCard; 8] = [
    GameCard { route: Route::Pagi, title: Msg::PagiTitle, blurb: Msg::PagiBlurb, accent: "indigo" },
    GameCard { route: Route::Deck, title: Msg::DeckTitle, blurb: Msg::DeckBlurb, accent: "gold" },
    GameCard {
        route: Route::Detective,
        title: Msg::DetectiveTitle,
        blurb: Msg::DetectiveBlurb,
        accent: "gold",
    },
    GameCard { route: Route::Alarm, title: Msg::AlarmTitle, blurb: Msg::AlarmBlurb, accent: "dawn" },
    GameCard { route: Route::Rupiah, title: Msg::RupiahTitle, blurb: Msg::RupiahBlurb, accent: "sawah" },
    GameCard { route: Route::Pasar, title: Msg::PasarTitle, blurb: Msg::PasarBlurb, accent: "sawah" },
    GameCard { route: Route::Unggah, title: Msg::UnggahTitle, blurb: Msg::UnggahBlurb, accent: "indigo" },
    GameCard { route: Route::Pairs, title: Msg::PairsTitle, blurb: Msg::PairsBlurb, accent: "indigo" },
];

/// The games: tiles by default; a list or one line each, in the hub's
/// order or by name.
const HUB_VIEW: crate::views::Collection = crate::views::Collection {
    key: "play.hub",
    layout: crate::views::Layout::DEFAULT,
    sorts: &[crate::views::Sort::Course, crate::views::Sort::Name],
};

#[component]
fn Hub() -> Element {
    let ui = use_context::<Ui>();
    let PlayNav(nav) = use_context::<PlayNav>();
    let db = use_context::<crate::Db>();
    let known = use_hook(|| content::bridge().as_ref().ok().map(|d| bridge::known_deck(d).len()));
    let pick = HUB_VIEW.pick(&db);
    let mut games: Vec<&GameCard> = GAMES.iter().collect();
    crate::views::sort_by(&mut games, pick.sort, |_| false, |g| ui.t(g.title).to_string());

    rsx! {
        header { class: "play-intro",
            h2 { "{ui.t(Msg::NavPlay)}" }
        }
        crate::views::ViewBar { of: HUB_VIEW }
        ul { class: "coll play-hub", "data-layout": pick.layout.as_str(),
            for game in games {
                li { key: "{game.route:?}",
                    button {
                        class: "coll-btn game-tile {game.accent}",
                        onclick: {
                            let route = game.route;
                            move |_| {
                                let mut nav = nav;
                                nav.set(route);
                            }
                        },
                        span { class: "coll-lead",
                            span { class: "game-icon", aria_hidden: "true", {icons::game(game.route)} }
                        }
                        span { class: "coll-main",
                            if game.route == Route::Deck && known.is_some() {
                                strong { class: "coll-title gold-text", lang: "id",
                                    bdi { {deck_headline(known.unwrap_or_default())} }
                                }
                            } else if game.route.latin_title() {
                                strong { class: "coll-title", lang: "id",
                                    bdi { dir: "ltr", "{ui.t(game.title)}" }
                                }
                            } else {
                                strong { class: "coll-title", "{ui.t(game.title)}" }
                            }
                            span { class: "coll-sub", "{ui.t(game.blurb)}" }
                        }
                        span { class: "chev", aria_hidden: "true" }
                    }
                }
            }
        }
    }
}

/// The title bar of a Play screen, with the way back to the hub.
#[component]
pub fn PlayHeader(title: Msg, #[props(default)] latin: bool) -> Element {
    let ui = use_context::<Ui>();
    let PlayNav(nav) = use_context::<PlayNav>();
    rsx! {
        div { class: "play-header",
            button {
                class: "quiet back",
                "data-back": "true",
                onclick: move |_| {
                    speech::stop();
                    let mut nav = nav;
                    nav.set(Route::Hub);
                },
                span { class: "chev back-chev", aria_hidden: "true" }
                "{ui.t(Msg::NavPlay)}"
            }
            if latin {
                h2 { lang: "id", bdi { dir: "ltr", "{ui.t(title)}" } }
            } else {
                h2 { "{ui.t(title)}" }
            }
        }
    }
}

/// Shown instead of a game when its content didn't load.
#[component]
pub fn ContentProblem(detail: String) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        crate::ui::DivCard {
            p { "{ui.t(Msg::PlayContentProblem)}" }
            details {
                summary { class: "muted small", "{ui.t(Msg::PlayDetails)}" }
                p { class: "mono small", dir: "ltr", bdi { "{detail}" } }
            }
        }
    }
}

/// End of a round: the score, a warm line and "again".
#[component]
pub fn RoundSummary(correct: u32, total: u32, onagain: EventHandler<()>, children: Element) -> Element {
    let ui = use_context::<Ui>();
    let line = if total > 0 && correct * 10 >= total * 8 {
        Msg::PlayGreat
    } else if total > 0 && correct * 2 >= total {
        Msg::PlayGood
    } else {
        Msg::PlayKeepGoing
    };
    let PlayNav(nav) = use_context::<PlayNav>();
    rsx! {
        crate::ui::DivCard { class: "summary-card reveal",
            p { class: "score",
                bdi { dir: "ltr",
                    strong { "{ui.num(correct)}" }
                    span { class: "muted", " / {ui.num(total)}" }
                }
            }
            p { "{ui.t(line)}" }
            {children}
            // What next: the same game again, another game, or the words
            // in a real conversation.
            div { class: "row play-next",
                button { class: "primary", onclick: move |_| onagain.call(()), "{ui.t(Msg::PlayAgain)}" }
                button {
                    onclick: move |_| {
                        speech::stop();
                        let mut nav = nav;
                        nav.set(Route::Hub);
                    },
                    "{ui.t(Msg::PlayOtherGames)}"
                }
                button {
                    class: "quiet-solid",
                    onclick: move |_| {
                        speech::stop();
                        let mut s = ui.screen;
                        s.set(Screen::Chat);
                    },
                    "{ui.t(Msg::PlayTryInChat)}"
                }
            }
        }
    }
}

/// "3 / 8" with a bar: where the player is in a round. The numbers read
/// left to right in every language.
#[component]
pub fn RoundCount(at: u64, total: u64, #[props(default)] label: Option<Msg>) -> Element {
    let ui = use_context::<Ui>();
    let done = at.saturating_sub(1).min(total);
    let percent = (done * 100).checked_div(total).unwrap_or(0);
    rsx! {
        div { class: "play-count",
            span { class: "muted small",
                if let Some(l) = label {
                    "{ui.t(l)} "
                }
                bdi { class: "ltr-count", "{ui.num(at)} / {ui.num(total)}" }
            }
            div { class: "progress-line", aria_hidden: "true",
                span { class: "fill", style: "inline-size: {percent}%" }
            }
        }
    }
}

/// Seconds since `start_ms`, for the activity log.
pub fn seconds_since(start_ms: u64) -> u32 {
    u32::try_from(crate::store::now_ms().saturating_sub(start_ms) / 1000).unwrap_or(u32::MAX)
}

/// A fresh seed for games that should vary each time.
pub fn fresh_seed() -> u64 {
    crate::store::now_ms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kabar_core::locale::UiLanguage;

    fn has_arabic(s: &str) -> bool {
        s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
    }

    /// The hub isolates exactly the titles that stay Latin in the Arabic UI,
    /// so "Pasar!" never renders as "!Pasar" (#147).
    #[test]
    fn latin_title_flag_matches_the_arabic_titles() {
        for game in GAMES.iter() {
            for lang in [UiLanguage::ArEg, UiLanguage::Ar] {
                let title = crate::i18n::t(lang, game.title);
                assert_eq!(game.route.latin_title(), !has_arabic(title), "{:?} {title}", game.route);
            }
        }
        assert!(Route::Pasar.latin_title());
        assert!(!Route::Deck.latin_title());
    }
}
