//! The word popover: a light peek (hover, pen hover, tap) and the full word
//! card (click, pen tap, long-press), both answered from local data.

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_protocol::reader::WordStatus;

use super::doc::{Tag, Word};
use super::glossary::{Shift, SpeechLevel};
use super::{Panel, Peek, ReaderState};
use crate::i18n::Msg;
use crate::{Db, Ui};

fn tag_msg(tag: Tag) -> Msg {
    match tag {
        Tag::Indonesian => Msg::LangIndonesian,
        Tag::Javanese => Msg::LangJavanese,
        Tag::English => Msg::LangEnglish,
        Tag::Proper => Msg::LangProper,
        Tag::Unknown => Msg::LangUnknown,
    }
}

fn status_msg(s: WordStatus) -> Msg {
    match s {
        WordStatus::New => Msg::StatusNew,
        WordStatus::Learning => Msg::StatusLearning,
        WordStatus::Known => Msg::StatusKnown,
        WordStatus::Ignored => Msg::StatusIgnored,
    }
}

fn shift_msg(s: Shift) -> Msg {
    match s {
        Shift::Same => Msg::ShiftSame,
        Shift::Narrowed => Msg::ShiftNarrowed,
        Shift::Broadened => Msg::ShiftBroadened,
        Shift::Shifted => Msg::ShiftShifted,
        Shift::FalseFriend => Msg::FalseFriend,
    }
}

fn level_msg(l: SpeechLevel) -> Msg {
    match l {
        SpeechLevel::Ngoko => Msg::LevelNgoko,
        SpeechLevel::Krama => Msg::LevelKrama,
        SpeechLevel::KramaInggil => Msg::LevelKramaInggil,
        SpeechLevel::KramaAndhap => Msg::LevelKramaAndhap,
    }
}

/// The floating peek next to a word.
#[component]
pub fn PeekView(peek: Peek) -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let voice = *st.voice.read();
    let open = st.open.read();
    let Some(open) = open.as_ref() else { return rsx! {} };
    let Some(t) = open.doc.tokens.get(peek.i) else { return rsx! {} };
    let Some(w) = &t.word else { return rsx! {} };
    let surface = open.doc.text(t.start, t.end).to_string();
    let anchor = if peek.below { "inset-block-start" } else { "inset-block-end" };
    let style = format!("inset-inline-start: {}px; {anchor}: {}px", peek.start, peek.y);

    rsx! {
        div { class: "peek", role: "tooltip", style: "{style}",
            Headline { w: w.clone(), surface }
            match &w.info {
                Some(info) => rsx! {
                    p { class: "gloss", "{info.gloss.get(voice)}" }
                    if let Some(b) = &info.bridge {
                        p { class: if b.shift == Shift::FalseFriend { "bridge-line false" } else { "bridge-line" },
                            span { class: "ar", lang: "ar", "{b.arabic}" }
                            " · {ui.t(shift_msg(b.shift))}"
                        }
                    }
                },
                None => rsx! {
                    p { class: "muted small", "{ui.t(Msg::NoLocalGloss)}" }
                },
            }
        }
    }
}

/// Surface word, normalized form and language tag.
#[component]
fn Headline(w: Word, surface: String) -> Element {
    let ui = use_context::<Ui>();
    let normalized =
        w.analyzed.normalized.as_ref().filter(|n| n.full.to_lowercase() != surface.to_lowercase()).map(|n| {
            match &n.alt {
                Some(alt) => format!("{} / {alt}", n.full),
                None => n.full.clone(),
            }
        });
    rsx! {
        div { class: "headline",
            bdi { class: "latin surface", lang: "id", "{surface}" }
            if let Some(n) = normalized {
                span { class: "muted", " → " }
                bdi { class: "latin", lang: "id", "{n}" }
            }
            span { class: "tag tag-{w.tag:?}", "{ui.t(tag_msg(w.tag))}" }
            if w.slang {
                span { class: "tag", "{ui.t(Msg::LangSlang)}" }
            }
            if w.info.as_ref().is_some_and(|i| !i.verified) {
                span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
            }
        }
    }
}

/// Side panel (wide windows) or bottom sheet (phone).
#[component]
pub fn PanelView() -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let panel = st.panel.read().clone();
    rsx! {
        aside { class: "card panel", aria_live: "polite",
            div { class: "panel-close",
                button {
                    class: "ghost",
                    aria_label: "{ui.t(Msg::Close)}",
                    onclick: move |_| {
                        let mut p = st.panel;
                        p.set(None);
                    },
                    "×"
                }
            }
            match panel {
                Some(Panel::Word(i)) => rsx! {
                    WordCard { i }
                },
                Some(Panel::Phrase { tokens, save_first }) => rsx! {
                    PhraseCard { tokens, save_first }
                },
                None => rsx! {},
            }
        }
    }
}

