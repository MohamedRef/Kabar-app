//! Pasar! (spec §14): haggle with a market seller under a budget. Numbers,
//! prices, polite refusals, and Javanese market words with some sellers.

use std::collections::HashSet;

use dioxus::prelude::*;
use kabar_core::games::pasar::{self, Game, Item, Move, Outcome, PasarResult, SellerTurn};
use kabar_core::games::rupiah;
use kabar_protocol::coach::{ActivityKind, Skill};

use super::logic::{self, meaning};
use super::{ContentProblem, PlayHeader, content, fresh_seed, seconds_since};
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Db, Ui};

/// One haggle in progress.
#[derive(Clone)]
struct Match {
    game: Game,
    opening: SellerTurn,
    start_ms: u64,
    logged: bool,
}

/// What a move is called in the UI language, under its Indonesian line.
fn move_hint(mv: &Move) -> Msg {
    match mv {
        Move::AskPrice => Msg::PasarAskPrice,
        Move::AskLower => Msg::PasarAskLower,
        Move::Offer(_) => Msg::PasarOffer,
        Move::Accept => Msg::PasarAccept,
        Move::Refuse => Msg::PasarRefuse,
    }
}

/// The seller's language tag: Javanese market speech or Indonesian.
fn seller_lang(item: &Item) -> &'static str {
    if item.javanese { "jv" } else { "id" }
}

