//! The Arabic Bridge (spec §12; redesign 2026-10-29): Indonesian words an
//! Egyptian already understands because they came from Arabic. Each row
//! shows the word and its Arabic source in Bridge gold; opening it shows the
//! root, the meaning (and how it moved) and a "I know it" that adds it to
//! your words. A search finds a word, and the round games are one tap away.

use dioxus::prelude::*;
use kabar_core::games::bridge;
use kabar_protocol::coach::ActivityKind;
use kabar_protocol::content::{BridgeEntry, MeaningShift};

use super::logic::{self, deck_headline, meaning};
use super::{ContentProblem, PlayHeader, PlayNav, Route, content};
use crate::coach::Coach;
use crate::embedded::{Css, css};
use crate::i18n::Msg;
use crate::{Db, Ui};

const CSS: Css = css!("assets/bridge.css");
/// Rows shown at a time; "show more" adds another page.
const PAGE: usize = 24;
/// Rough study time for reading one card, logged with the word.
const SECONDS_PER_WORD: u32 = 15;

/// Whether a Bridge entry answers a search (word, Arabic or meaning).
fn found(e: &BridgeEntry, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    q.is_empty()
        || e.word.to_lowercase().contains(&q)
        || e.arabic.contains(&q)
        || [&e.meaning.arz, &e.meaning.ar, &e.meaning.en].iter().any(|m| m.to_lowercase().contains(&q))
}

fn search_label(lang: kabar_core::locale::UiLanguage) -> &'static str {
    use kabar_core::locale::UiLanguage::*;
    match lang {
        ArEg => "دوّر على كلمة بالإندونيسي أو بالعربي…",
        Ar => "ابحث عن كلمة بالإندونيسية أو بالعربية…",
        En => "Find a word in Indonesian or Arabic…",
    }
}

fn games_label(lang: kabar_core::locale::UiLanguage) -> &'static str {
    use kabar_core::locale::UiLanguage::*;
    match lang {
        ArEg => "جرّب الكلمات دي في لعبة",
        Ar => "جرّب هذه الكلمات في لعبة",
        En => "Try these words in a game",
    }
}

fn no_match(lang: kabar_core::locale::UiLanguage) -> &'static str {
    use kabar_core::locale::UiLanguage::*;
    match lang {
        ArEg => "مالقيناش الكلمة دي في الجسر.",
        Ar => "لا توجد هذه الكلمة في الجسر.",
        En => "That word isn't on the Bridge.",
    }
}

#[component]
pub fn BridgeDeck() -> Element {
    let data = match content::bridge() {
        Ok(d) => d,
        Err(e) => {
            return rsx! {
                PlayHeader { title: Msg::DeckTitle }
                ContentProblem { detail: e.clone() }
            };
        }
    };
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let PlayNav(mut nav) = use_context::<PlayNav>();
    let deck = use_hook(|| bridge::known_deck(data));
    let mut shown = use_signal(|| PAGE);
    let mut query = use_signal(String::new);
    let logged = logic::logged_words(&coach.events.read());
    let marked = deck.iter().filter(|e| logged.contains(&e.word)).count();
    let q = query.read().clone();
    let matching: Vec<&BridgeEntry> = deck.iter().copied().filter(|e| found(e, &q)).collect();
    let visible = (*shown.read()).min(matching.len());

    rsx! {
        style { {CSS.text()} }
        PlayHeader { title: Msg::DeckTitle }
        div { class: "card deck-hero",
            span { class: "pill gold small", "{ui.t(Msg::BridgeFromArabic)}" }
            p { class: "deck-count", lang: "id", bdi { "{deck_headline(deck.len())}" } }
            p { class: "muted", "{ui.t(Msg::DeckHeroNote)}" }
            p { class: "small deck-marked",
                "{ui.t(Msg::DeckMarked)} "
                span { dir: "ltr", strong { "{ui.num(marked as u64)}" } " / {ui.num(deck.len() as u64)}" }
            }
        }
        input {
            class: "deck-search",
            r#type: "search",
            dir: "auto",
            placeholder: "{search_label(ui.lang())}",
            aria_label: "{search_label(ui.lang())}",
            value: "{query}",
            oninput: move |e| {
                query.set(e.value());
                shown.set(PAGE);
            },
        }
        if matching.is_empty() {
            p { class: "card muted", role: "status", "{no_match(ui.lang())}" }
        } else {
            ul { class: "deck-list",
                for entry in matching.iter().take(visible).copied() {
                    li { key: "{entry.word}",
                        DeckRow { entry: entry.clone(), known: logged.contains(&entry.word) }
                    }
                }
            }
        }
        if visible < matching.len() {
            div { class: "row center",
                button { onclick: move |_| shown += PAGE, "{ui.t(Msg::ShowMore)}" }
            }
        }
        button {
            class: "deck-games",
            onclick: move |_| nav.set(Route::Hub),
            "🎲 {games_label(ui.lang())}"
        }
    }
}

/// One Bridge word: the word and its Arabic; open it for the root, the
/// meaning and "I know it".
#[component]
fn DeckRow(entry: BridgeEntry, known: bool) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let word = entry.word.clone();
    let shift = match entry.shift {
        MeaningShift::Narrowed => Some(Msg::ShiftNarrowed),
        MeaningShift::Broadened => Some(Msg::ShiftBroadened),
        _ => None,
    };

    rsx! {
        details { class: if known { "deck-row known" } else { "deck-row" },
            summary {
                span { class: "deck-word",
                    bdi { class: "bridge-word latin", lang: "id", dir: "ltr", "{entry.word}" }
                    span { class: "muted small", "{meaning(ui.lang(), &entry.meaning)}" }
                }
                if known {
                    span { class: "ok small", aria_label: "{ui.t(Msg::DeckKnown)}", "✓" }
                }
                span { class: "ar bridge-ar", lang: "ar", dir: "rtl", "{entry.arabic}" }
            }
            div { class: "deck-detail",
                if let Some(root) = &entry.root {
                    p { class: "muted small",
                        "{ui.t(Msg::Root)} "
                        span { lang: "ar", dir: "rtl", "{root}" }
                    }
                }
                if meaning(ui.lang(), &entry.arabic_meaning) != meaning(ui.lang(), &entry.meaning) {
                    p { class: "small",
                        span { class: "ar", lang: "ar", "{entry.arabic}" }
                        " · {meaning(ui.lang(), &entry.arabic_meaning)}"
                    }
                }
                if let Some(shift) = shift {
                    span { class: "pill gold small", "{ui.t(shift)}" }
                }
                if known {
                    span { class: "ok small", "✓ {ui.t(Msg::DeckKnown)}" }
                } else {
                    button {
                        class: "quiet-solid",
                        onclick: move |_| {
                            let logged = logic::logged_words(&coach.events.peek());
                            let words = logic::unlogged(std::slice::from_ref(&word), &logged);
                            if !words.is_empty() {
                                coach.record(&db, ActivityKind::NewWords { words }, SECONDS_PER_WORD);
                            }
                        },
                        "{ui.t(Msg::DeckMarkKnown)}"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_the_word_its_arabic_or_its_meaning() {
        let data = content::bridge().as_ref().expect("bridge content");
        let kabar = data.entries.iter().find(|e| e.word == "kabar").expect("kabar");
        assert!(found(kabar, ""));
        assert!(found(kabar, "KAB"));
        assert!(found(kabar, &kabar.arabic));
        assert!(found(kabar, &kabar.meaning.en));
        assert!(!found(kabar, "zzzz"));
    }
}
