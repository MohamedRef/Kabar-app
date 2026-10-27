//! Grammar spotted in the open text, offline (core `grammar::detect`): a
//! collapsible list for the whole text, and the rules a word takes part in
//! for its word card. What the learner already learned comes first; points a
//! later level teaches are marked "not taught yet" instead of hidden.

use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::grammar::Voice;
use kabar_core::grammar::detect::{Found, Hit, Standing, Syllabus};
use kabar_core::journey::Progress;
use kabar_core::lang::Lang;
use kabar_core::lang::analyze::AnalyzedToken;

use super::ReaderState;
use super::doc::ReadDoc;
use super::explain::{GrammarEntry, library, voice_of};
use crate::i18n::Msg;
use crate::{Db, Ui};

pub const CSS: &str = include_str!("../../assets/reader-grammar.css");

/// One sentence of the text and the grammar found in it.
#[derive(Clone, Debug, PartialEq)]
pub struct SentenceGrammar {
    /// Byte range in the body, as `ReadDoc::sentence_span` gives it.
    pub span: (usize, usize),
    pub lang: Lang,
    pub found: Vec<Found>,
}

/// Every hit of a text, worked out once per open text.
#[derive(Clone, Debug, PartialEq)]
pub struct DocGrammar {
    pub sentences: Vec<SentenceGrammar>,
    /// The whole text's points, each once with all its hits, in learner order.
    pub text: Vec<Found>,
}

impl DocGrammar {
    pub fn new(doc: &ReadDoc, syllabus: &Syllabus) -> Self {
        let lib = library();
        let mut sentences = Vec::new();
        let mut i = 0;
        while i < doc.tokens.len() {
            let span = doc.sentence_span(i);
            let ids: Vec<usize> = doc.tokens_in(span.0, span.1).filter(|&t| t >= i).collect();
            let next = ids.last().map_or(i + 1, |&l| l + 1).max(i + 1);
            let words: Vec<(usize, &AnalyzedToken)> =
                ids.iter().filter_map(|&t| doc.tokens[t].word.as_ref().map(|w| (t, &w.analyzed))).collect();
            let lang = doc.lang_of(ids.iter().copied());
            if !words.is_empty() {
                let hits = lib.spot(&words, lang);
                if !hits.is_empty() {
                    sentences.push(SentenceGrammar { span, lang, found: lib.found(&hits, syllabus) });
                }
            }
            i = next;
        }
        let hits: Vec<Hit> =
            sentences.iter().flat_map(|s| s.found.iter().flat_map(|f| f.hits.iter().cloned())).collect();
        let text = lib.found(&hits, syllabus);
        Self { sentences, text }
    }

    /// The points token `i` takes part in, with the hit that covers it.
    pub fn at(&self, i: usize) -> Vec<(Found, Hit)> {
        self.sentences
            .iter()
            .flat_map(|s| &s.found)
            .filter_map(|f| f.hits.iter().find(|h| h.tokens.contains(&i)).map(|h| (f.clone(), h.clone())))
            .collect()
    }
}

/// What the learner's routes teach and how far they've got.
pub fn syllabus(db: &Db) -> Syllabus {
    let store = db.0.borrow();
    let completed = store.completed_lessons().unwrap_or_default();
    let start = store.placement().ok().flatten().map(|p| p.start_level);
    let mut s = Syllabus::default();
    for lang in ["id", "jv"] {
        let Some(cur) = crate::journey::curriculum(lang) else { continue };
        let progress = Progress::new(cur, &completed);
        let progress = match &start {
            Some(level) if lang == "id" => progress.start_at(cur, level),
            _ => progress,
        };
        s.add(cur, &progress);
    }
    s
}

fn title(id: &str, v: Voice) -> String {
    library().get(id).map_or_else(|| id.to_string(), |p| v.title(&p.title).to_string())
}

fn standing_msg(s: Standing) -> Option<Msg> {
    match s {
        Standing::Learned => Some(Msg::GrammarLearned),
        Standing::AtLevel => Some(Msg::GrammarAtLevel),
        Standing::Ahead => Some(Msg::GrammarNotYet),
        Standing::Unknown => None,
    }
}

fn standing_class(s: Standing) -> &'static str {
    match s {
        Standing::Learned => "gr-learned",
        Standing::AtLevel => "gr-level",
        Standing::Ahead => "gr-ahead",
        Standing::Unknown => "gr-unknown",
    }
}

/// The words of a hit, with `…` where a frame skips some.
fn hit_label(doc: &ReadDoc, h: &Hit) -> String {
    h.label(|t| doc.tokens.get(t).map_or_else(String::new, |r| doc.text(r.start, r.end).to_string()))
}