#[component]
fn WordCard(i: usize) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let st = use_context::<ReaderState>();
    let mut thanks = use_signal(|| false);
    let mut added = use_signal(|| false);
    let voice = *st.voice.read();
    let open = st.open.read().clone();
    let Some(open) = open else { return rsx! {} };
    let Some(t) = open.doc.tokens.get(i).cloned() else { return rsx! {} };
    let Some(w) = t.word.clone() else { return rsx! {} };
    let surface = open.doc.text(t.start, t.end).to_string();
    let sentence = open.doc.sentence_of(i).to_string();
    let key = w.key.clone();
    let status = key.as_deref().map(|k| st.status(k));
    let breakdown = w.breakdown();
    let speak_text = w.analyzed.normalized.as_ref().map_or(surface.clone(), |n| n.full.clone());

    rsx! {
        div { class: "word-card",
            Headline { w: w.clone(), surface: surface.clone() }

            if let Some(b) = breakdown {
                div { class: "field",
                    span { class: "label", "{ui.t(Msg::Breakdown)}" }
                    bdi { class: "latin", lang: "id", "{b}" }
                }
            }

            match &w.info {
                Some(info) => rsx! {
                    div { class: "field",
                        if let Some(pos) = &info.pos {
                            span { class: "label latin", "{pos}" }
                        }
                        p { class: "gloss big", "{info.gloss.get(voice)}" }
                        if voice != UiLanguage::En {
                            p { class: "muted", lang: "en", dir: "ltr", "{info.gloss.en}" }
                        }
                    }
                    if let Some(note) = &info.note {
                        p { class: "note", "{note.get(voice)}" }
                    }
                    if let Some((text, tr)) = &info.example {
                        div { class: "field",
                            span { class: "label", "{ui.t(Msg::Example)}" }
                            p { class: "sentence", dir: "ltr", lang: "id", "{text}" }
                            p { class: "muted", "{tr.get(voice)}" }
                        }
                    }
                    if let Some(l) = &info.levels {
                        div { class: "field levels",
                            span { class: "label", "{ui.t(Msg::SpeechLevels)} · {ui.t(level_msg(l.level))}" }
                            dl {
                                dt { "{ui.t(Msg::LevelNgoko)}" }
                                dd { bdi { class: "latin", lang: "jv", "{l.ngoko}" } }
                                if let Some(k) = &l.krama {
                                    dt { "{ui.t(Msg::LevelKrama)}" }
                                    dd { bdi { class: "latin", lang: "jv", "{k}" } }
                                }
                                if let Some(k) = &l.krama_inggil {
                                    dt { "{ui.t(Msg::LevelKramaInggil)}" }
                                    dd { bdi { class: "latin", lang: "jv", "{k}" } }
                                }
                            }
                        }
                    }
                    if let Some(b) = &info.bridge {
                        div { class: if b.shift == Shift::FalseFriend { "bridge-card false" } else { "bridge-card" },
                            span { class: "label", "{ui.t(Msg::BridgeTitle)}" }
                            div { class: "bridge",
                                span { class: "bridge-word latin", lang: "id", "{info.lemma}" }
                                span { class: "ar", lang: "ar", "{b.arabic}" }
                            }
                            if let Some(root) = &b.root {
                                p { class: "muted", lang: "ar", "{root}" }
                            }
                            p { strong { "{ui.t(shift_msg(b.shift))}" } }
                            if let Some(note) = &b.note {
                                p { "{note.get(voice)}" }
                            }
                        }
                    }
                },
                None => rsx! {
                    p { class: "muted", "{ui.t(Msg::NoLocalGloss)}" }
                },
            }

            div { class: "field",
                span { class: "label", "{ui.t(Msg::InThisSentence)}" }
                p { class: "sentence", dir: "ltr", lang: "id", "{sentence}" }
            }

            div { class: "row",
                button {
                    onclick: move |_| speak(&speak_text),
                    "🔊 {ui.t(Msg::Listen)}"
                }
                if let Some(k) = key.clone() {
                    button {
                        class: "primary",
                        disabled: *added.read(),
                        onclick: {
                            let db = db.clone();
                            let surface = surface.clone();
                            let sentence = sentence.clone();
                            move |_| {
                                let _ = db.0.borrow_mut().save_word(&k, &surface, &sentence, Some(&open.text.id));
                                if st.status(&k) == WordStatus::New {
                                    st.set_status(&db, &k, WordStatus::Learning);
                                }
                                added.set(true);
                            }
                        },
                        if *added.read() { "{ui.t(Msg::AddedToReview)}" } else { "{ui.t(Msg::AddToReview)}" }
                    }
                }
            }

            if let (Some(k), Some(current)) = (key.clone(), status) {
                div { class: "row status-row", role: "group",
                    for s in WordStatus::ALL {
                        button {
                            key: "{s.as_str()}",
                            class: "status-btn st-{s.as_str()}",
                            aria_pressed: if s == current { "true" } else { "false" },
                            onclick: {
                                let db = db.clone();
                                let k = k.clone();
                                move |_| st.set_status(&db, &k, s)
                            },
                            "{ui.t(status_msg(s))}"
                        }
                    }
                }
            }

            if let Some(k) = key {
                div { class: "row feedback",
                    if *thanks.read() {
                        span { class: "muted", "{ui.t(Msg::ThanksFeedback)}" }
                    } else {
                        for (up , label , icon) in [(true, Msg::Helpful, "👍"), (false, Msg::NotHelpful, "👎")] {
                            button {
                                key: "{up}",
                                class: "ghost",
                                aria_label: "{ui.t(label)}",
                                title: "{ui.t(label)}",
                                onclick: {
                                    let db = db.clone();
                                    let k = k.clone();
                                    move |_| {
                                        let _ = db.0.borrow_mut().add_feedback("gloss", &k, up);
                                        thanks.set(true);
                                    }
                                },
                                "{icon}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PhraseCard(tokens: Vec<usize>, save_first: bool) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let st = use_context::<ReaderState>();
    let voice = *st.voice.read();
    let open = st.open.read().clone();
    let Some(open) = open else { return rsx! {} };
    let Some((start, end)) = open.doc.span_of(&tokens) else { return rsx! {} };
    let phrase = open.doc.text(start, end).to_string();
    let existing =
        tokens.first().and_then(|&i| st.highlight_at(i)).filter(|h| h.start == start && h.end == end);
    let words: Vec<(usize, String, Option<String>)> = tokens
        .iter()
        .filter_map(|&i| {
            let t = open.doc.tokens.get(i)?;
            let w = t.word.as_ref()?;
            Some((
                i,
                open.doc.text(t.start, t.end).to_string(),
                w.info.as_ref().map(|x| x.gloss.get(voice).to_string()),
            ))
        })
        .collect();

    let save = {
        let db = db.clone();
        let tokens = tokens.clone();
        move |_| st.save_phrase(&db, &tokens)
    };

    rsx! {
        div { class: "word-card",
            p { class: "phrase", dir: "ltr", lang: "id", bdi { "{phrase}" } }
            div { class: "row",
                match existing {
                    Some(h) => rsx! {
                        button {
                            onclick: {
                                let db = db.clone();
                                move |_| {
                                    let _ = db.0.borrow_mut().remove_highlight(&h.id);
                                    let mut hl = st.highlights;
                                    hl.write().retain(|x| x.id != h.id);
                                }
                            },
                            "{ui.t(Msg::RemoveHighlight)}"
                        }
                    },
                    None => rsx! {
                        button { class: if save_first { "primary" } else { "" }, onclick: save, "{ui.t(Msg::SavePhrase)}" }
                    },
                }
                button { onclick: move |_| speak(&phrase), "🔊 {ui.t(Msg::Listen)}" }
                button {
                    onclick: move |_| {
                        let mut t = st.toast;
                        t.set(Some(Msg::ExplainNeedsCloud));
                    },
                    "{ui.t(Msg::ExplainSentence)}"
                }
            }
            ul { class: "phrase-words",
                for (i , surface , gloss) in words {
                    li { key: "{i}",
                        button {
                            class: "ghost",
                            onclick: move |_| {
                                let mut p = st.panel;
                                p.set(Some(Panel::Word(i)));
                            },
                            bdi { class: "latin", lang: "id", "{surface}" }
                        }
                        span { " {gloss.as_deref().unwrap_or(\"…\")}" }
                    }
                }
            }
        }
    }
}

/// System text-to-speech until the Azure voices arrive through Kabar Cloud.
fn speak(text: &str) {
    let text = serde_json::to_string(text).unwrap_or_default();
    document::eval(&format!(
        r#"if ("speechSynthesis" in window) {{
            const u = new SpeechSynthesisUtterance({text});
            u.lang = "id-ID";
            u.rate = 0.9;
            speechSynthesis.cancel();
            speechSynthesis.speak(u);
        }}"#
    ));
}
