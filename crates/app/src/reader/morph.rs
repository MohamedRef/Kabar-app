//! Word colours and the word card's breakdown (تفصيل): what kind of word it
//! is, its pieces in their colours, what each affix does, the root's
//! meaning, the time words around a verb and the grammar entries. All of it
//! comes from `kabar_core::grammar::word`; this file only draws it.
//! `PartsLine`, `ClassChip` and `Legend` take the voice as a prop, so any
//! screen can use them.

use dioxus::prelude::*;
use kabar_core::grammar::Voice;
use kabar_core::grammar::word::{self, LEGEND, Part, PartKind, WordClass};
use kabar_core::lang::Lang;
use kabar_core::locale::UiLanguage;
use kabar_protocol::grammar::GrammarPoint;

use super::ReaderState;
use super::doc::{Tag, Word};
use super::explain::{library, voice_of};
use super::glossary::WordLang;
use crate::Ui;
use crate::i18n::Msg;

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

/// The word's class, for its colour. `None` for names, English and numbers.
pub fn class_of_word(w: &Word) -> Option<WordClass> {
    word::class_in(&w.analyzed, lang_of(w)?, pos_of(w))
}

/// "wc-verb" / "wc-noun" / "wc-adj" / "wc-func".
pub fn class_css(c: WordClass) -> &'static str {
    c.css()
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

/// A coloured chip naming the word's class.
#[component]
pub fn ClassChip(class: WordClass, voice: UiLanguage) -> Element {
    let voice = voice_of(voice);
    rsx! {
        span { class: "class-chip {class.css()}", "{class.label().get(voice)}" }
    }
}

fn title(p: &GrammarPoint, v: Voice) -> &str {
    match v {
        Voice::Arz => &p.title.ar_eg,
        Voice::Ar => &p.title.ar,
        Voice::En => &p.title.en,
    }
}

fn summary(p: &GrammarPoint, v: Voice) -> &str {
    match v {
        Voice::Arz => &p.summary.ar_eg,
        Voice::Ar => &p.summary.ar,
        Voice::En => &p.summary.en,
    }
}

/// The optional breakdown under the word card: closed until asked for.
#[component]
pub fn MorphDetail(i: usize) -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let ui_voice: UiLanguage = *st.voice.read();
    let voice = voice_of(ui_voice);
    let open = st.open.read().clone();
    let Some(open) = open else { return rsx! {} };
    let Some(w) = open.doc.tokens.get(i).and_then(|t| t.word.clone()) else { return rsx! {} };
    let Some(lang) = lang_of(&w) else { return rsx! {} };

    let (start, end) = open.doc.sentence_span(i);
    let sentence: Vec<_> = open
        .doc
        .tokens_in(start, end)
        .filter_map(|j| open.doc.tokens[j].word.as_ref().map(|w| w.analyzed.clone()))
        .collect();
    let lib = library();
    let detail = lib.word_detail(&w.analyzed, &sentence, lang, pos_of(&w));
    let class = class_of_word(&w);
    if detail.is_empty() && class.is_none() {
        return rsx! {};
    }

    // What the root means, from the dictionary, when it isn't the word itself.
    let prefer = Some(if lang == Lang::Javanese { WordLang::Javanese } else { WordLang::Indonesian });
    let root_gloss = (detail.parts.len() > 1)
        .then(|| {
            super::glossary()
                .lookup(prefer, &[detail.root.as_str()])
                .map(|i| i.gloss.get(ui_voice).to_string())
        })
        .flatten();
    let notes: Vec<(Part, word::AffixNote)> = detail
        .parts
        .iter()
        .filter(|p| p.kind != PartKind::Root)
        .filter_map(|p| word::affix_note(lang, &p.label).map(|n| (p.clone(), n)))
        // A confix shows once, not for each half.
        .fold(Vec::new(), |mut acc, (p, n)| {
            if !acc.iter().any(|(q, _): &(Part, _)| q.label == p.label) {
                acc.push((p, n));
            }
            acc
        });
    let redup =
        detail.redup.as_ref().map(|(kind, base)| (word::redup_note(*kind, class).get(voice), base.clone()));
    let entries: Vec<GrammarPoint> = detail.grammar.iter().filter_map(|c| lib.get(&c.id).cloned()).collect();
    let class_css = class.map_or("", WordClass::css);

    rsx! {
        details { class: "morph-detail {class_css}",
            summary {
                span { "{ui.t(Msg::MorphMore)}" }
                if let Some(c) = class {
                    ClassChip { class: c, voice: ui_voice }
                }
            }
            div { class: "morph-body",
                if detail.parts.len() > 1 {
                    PartsLine { parts: detail.parts.clone(), class, labels: true, voice: ui_voice }
                }
                if let Some(change) = &detail.sound_change {
                    p { class: "muted small",
                        bdi { class: "latin", dir: "ltr", "{change}" }
                        " · {ui.t(Msg::MorphSoundChange)}"
                    }
                }
                ul { class: "morph-notes",
                    if detail.parts.len() > 1 {
                        li {
                            bdi { class: "latin piece wp-root", lang: "id", dir: "ltr", "{detail.root}" }
                            if let Some(g) = &root_gloss {
                                span { " · {g}" }
                            }
                            if !detail.root_verified {
                                span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
                            }
                        }
                    }
                    for (j, (p, n)) in notes.into_iter().enumerate() {
                        li { key: "{j}",
                            bdi { class: "latin piece {p.kind.css()}", lang: "id", dir: "ltr", "{p.label}" }
                            span { " · " }
                            Isolated { text: n.meaning.get(voice) }
                            if !n.verified {
                                span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
                            }
                        }
                    }
                    if let Some((note, base)) = redup {
                        li {
                            bdi { class: "latin piece wp-root", lang: "id", dir: "ltr", "{base}-{base}" }
                            span { " · " }
                            Isolated { text: note }
                        }
                    }
                }
                if !detail.aspect.is_empty() {
                    div { class: "field",
                        span { class: "label", "{ui.t(Msg::MorphTimeWords)}" }
                        p {
                            for (j, a) in detail.aspect.iter().enumerate() {
                                bdi { key: "{j}", class: "latin class-chip wc-func", lang: "id", dir: "ltr", "{a.surface}" }
                            }
                        }
                    }
                }
                if !entries.is_empty() {
                    div { class: "field",
                        span { class: "label", "{ui.t(Msg::MorphGrammar)}" }
                        for (j, e) in entries.iter().enumerate() {
                            details { key: "{j}", class: "morph-entry",
                                summary { "{title(e, voice)}" }
                                p { "{summary(e, voice)}" }
                            }
                        }
                    }
                }
                Legend { voice: ui_voice }
            }
        }
    }
}

/// A note whose Latin pieces are marked with U+2068…U+2069: each piece is
/// drawn as its own left-to-right element, since some WebViews ignore the
/// isolate characters and would show "(زي -kan)" as "(زي kan-".
#[component]
fn Isolated(text: &'static str) -> Element {
    let pieces = text.split('\u{2068}').enumerate().flat_map(|(k, chunk)| {
        let (latin, rest) = match chunk.split_once('\u{2069}') {
            Some((l, r)) if k > 0 => (Some(l.replace('\u{2060}', "")), r),
            _ => (None, chunk),
        };
        latin.map(|l| (true, l)).into_iter().chain([(false, rest.to_string())])
    });
    rsx! {
        for (j, (latin, piece)) in pieces.enumerate() {
            if latin {
                bdi { key: "{j}", class: "latin", lang: "id", dir: "ltr", "{piece}" }
            } else {
                span { key: "{j}", "{piece}" }
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
