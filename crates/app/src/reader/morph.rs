//! Word colours and the word card's breakdown (تفصيل): what kind of word it
//! is, its pieces in their colours, what each affix does, the root's
//! meaning, the time words around a verb and the grammar entries. All of it
//! comes from `kabar_core::grammar::word`; this file only draws it.
//! `PartsLine`, `ClassChip` and `Legend` take the voice as a prop, so any
//! screen can use them.

use dioxus::prelude::*;
use kabar_core::grammar::word::{self, LEGEND, Part, PartKind, WordClass, WordDetail};
use kabar_core::lang::Lang;
use kabar_core::locale::UiLanguage;

use super::ReaderState;
use super::doc::{Tag, Word};
use super::explain::{library, voice_of};
use super::glossary::WordLang;
use crate::Ui;
use crate::i18n::Msg;
use crate::lesson::Rich;
use crate::pron::Pron;

/// The language Kabar treats the word as: its tag, else its dictionary entry.
fn lang_of(w: &Word) -> Option<Lang> {
    match (w.tag, w.info.as_ref().map(|i| i.lang)) {
        (Tag::Javanese, _) | (Tag::Unknown, Some(WordLang::Javanese)) => Some(Lang::Javanese),
        (Tag::Indonesian, _) | (Tag::Unknown, Some(WordLang::Indonesian)) => Some(Lang::Indonesian),
        _ => None,
    }
}

/// The dictionary's part of speech for the word (or the root it was found by).
fn pos_of(w: &Word) -> Option<&str> {
    w.info.as_ref().and_then(|i| i.pos.as_deref())
}

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
                    super::speak(&spoken, voice_tag);
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

