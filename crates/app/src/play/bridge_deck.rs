//! "Kamu sudah tahu N kata": the quick-win deck of Arabic loans whose meaning
//! an Egyptian already knows (spec §12). Flip a card to see the Arabic word,
//! its root and the meaning; mark it as known to add it to your words.

use std::collections::HashSet;

use dioxus::prelude::*;
use kabar_core::games::bridge;
use kabar_protocol::coach::ActivityKind;
use kabar_protocol::content::{BridgeEntry, MeaningShift};

use super::logic::{self, deck_headline, meaning};
use super::{ContentProblem, PlayHeader, content};
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Db, Ui};

/// Cards shown at a time; "show more" adds another page.
const PAGE: usize = 24;
/// Rough study time for reading one card, logged with the word.
const SECONDS_PER_WORD: u32 = 15;

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
    let deck = use_hook(|| bridge::known_deck(data));
    let mut shown = use_signal(|| PAGE);
    let flipped = use_signal(HashSet::<String>::new);
    let logged = logic::logged_words(&coach.events.read());
    let marked = deck.iter().filter(|e| logged.contains(&e.word)).count();
    let visible = (*shown.read()).min(deck.len());

    rsx! {
        PlayHeader { title: Msg::DeckTitle }
        div { class: "card deck-hero reveal",
            p { class: "deck-count", lang: "id", bdi { "{deck_headline(deck.len())}" } }
            p { class: "muted", "{ui.t(Msg::DeckHeroNote)}" }
            p { class: "small",
                "{ui.t(Msg::DeckMarked)} "
                strong { "{ui.num(marked as u64)}" }
                " / {ui.num(deck.len() as u64)}"
            }
        }
        div { class: "play-grid deck",
            for entry in deck.iter().take(visible).copied() {
                DeckCard {
                    key: "{entry.word}",
                    entry: entry.clone(),
                    known: logged.contains(&entry.word),
                    flipped,
                }
            }
        }
        if visible < deck.len() {
            div { class: "row center",
                button { onclick: move |_| shown += PAGE, "{ui.t(Msg::ShowMore)}" }
            }
        }
    }
}

#[component]
fn DeckCard(entry: BridgeEntry, known: bool, flipped: Signal<HashSet<String>>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let is_flipped = flipped.read().contains(&entry.word);
    let word = entry.word.clone();
    let shift = match entry.shift {
        MeaningShift::Narrowed => Some(Msg::ShiftNarrowed),
        MeaningShift::Broadened => Some(Msg::ShiftBroadened),
        _ => None,
    };

    rsx! {
        div { class: if known { "flip-card known" } else { "flip-card" },
            button {
                class: "flip-face",
                aria_pressed: if is_flipped { "true" } else { "false" },
                aria_label: "{ui.t(Msg::DeckFlip)}",
                onclick: {
                    let word = entry.word.clone();
                    move |_| {
                        let mut set = flipped.write();
                        if !set.remove(&word) {
                            set.insert(word.clone());
                        }
                    }
                },
                if is_flipped {
                    span { class: "flip-back reveal",
                        span { class: "ar bridge-ar", lang: "ar", dir: "rtl", "{entry.arabic}" }
                        if let Some(root) = &entry.root {
                            span { class: "muted small",
                                "{ui.t(Msg::Root)} "
                                span { lang: "ar", dir: "rtl", "{root}" }
                            }
                        }
                        span { class: "meaning", "{meaning(ui.lang(), &entry.meaning)}" }
                        if let Some(shift) = shift {
                            span { class: "pill gold-pill small", "{ui.t(shift)}" }
                        }
                    }
                } else {
                    span { class: "flip-front",
                        span { class: "bridge-word latin big", lang: "id", bdi { "{entry.word}" } }
                        span { class: "muted small", "{ui.t(Msg::DeckTapToFlip)}" }
                    }
                }
            }
            if known {
                span { class: "ok small known-tag", "✓ {ui.t(Msg::DeckKnown)}" }
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