#[component]
pub fn Pasar() -> Element {
    let data = match content::pasar() {
        Ok(d) => d,
        Err(e) => {
            return rsx! {
                PlayHeader { title: Msg::PasarTitle, latin: true }
                ContentProblem { detail: e.clone() }
            };
        }
    };
    let ui = use_context::<Ui>();
    let mut current = use_signal(|| None::<Match>);
    if current.read().is_some() {
        return rsx! {
            PlayHeader { title: Msg::PasarTitle, latin: true }
            Haggle { current }
        };
    }

    rsx! {
        PlayHeader { title: Msg::PasarTitle, latin: true }
        crate::ui::DivCard {
            p { class: "muted", "{ui.t(Msg::PasarIntro)}" }
        }
        if data.items.is_empty() {
            crate::ui::Card { element: crate::ui::CardElement::Div, class: "", p { "{ui.t(Msg::PasarNoItems)}" } }
        }
        div { class: "play-grid",
            for item in data.items.iter() {
                button {
                    key: "{item.id}",
                    class: if item.bonus { "game-card item bonus" } else { "game-card item" },
                    onclick: {
                        let item = item.clone();
                        move |_| {
                            let game = Game::new(&item, logic::budget_for(item.fair), fresh_seed());
                            let opening = game.opening();
                            current.set(Some(Match { game, opening, start_ms: crate::store::now_ms(), logged: false }));
                        }
                    },
                    span { class: "game-text",
                        if item.bonus {
                            span { class: "pill bonus-pill small", "{ui.t(Msg::PasarBonus)}" }
                        }
                        strong { lang: seller_lang(item), bdi { class: "latin", "{item.name}" } }
                        span { class: "muted small", "{meaning(ui.lang(), &item.meaning)}" }
                        span { class: "small",
                            "{ui.t(Msg::PasarBudget)} "
                            bdi { class: "latin", "{rupiah::format_rupiah(logic::budget_for(item.fair))}" }
                        }
                        if item.javanese {
                            span { class: "pill unverified small", "{ui.t(Msg::Unverified)} · " bdi { "Jawa" } }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Haggle(current: Signal<Option<Match>>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut offer = use_signal(String::new);
    let revealed = use_signal(HashSet::<usize>::new);
    let Some(m) = current.read().clone() else {
        return rsx! {};
    };
    let item = m.game.item().clone();
    let jv = item.javanese;
    let lang = seller_lang(&item);
    let budget = m.game.budget();
    let result = m.game.result();
    let on_table = m.game.turns().iter().rev().find_map(|(_, t)| t.price).or(m.opening.price);
    let amount = logic::parse_amount(&offer.read());

    let mut play = {
        let db = db.clone();
        move |mv: Move| {
            let finished = {
                let mut w = current.write();
                let Some(m) = w.as_mut() else { return };
                if m.game.result().is_some() {
                    return;
                }
                m.game.play(mv);
                match m.game.result() {
                    Some(r) if !m.logged => {
                        m.logged = true;
                        Some((r, m.start_ms))
                    }
                    _ => None,
                }
            };
            if let Some((r, start)) = finished {
                coach.record(
                    &db,
                    ActivityKind::Practice {
                        skill: Skill::Speaking,
                        topic: logic::TOPIC_PASAR.to_string(),
                        correct: u32::from(r.stars),
                        total: 3,
                    },
                    seconds_since(start),
                );
            }
        }
    };

    rsx! {
        div { class: "pasar-layout",
            crate::ui::DivCard { class: "pasar-head",
                p { class: "item-name", lang, bdi { class: "latin", "{item.name}" } }
                p { class: "muted small", "{meaning(ui.lang(), &item.meaning)}" }
                div { class: "row",
                    span { class: "pill", "{ui.t(Msg::PasarBudget)} " bdi { class: "latin", "{rupiah::format_rupiah(budget)}" } }
                    if let Some(p) = on_table {
                        span { class: "pill price-pill",
                            "{ui.t(Msg::PasarOnTable)} "
                            bdi { class: "latin", "{rupiah::format_rupiah(p)}" }
                        }
                    }
                    if jv {
                        span { class: "pill unverified", "{ui.t(Msg::Unverified)}" }
                    }
                }
            }
            crate::ui::Card { element: crate::ui::CardElement::Div, class: "transcript", role: "log", aria_live: "polite",
                SellerBubble { index: 0, turn: m.opening.clone(), lang, revealed }
                for (i , (mv , turn)) in m.game.turns().iter().enumerate() {
                    div { key: "{i}",
                        div { class: "bubble me", lang,
                            bdi { class: "latin", "{pasar::phrase(mv, jv)}" }
                        }
                        SellerBubble { index: i + 1, turn: turn.clone(), lang, revealed }
                    }
                }
            }
            if let Some(r) = result {
                ResultCard {
                    result: r,
                    onagain: move |_| {
                        offer.set(String::new());
                        current.set(None);
                    },
                }
            } else {
                crate::ui::DivCard { class: "moves",
                    div { class: "move-grid",
                        for mv in [Move::AskPrice, Move::AskLower, Move::Accept, Move::Refuse] {
                            button {
                                key: "{mv:?}",
                                class: "move",
                                onclick: {
                                    let mv = mv.clone();
                                    let mut play = play.clone();
                                    move |_| play(mv.clone())
                                },
                                bdi { class: "latin", lang, "{pasar::phrase(&mv, jv)}" }
                                span { class: "muted small", "{ui.t(move_hint(&mv))}" }
                            }
                        }
                    }
                    form {
                        class: "offer",
                        onsubmit: move |e: FormEvent| {
                            e.prevent_default();
                            if let Some(n) = amount {
                                play(Move::Offer(n));
                                offer.set(String::new());
                            }
                        },
                        label { class: "label", r#for: "pasar-offer", "{ui.t(Msg::PasarOffer)}" }
                        div { class: "row",
                            input {
                                id: "pasar-offer",
                                class: "text-input latin",
                                r#type: "text",
                                inputmode: "numeric",
                                dir: "ltr",
                                autocomplete: "off",
                                placeholder: "25000",
                                value: "{offer}",
                                oninput: move |e: FormEvent| offer.set(e.value()),
                            }
                            button { class: "primary", r#type: "submit", disabled: amount.is_none(),
                                if let Some(n) = amount {
                                    bdi { class: "latin", lang, "{pasar::phrase(&Move::Offer(n), jv)}" }
                                } else {
                                    "{ui.t(Msg::PasarOffer)}"
                                }
                            }
                        }
                        if let Some(n) = amount {
                            p { class: "muted small", lang: "id",
                                bdi { class: "latin", "{rupiah::format_rupiah(n)} · {rupiah::rupiah_words(n)}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SellerBubble(
    index: usize,
    turn: SellerTurn,
    lang: &'static str,
    revealed: Signal<HashSet<usize>>,
) -> Element {
    let ui = use_context::<Ui>();
    let open = revealed.read().contains(&index);
    rsx! {
        div { class: "bubble seller",
            p { lang, bdi { class: "latin", "{turn.text}" } }
            if open {
                p { class: "muted small reveal", "{meaning(ui.lang(), &turn.meaning)}" }
            }
            button {
                class: "quiet small",
                aria_expanded: if open { "true" } else { "false" },
                onclick: move |_| {
                    let mut set = revealed.write();
                    if !set.remove(&index) {
                        set.insert(index);
                    }
                },
                if open { "{ui.t(Msg::HideMeaning)}" } else { "{ui.t(Msg::ShowMeaning)}" }
            }
        }
    }
}

#[component]
fn ResultCard(result: PasarResult, onagain: EventHandler<()>) -> Element {
    let ui = use_context::<Ui>();
    let headline = match result.outcome {
        Outcome::Deal { .. } if result.within_budget => Msg::PasarDealIn,
        Outcome::Deal { .. } => Msg::PasarDealOver,
        Outcome::WalkedAway => Msg::PasarWalked,
        Outcome::Continue => Msg::PasarDealIn,
    };
    let stars = result.stars.min(3);
    rsx! {
        crate::ui::DivCard { class: "summary-card reveal",
            p { class: "stars", aria_label: "{ui.num(stars)} / {ui.num(3u8)}",
                for i in 0..3u8 {
                    span { key: "{i}", class: if i < stars { "star on" } else { "star" }, aria_hidden: "true", "★" }
                }
            }
            p { strong { "{ui.t(headline)}" } }
            if let Outcome::Deal { price } = result.outcome {
                p { "{ui.t(Msg::PasarPaid)} " bdi { class: "latin", "{rupiah::format_rupiah(price)}" } }
            }
            if result.saved > 0 {
                p { class: "ok", "{ui.t(Msg::PasarSaved)} " bdi { class: "latin", "{rupiah::format_rupiah(result.saved)}" } }
            }
            div { class: "row",
                button { class: "primary", onclick: move |_| onagain.call(()), "{ui.t(Msg::PlayAgain)}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::content::Trilingual;

    use super::*;

    #[test]
    fn sellers_speak_their_language() {
        let mut item = Item {
            id: "kaos".into(),
            name: "kaos batik".into(),
            meaning: Trilingual {
                arz: "تيشيرت باتيك".into(),
                ar: "قميص باتيك".into(),
                en: "batik T-shirt".into(),
            },
            ask: 120_000,
            floor: 60_000,
            fair: 75_000,
            javanese: false,
            bonus: false,
            seller_lines: kabar_core::games::pasar::LineBank::new(Vec::new()),
        };
        assert_eq!(seller_lang(&item), "id");
        item.javanese = true;
        assert_eq!(seller_lang(&item), "jv");
        assert_eq!(move_hint(&Move::Offer(5)), Msg::PasarOffer);
    }
}