/// Everything the open breakdown shows, worked out once per word and voice.
#[derive(Clone, PartialEq)]
struct Shown {
    detail: WordDetail,
    root_gloss: Option<String>,
    /// Each affix once (a confix's two halves are one), with what it does
    /// and whether the note is reviewed.
    notes: Vec<(Part, &'static str, bool)>,
    redup: Option<(&'static str, String)>,
    /// Grammar entries: title and summary in the voice.
    entries: Vec<(String, String)>,
    /// The whole word as it is said (the full form of a chat spelling).
    say: String,
    tag: &'static str,
}

fn shown(st: &ReaderState, i: usize, ui_voice: UiLanguage) -> Option<Shown> {
    let voice = voice_of(ui_voice);
    let open = st.open.read();
    let doc = &open.as_ref()?.doc;
    let w = doc.tokens.get(i)?.word.as_ref()?;
    let lang = lang_of(w)?;
    let (start, end) = doc.sentence_span(i);
    let sentence: Vec<_> = doc
        .tokens_in(start, end)
        .filter_map(|j| doc.tokens[j].word.as_ref().map(|w| w.analyzed.clone()))
        .collect();
    let lib = library();
    let detail = lib.word_detail(&w.analyzed, &sentence, lang, pos_of(w));

    // What the root means, from the dictionary, when it isn't the word itself.
    let prefer = Some(if lang == Lang::Javanese { WordLang::Javanese } else { WordLang::Indonesian });
    let root_gloss = (detail.parts.len() > 1)
        .then(|| {
            super::glossary()
                .lookup(prefer, &[detail.root.as_str()])
                .map(|i| i.gloss.get(ui_voice).to_string())
        })
        .flatten();
    let mut notes: Vec<(Part, &'static str, bool)> = Vec::new();
    for p in detail.parts.iter().filter(|p| p.kind != PartKind::Root) {
        if let Some(n) = word::affix_note(lang, &p.label)
            && !notes.iter().any(|(q, _, _)| q.label == p.label)
        {
            notes.push((p.clone(), n.meaning.get(voice), n.verified));
        }
    }
    let redup = detail
        .redup
        .as_ref()
        .map(|(kind, base)| (word::redup_note(*kind, w.class).get(voice), format!("{base}-{base}")));
    let entries = detail
        .grammar
        .iter()
        .filter_map(|c| lib.get(&c.id))
        .map(|e| (voice.title(&e.title).to_string(), voice.of(&e.summary).to_string()))
        .collect();
    let say = w
        .analyzed
        .normalized
        .as_ref()
        .map_or_else(|| doc.text(doc.tokens[i].start, doc.tokens[i].end).to_string(), |n| n.full.clone());
    let tag = if lang == Lang::Javanese { "jv" } else { "id" };
    Some(Shown { detail, root_gloss, notes, redup, entries, say, tag })
}

/// Core notes mark Latin with U+2068…U+2069 (and U+2060 after a hyphen);
/// [`Rich`] isolates Latin runs itself, and some WebViews ignore the marks.
fn plain(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\u{2060}' | '\u{2068}' | '\u{2069}')).collect()
}

/// The optional breakdown under the word card: closed until asked for, and
/// worked out only once opened.
#[component]
pub fn MorphDetail(i: usize) -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let mut expanded = use_signal(|| false);
    // Recomputed when the word, the voice or the open text changes; the
    // breakdown stays open as the learner taps from word to word.
    let body = use_memo(use_reactive(
        (&i,),
        move |(i,)| {
            if expanded() { shown(&st, i, *st.voice.read()) } else { None }
        },
    ));
    let ui_voice: UiLanguage = *st.voice.read();

    // Whether there is anything to break down, from what the word already knows.
    let class = {
        let open = st.open.read();
        let Some(w) = open.as_ref().and_then(|o| o.doc.tokens.get(i)).and_then(|t| t.word.as_ref()) else {
            return rsx! {};
        };
        let built = w.analyzed.morph.is_some() || w.analyzed.token.redup.is_some();
        if lang_of(w).is_none() || (w.class.is_none() && !built) {
            return rsx! {};
        }
        w.class
    };
    let class_css = class.map_or("", WordClass::css);
    let body = body.read().clone();

    rsx! {
        details { class: "morph-detail {class_css}", open: expanded(),
            summary {
                onclick: move |e| {
                    e.prevent_default();
                    expanded.toggle();
                },
                span { "{ui.t(Msg::MorphMore)}" }
                if let Some(c) = class {
                    ClassChip { class: c, voice: ui_voice }
                }
            }
            if let Some(s) = body {
                div { class: "morph-body",
                    div { class: "parts-row",
                        if s.detail.parts.len() > 1 {
                            PartsLine { parts: s.detail.parts.clone(), class, labels: true, voice: ui_voice }
                        }
                        SayWord { text: s.say.clone(), tag: s.tag.to_string() }
                    }
                    if let Some(change) = &s.detail.sound_change {
                        p { class: "muted small",
                            bdi { class: "latin", dir: "ltr", "{change}" }
                            " · {ui.t(Msg::MorphSoundChange)}"
                        }
                    }
                    ul { class: "morph-notes",
                        if s.detail.parts.len() > 1 {
                            li {
                                bdi { class: "latin piece wp-root", lang: "id", dir: "ltr", "{s.detail.root}" }
                                if let Some(g) = &s.root_gloss {
                                    span { " · {g}" }
                                }
                                if !s.detail.root_verified {
                                    span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
                                }
                            }
                        }
                        for (j, (p, meaning, verified)) in s.notes.iter().enumerate() {
                            li { key: "{j}",
                                bdi { class: "latin piece {p.kind.css()}", lang: "id", dir: "ltr", "{p.label}" }
                                span { " · " }
                                Rich { text: plain(meaning), target: "id" }
                                if !verified {
                                    span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
                                }
                            }
                        }
                        if let Some((note, doubled)) = &s.redup {
                            li {
                                bdi { class: "latin piece wp-root", lang: "id", dir: "ltr", "{doubled}" }
                                span { " · " }
                                Rich { text: plain(note), target: "id" }
                            }
                        }
                    }
                    if !s.detail.aspect.is_empty() {
                        div { class: "field",
                            span { class: "label", "{ui.t(Msg::MorphTimeWords)}" }
                            p {
                                for (j, a) in s.detail.aspect.iter().enumerate() {
                                    bdi { key: "{j}", class: "latin class-chip wc-func", lang: "id", dir: "ltr", "{a.surface}" }
                                }
                            }
                        }
                    }
                    if !s.entries.is_empty() {
                        div { class: "field",
                            span { class: "label", "{ui.t(Msg::MorphGrammar)}" }
                            for (j, (title, summary)) in s.entries.iter().enumerate() {
                                details { key: "{j}", class: "morph-entry",
                                    summary { Rich { text: title.clone(), target: "id" } }
                                    p { Rich { text: summary.clone(), target: "id" } }
                                }
                            }
                        }
                    }
                    Legend { voice: ui_voice }
                }
            }
        }
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
