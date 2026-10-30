//! Review completion summary.

use super::*;

// ---------------------------------------------------------------- summary

#[component]
pub(super) fn Done() -> Element {
    let ui = use_context::<Ui>();
    let engine = use_context::<Engine>();
    let summary = engine.summary.read().clone().unwrap_or_default();
    let now = now_ms();
    let deck = engine.deck.read();
    let word = |id: &String| deck.card(id).map(|c| (c.word.clone(), c.language.tag()));
    let strongest = summary.strongest.as_ref().and_then(word);
    let revisit: Vec<(String, &'static str)> = summary.revisit.iter().filter_map(word).collect();
    let next = deck.next_due_ms(now).map(|d| when_text(&ui, &engine.sched, now, d));
    drop(deck);

    if summary.answers == 0 {
        return rsx! {
            crate::ui::DivCard { class: "rev-done",
                p { "{ui.t(Msg::RevNothingHere)}" }
                if let Some(when) = &next {
                    p { class: "muted", {ui.t(Msg::RevNextReview).replace("{when}", when)} }
                }
                button { class: "primary", onclick: move |_| engine.home(), "{ui.t(Msg::RevBackHome)}" }
            }
        };
    }

    rsx! {
        crate::ui::DivCard { class: "rev-done",
            h2 { "{ui.t(Msg::RevDoneTitle)}" }
            div { class: "rev-stats",
                Stat { label: Msg::RevCards, value: ui.num(summary.answers as u64), accent: false }
                Stat {
                    label: Msg::RevRemembered,
                    value: ui.num((summary.answers - summary.again) as u64),
                    accent: true,
                }
                Stat {
                    label: Msg::RevRetention,
                    value: summary.retention.map(|r| ui.percent(r)).unwrap_or_else(|| "–".into()),
                    accent: false,
                }
                Stat { label: Msg::RevTime, value: mm_ss(&ui, summary.duration_ms), accent: false }
            }
            if summary.again == 0 {
                p { class: "rev-cheer", "{ui.t(Msg::RevAllRemembered)}" }
            }
            if let Some((w, lang)) = strongest {
                p {
                    "{ui.t(Msg::RevStrongest)}: "
                    bdi { class: "chip", lang, dir: "ltr", "{w}" }
                }
            }
            if !revisit.is_empty() {
                p { "{ui.t(Msg::RevRevisit)}:" }
                div { class: "row",
                    for (w, lang) in revisit {
                        bdi { key: "{w}", class: "chip soft", lang, dir: "ltr", "{w}" }
                    }
                }
            }
            if let Some(when) = &next {
                p { class: "muted", {ui.t(Msg::RevNextReview).replace("{when}", when)} }
            }
            button { class: "primary", onclick: move |_| engine.home(), "{ui.t(Msg::RevBackHome)}" }
        }
    }
}
