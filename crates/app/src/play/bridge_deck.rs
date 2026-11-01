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

/// Whether the Arabic word's own meaning adds something: not when it's the
/// Indonesian meaning again, or just the Arabic word itself.
fn says_more(e: &BridgeEntry, arabic_meaning: &str, meaning: &str) -> bool {
    let bare = |s: &str| s.chars().filter(|c| !('\u{064B}'..='\u{0652}').contains(c)).collect::<String>();
    let m = bare(arabic_meaning.trim());
    !m.is_empty() && m != bare(meaning.trim()) && m != bare(e.arabic.trim())
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
    let db = use_context::<Db>();
    let PlayNav(mut nav) = use_context::<PlayNav>();
    let deck = use_hook(|| bridge::known_deck(data));
    let mut shown = use_signal(|| PAGE);
    let mut query = use_signal(String::new);
    let logged = logic::logged_words(&coach.events.read());
    let marked = deck.iter().filter(|e| logged.contains(&e.word)).count();
    let q = query.read().clone();
    let mut matching: Vec<&BridgeEntry> = deck.iter().copied().filter(|e| found(e, &q)).collect();
    let pick = DECK_VIEW.pick(&db);
    crate::views::sort_by(&mut matching, pick.sort, |e| !logged.contains(&e.word), |e| e.word.clone());
    let visible = (*shown.read()).min(matching.len());

    rsx! {
        style { {CSS.text()} }
        PlayHeader { title: Msg::DeckTitle }
        crate::ui::DivCard { class: "flat deck-hero",
            span { class: "status-chip gold", "{ui.t(Msg::BridgeFromArabic)}" }
            p { class: "deck-count", lang: "id", bdi { "{deck_headline(deck.len())}" } }
            p { class: "small deck-marked",
                "{ui.t(Msg::DeckMarked)} "
                span { class: "ltr-count", strong { "{ui.num(marked as u64)}" } " / {ui.num(deck.len() as u64)}" }
            }
        }
        label { class: "search-field deck-search",
            {crate::icons::search()}
            input {
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
        }
        if matching.is_empty() {
            crate::ui::Card { element: crate::ui::CardElement::Paragraph, class: "flat muted", role: "status", "{no_match(ui.lang())}" }
        } else {
            crate::views::ViewBar { of: DECK_VIEW }
            ul { class: "coll deck-list", "data-layout": pick.layout.as_str(),
                for entry in matching.iter().take(visible).copied() {
                    li { key: "{entry.word}",
                        DeckRow { entry: entry.clone(), known: logged.contains(&entry.word) }
                    }
                }
            }
        }
        if visible < matching.len() {
            div { class: "row center",
                button { class: "outline", onclick: move |_| shown += PAGE, "{ui.t(Msg::ShowMore)}" }
            }
        }
        button {
            class: "tonal wide deck-games",
            onclick: move |_| nav.set(Route::Hub),
            "🎲 {games_label(ui.lang())}"
        }
    }
}

/// The deck: rows by default; the deck's order, the words not marked
/// known yet first, or A to Z.
const DECK_VIEW: crate::views::Collection = crate::views::Collection {
    key: "bridge.deck",
    layout: crate::views::Layout::DEFAULT,
    sorts: &[crate::views::Sort::Course, crate::views::Sort::Left, crate::views::Sort::Name],
};

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
            summary { class: "coll-btn",
                span { class: "coll-main deck-word",
                    bdi { class: "coll-title bridge-word latin", lang: "id", dir: "ltr", "{entry.word}" }
                    span { class: "coll-sub", "{meaning(ui.lang(), &entry.meaning)}" }
                }
                span { class: "coll-end deck-end",
                    span { class: "ar bridge-ar", lang: "ar", dir: "rtl", "{entry.arabic}" }
                    if known {
                        span { class: "status-chip good", aria_label: "{ui.t(Msg::DeckKnown)}", "✓" }
                    }
                }
            }
            div { class: "deck-detail",
                if let Some(root) = &entry.root {
                    p { class: "muted small",
                        "{ui.t(Msg::Root)} "
                        span { lang: "ar", dir: "rtl", "{root}" }
                    }
                }
                if says_more(&entry, meaning(ui.lang(), &entry.arabic_meaning), meaning(ui.lang(), &entry.meaning)) {
                    p { class: "small",
                        span { class: "ar", lang: "ar", "{entry.arabic}" }
                        " · {meaning(ui.lang(), &entry.arabic_meaning)}"
                    }
                }
                if let Some(shift) = shift {
                    span { class: "status-chip gold", "{ui.t(shift)}" }
                }
                if known {
                    span { class: "status-chip good", "✓ {ui.t(Msg::DeckKnown)}" }
                } else {
                    button {
                        class: "tonal",
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
        assert!(!says_more(kabar, &kabar.arabic, "news"), "the Arabic word again says nothing new");
        assert!(!says_more(kabar, "أخبار", "أخبار"));
        assert!(says_more(kabar, "نبأ", "أخبار"));
    }
}