/// What a hit means here: the cue's note, or the full form of a chat spelling.
fn hit_note(h: &Hit, v: Voice) -> Option<String> {
    if let Some(full) = &h.expands_to {
        return Some(full.clone());
    }
    library().cue_note(h).map(|n| v.of(n).to_string())
}

/// "Learned in …" / "Comes in …", with a button to open that lesson.
#[component]
fn PlaceLine(found: Found) -> Element {
    let ui = use_context::<Ui>();
    let Some(place) = found.place.clone() else { return rsx! {} };
    let msg = if found.standing == Standing::Learned { Msg::GrammarLearnedIn } else { Msg::GrammarTaughtIn };
    let lesson = place.lesson.clone();
    rsx! {
        p { class: "muted small gr-place",
            "{ui.t(msg)} "
            bdi { class: "latin", dir: "ltr", "{place.title}" }
            span { class: "chip level", "{place.level}" }
            " "
            button {
                class: "ghost small",
                onclick: move |_| ui.open_lesson(lesson.clone()),
                "{ui.t(Msg::GrammarOpenLesson)}"
            }
        }
    }
}

/// Grammar points found in the open text, learned ones first.
#[component]
pub fn TextGrammar() -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let voice = voice_of(*st.voice.read());
    let Some(grammar) = st.grammar.read().clone() else { return rsx! {} };
    let Some(doc) = st.open.read().as_ref().map(|o| o.doc.clone()) else { return rsx! {} };
    let found = &grammar.text;
    if found.is_empty() {
        return rsx! {};
    }
    let ahead = found.iter().filter(|f| f.standing == Standing::Ahead).count();
    rsx! {
        details { class: "card text-grammar",
            summary {
                "{ui.t(Msg::GrammarInText)} "
                span { class: "chip", "{found.len()}" }
                if ahead > 0 {
                    " "
                    span { class: "chip gr-ahead", "{ui.t(Msg::GrammarNotYet)} · {ahead}" }
                }
            }
            p { class: "muted small", "{ui.t(Msg::GrammarInTextHint)}" }
            ul { class: "gr-list",
                for f in found.iter() {
                    li { key: "{f.grammar}", class: "gr-item {standing_class(f.standing)}",
                        div { class: "gr-head",
                            span { class: "gr-title", "{title(&f.grammar, voice)}" }
                            span { class: "chip level", "{f.level}" }
                            if let Some(m) = standing_msg(f.standing) {
                                span { class: "chip gr-standing", "{ui.t(m)}" }
                            }
                        }
                        p { class: "gr-hits",
                            for (n, h) in f.hits.iter().take(6).enumerate() {
                                bdi { key: "{n}", class: "latin chip gr-hit {h.role.css()}", dir: "ltr",
                                    "{hit_label(&doc, h)}"
                                }
                                " "
                            }
                            if f.hits.len() > 6 {
                                span { class: "muted small", "+{f.hits.len() - 6}" }
                            }
                        }
                        PlaceLine { found: f.clone() }
                        if let Some(p) = library().get(&f.grammar) {
                            details { class: "gr-more",
                                summary { "{ui.t(Msg::GrammarReadMore)}" }
                                GrammarEntry { point: p.clone(), matched: Vec::new(), voice }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The rules word `i` takes part in, for its word card.
#[component]
pub fn WordGrammar(i: usize) -> Element {
    let st = use_context::<ReaderState>();
    let voice = voice_of(*st.voice.read());
    let Some(grammar) = st.grammar.read().clone() else { return rsx! {} };
    let Some(doc) = st.open.read().as_ref().map(|o| o.doc.clone()) else { return rsx! {} };
    rsx! {
        GrammarAt { doc, grammar, i, voice }
    }
}

/// The rules token `i` of `doc` takes part in, each once: the words that
/// carry it, its title, where it's taught and the full entry on demand.
/// Shared by the Reader's word card and every annotated text's.
#[component]
pub fn GrammarAt(doc: Rc<ReadDoc>, grammar: Rc<DocGrammar>, i: usize, voice: Voice) -> Element {
    let ui = use_context::<Ui>();
    if ui.settings.read().grammar_hints_off {
        return rsx! {};
    }
    let mut seen = BTreeSet::new();
    let here: Vec<(Found, Hit)> =
        grammar.at(i).into_iter().filter(|(f, _)| seen.insert(f.grammar.clone())).collect();
    if here.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "field word-grammar",
            span { class: "label", "{ui.t(Msg::GrammarForWord)}" }
            for (f, h) in here {
                div { key: "{f.grammar}", class: "gr-item {standing_class(f.standing)}",
                    div { class: "gr-head",
                        bdi { class: "latin chip gr-hit {h.role.css()}", dir: "ltr", "{hit_label(&doc, &h)}" }
                        span { class: "gr-title", "{title(&f.grammar, voice)}" }
                        if let Some(m) = standing_msg(f.standing) {
                            span { class: "chip gr-standing", "{ui.t(m)}" }
                        }
                    }
                    if let Some(note) = hit_note(&h, voice) {
                        p { class: "gr-note", "{note}" }
                    }
                    PlaceLine { found: f.clone() }
                    if let Some(p) = library().get(&f.grammar) {
                        details { class: "gr-more",
                            summary { "{ui.t(Msg::GrammarReadMore)}" }
                            GrammarEntry { point: p.clone(), matched: Vec::new(), voice }
                        }
                    }
                }
            }
        }
    }
}

/// A row of rule chips under one line of the text (segment `seg`), learned
/// ones first; a chip opens "Explain this sentence" for its sentence.
#[component]
pub fn LineGrammar(seg: usize) -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    if ui.settings.read().grammar_hints_off {
        return rsx! {};
    }
    let voice = voice_of(*st.voice.read());
    let Some(grammar) = st.grammar.read().clone() else { return rsx! {} };
    let Some(range) = st.open.read().as_ref().and_then(|o| o.doc.segments.get(seg).map(|s| (s.start, s.end)))
    else {
        return rsx! {};
    };
    let mut seen = BTreeSet::new();
    let chips: Vec<(&Found, usize)> = grammar
        .sentences
        .iter()
        .filter(|s| s.span.0 >= range.0 && s.span.1 <= range.1)
        .flat_map(|s| s.found.iter().map(|f| (f, f.hits[0].tokens[0])))
        .filter(|(f, _)| seen.insert(f.grammar.clone()))
        .collect();
    if chips.is_empty() {
        return rsx! {};
    }
    rsx! {
        p { class: "line-grammar",
            for (f, token) in chips {
                button {
                    key: "{f.grammar}",
                    class: "chip gr-chip {standing_class(f.standing)} {f.hits[0].role.css()}",
                    title: if f.standing == Standing::Ahead { ui.t(Msg::GrammarNotYet) } else { "" },
                    onclick: move |_| {
                        let mut p = st.panel;
                        p.set(Some(super::Panel::Explain(token)));
                    },
                    "{title(&f.grammar, voice)}"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::doc::Lexicon;
    use crate::reader::glossary::SeedGlossary;
    use kabar_core::learn::PersonalDictionary;

    fn doc(text: &str) -> ReadDoc {
        ReadDoc::from_text(
            text,
            Lexicon { glossary: &SeedGlossary::default(), taught: &PersonalDictionary::default() },
        )
    }

    fn at(d: &ReadDoc, w: &str) -> usize {
        d.tokens.iter().position(|t| d.text(t.start, t.end) == w).unwrap()
    }

    #[test]
    fn sentences_and_words_get_their_grammar() {
        let d = doc("Saya sedang membaca buku. Kopi ini lebih enak daripada teh!\nSampeyan sampun nedha?");
        let g = DocGrammar::new(&d, &Syllabus::default());
        assert_eq!(g.sentences.len(), 3);
        assert_eq!(d.text(g.sentences[0].span.0, g.sentences[0].span.1), "Saya sedang membaca buku.");
        let ids = |s: &SentenceGrammar| s.found.iter().map(|f| f.grammar.clone()).collect::<Vec<_>>();
        assert!(ids(&g.sentences[0]).contains(&"id.aspect.markers".to_string()));
        assert!(ids(&g.sentences[1]).contains(&"id.comparison.lebih".to_string()));
        assert_eq!(g.sentences[2].lang, Lang::Javanese);
        assert!(ids(&g.sentences[2]).contains(&"jv.krama.function-words".to_string()));

        // Both ends of a frame belong to it.
        for w in ["lebih", "daripada"] {
            let here = g.at(at(&d, w));
            assert!(here.iter().any(|(f, _)| f.grammar == "id.comparison.lebih"), "{w}: {here:?}");
        }
        let (_, h) =
            g.at(at(&d, "sedang")).into_iter().find(|(f, _)| f.grammar == "id.aspect.markers").unwrap();
        assert!(hit_note(&h, Voice::Arz).is_some());
        assert_eq!(hit_label(&d, &g.at(at(&d, "lebih"))[0].1), "lebih … daripada");

        // The whole text lists each point once.
        let all = &g.text;
        let mut ids: Vec<_> = all.iter().map(|f| &f.grammar).collect();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
