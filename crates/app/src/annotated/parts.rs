//! The word-colour pieces every screen shares: a word's parts in their
//! colours, its class chip, the colour legend and the whole word aloud.
//! They take the voice as a prop and read no screen's state. The global
//! "word colours" switch is CSS (`.app[data-word-colours="off"]`), so none
//! of them checks it.

use dioxus::prelude::*;
use kabar_core::grammar::word::{LEGEND, Part, PartKind, WordClass};
use kabar_core::locale::UiLanguage;

use crate::Ui;
use crate::i18n::Msg;
use crate::pron::Pron;
use crate::reader::card::speak;
use crate::reader::explain::voice_of;

fn piece_text(p: &Part) -> String {
    match p.kind {
        PartKind::Prefix => format!("{}-", p.form),
        PartKind::Suffix | PartKind::Clitic => format!("-{}", p.form),
        PartKind::Confix if p.label.starts_with(&format!("{}-", p.form)) => format!("{}-", p.form),
        PartKind::Confix => format!("-{}", p.form),
        PartKind::Root => p.form.clone(),
    }
}

/// The pieces in their colours, with a "+" between them and the kind of
/// each piece under it, so the colour is never the only cue.
#[component]
pub fn PartsLine(parts: Vec<Part>, class: Option<WordClass>, labels: bool, voice: UiLanguage) -> Element {
    let voice = voice_of(voice);
    let class_css = class.map_or("", WordClass::css);
    let n = parts.len();
    rsx! {
        span { class: "parts-line {class_css}", dir: "ltr",
            for (j, p) in parts.into_iter().enumerate() {
                span { key: "{j}", class: "piece {p.kind.css()}",
                    bdi { class: "latin", lang: "id", "{piece_text(&p)}" }
                    if labels {
                        small { class: "piece-kind", "{p.kind.label().get(voice)}" }
                    }
                }
                if j + 1 < n {
                    span { class: "plus", aria_hidden: "true", "+" }
                }
            }
        }
    }
}

/// The whole word aloud (🔊) and in Arabic letters, beside its pieces.
/// `tag` is "id" or "jv"; `letters: false` where a headline shows them already.
#[component]
pub fn SayWord(text: String, tag: String, #[props(default = true)] letters: bool) -> Element {
    let ui = use_context::<Ui>();
    let voice_tag = if tag == "jv" { "jv-ID" } else { "id-ID" };
    let spoken = text.clone();
    rsx! {
        span { class: "say-word",
            button {
                class: "ghost icon",
                title: "{ui.t(Msg::Listen)}",
                aria_label: "{ui.t(Msg::Listen)}",
                onclick: move |e| {
                    e.stop_propagation();
                    speak(&spoken, voice_tag);
                },
                "🔊"
            }
            if letters {
                Pron { text, tag, inline: true }
            }
        }
    }
}

/// A coloured chip naming the word's class.
#[component]
pub fn ClassChip(class: WordClass, voice: UiLanguage) -> Element {
    let voice = voice_of(voice);
    rsx! {
        span { class: "class-chip {class.css()}", "{class.label().get(voice)}" }
    }
}

/// Every colour with its name.
#[component]
pub fn Legend(voice: UiLanguage) -> Element {
    let ui = use_context::<Ui>();
    let voice = voice_of(voice);
    rsx! {
        div { class: "colour-legend", aria_label: "{ui.t(Msg::MorphColours)}",
            for (j, m) in LEGEND.into_iter().enumerate() {
                span { key: "{j}", class: "legend-item {m.css()}",
                    span { class: "dot", aria_hidden: "true" }
                    "{m.label().get(voice)}"
                }
            }
        }
    }
}
