//! "Explain this sentence" (spec §11, §15): the grammar-library entries that
//! fit the sentence, picked on the device, plus the `explain_sentence` glue
//! (paraphrase, what the entries don't cover, register, culture) when an
//! engine is connected. Reader texts are often someone's chat, so the request
//! is personal and never reaches an engine that trains on inputs.

use std::sync::OnceLock;

use dioxus::prelude::*;
use kabar_core::ai::tasks::{ExplainSentence, Register};
use kabar_core::ai::{AiRequest, FriendlyReason};
use kabar_core::grammar::{self, Candidate, Library, MAX_CANDIDATES, Voice};
use kabar_core::lang::Lang;
use kabar_core::lang::analyze::AnalyzedToken;
use kabar_core::locale::UiLanguage;
use kabar_protocol::ai::Task;
use kabar_protocol::grammar::{GrammarExample, GrammarPoint, Three, Title};

use super::ReaderState;
use super::doc::{ReadDoc, Tag};
use crate::Ui;
use crate::i18n::Msg;
use crate::teach::Ai;

use crate::embedded::{self, GRAMMAR};

/// The embedded grammar library, parsed once.
pub fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let files: Vec<_> = embedded::all(GRAMMAR).collect();
        let (library, problems) = Library::from_yaml(files.iter().map(|(id, yaml)| (*id, yaml.as_str())));
        for p in problems {
            eprintln!("kabar: grammar entry skipped: {p}");
        }
        library
    })
}

/// One sentence of the open text, ready to explain.
#[derive(Clone, Debug, PartialEq)]
pub struct Sentence {
    pub text: String,
    pub lang: Lang,
    pub candidates: Vec<Candidate>,
}

impl Sentence {
    /// The sentence around `token`. Javanese when most of its words are.
    pub fn around(doc: &ReadDoc, token: usize) -> Option<Self> {
        doc.tokens.get(token)?;
        let (start, end) = doc.sentence_span(token);
        let words: Vec<_> = doc.tokens_in(start, end).filter_map(|i| doc.tokens[i].word.as_ref()).collect();
        let count = |tag: Tag| words.iter().filter(|w| w.tag == tag).count();
        let lang =
            if count(Tag::Javanese) > count(Tag::Indonesian) { Lang::Javanese } else { Lang::Indonesian };
        let analyzed: Vec<AnalyzedToken> = words.iter().map(|w| w.analyzed.clone()).collect();
        let candidates = library().candidates(&analyzed, lang, MAX_CANDIDATES);
        Some(Self { text: doc.text(start, end).to_string(), lang, candidates })
    }

    #[cfg(test)]
    fn ids(&self) -> Vec<String> {
        self.candidates.iter().map(|c| c.id.clone()).collect()
    }
}

pub fn voice_of(l: UiLanguage) -> Voice {
    match l {
        UiLanguage::ArEg => Voice::Arz,
        UiLanguage::Ar => Voice::Ar,
        UiLanguage::En => Voice::En,
    }
}

fn three(t: &Three, v: Voice) -> &str {
    match v {
        Voice::Arz => &t.ar_eg,
        Voice::Ar => &t.ar,
        Voice::En => &t.en,
    }
}

fn title(t: &Title, v: Voice) -> &str {
    match v {
        Voice::Arz => &t.ar_eg,
        Voice::Ar => &t.ar,
        Voice::En => &t.en,
    }
}

fn example(e: &GrammarExample, v: Voice) -> &str {
    match v {
        Voice::Arz => &e.ar_eg,
        Voice::Ar => &e.ar,
        Voice::En => &e.en,
    }
}

/// The AI half, once it answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Glue {
    pub answer: ExplainSentence,
    /// Javanese, or a route that hasn't passed the bake-off: shown unverified.
    pub unverified: bool,
}

/// The router already checked the answer against the request (ids from the
/// candidates, the voice's script); Javanese and unproven routes stay unverified.
fn accept(answer: ExplainSentence, s: &Sentence, needs_review: bool) -> Glue {
    let unverified = needs_review || s.lang == Lang::Javanese;
    Glue { answer, unverified }
}

fn friendly(r: FriendlyReason) -> Msg {
    match r {
        FriendlyReason::ResetsLater => Msg::ExplainResetsLater,
        FriendlyReason::Unreachable => Msg::ExplainUnreachable,
        FriendlyReason::NotAvailable => Msg::ExplainNeedsCloud,
        FriendlyReason::TryAgain => Msg::ExplainTryAgain,
    }
}

fn register_msg(r: Register) -> Msg {
    match r {
        Register::Chat => Msg::RegisterChat,
        Register::Formal => Msg::RegisterFormal,
        Register::Neutral | Register::Standard => Msg::RegisterNeutral,
        Register::Slang | Register::Casual => Msg::RegisterSlang,
        Register::Krama | Register::KramaInggil | Register::Madya => Msg::RegisterKrama,
        Register::Ngoko => Msg::RegisterNgoko,
        Register::Mixed => Msg::RegisterMixed,
    }
}

/// Shown in the side panel / bottom sheet, keyed by text and token.
#[component]
pub fn ExplainCard(token: usize) -> Element {
    let ui = use_context::<Ui>();
    let st = use_context::<ReaderState>();
    let ai = try_consume_context::<Ai>();
    let voice = voice_of(*st.voice.read());
    let open = st.open.read().clone();
    let sentence = open.as_ref().and_then(|o| Sentence::around(&o.doc, token));

    let glue = use_resource(move || {
        let ai = ai.clone();
        let voice = voice_of(*st.voice.read());
        let s = st.open.read().as_ref().and_then(|o| Sentence::around(&o.doc, token));
        async move {
            let s = s.ok_or(Msg::ExplainTryAgain)?;
            let ai = ai.ok_or(Msg::ExplainNeedsCloud)?;
            ai.0.plan(Task::ExplainSentence, true).map_err(|_| Msg::ExplainNeedsCloud)?;
            let req = AiRequest {
                input: grammar::explain_input(&s.text, s.lang, voice, &s.candidates),
                personal: true,
                image_sha256: None,
            };
            let a = ai.0.run::<ExplainSentence>(&req).await.map_err(|e| friendly(e.friendly()))?;
            Ok(accept(a.value, &s, a.needs_review || a.provisional))
        }
    });

    let Some(s) = sentence else { return rsx! {} };
    let glue = glue.read().clone();
    // The model's picks first, then the rest of the candidates.
    let picked: Vec<String> = match &glue {
        Some(Ok(g)) => g.answer.grammar_ids.clone(),
        _ => Vec::new(),
    };
    let mut ordered: Vec<&Candidate> = picked
        .iter()
        .filter_map(|id| s.candidates.iter().find(|c| &c.id == id))
        .chain(s.candidates.iter().filter(|c| !picked.contains(&c.id)))
        .collect();
    if !picked.is_empty() {
        // When the model answered, entries it didn't pick are only a guess: show fewer.
        ordered.truncate(picked.len().max(3));
    }
    let lang_code = if s.lang == Lang::Javanese { "jv" } else { "id" };

    rsx! {
        div { class: "word-card explain",
            p { class: "phrase", dir: "ltr", lang: lang_code, bdi { "{s.text}" } }
            match glue {
                None => rsx! {
                    p { class: "muted", aria_busy: "true", "{ui.t(Msg::ExplainThinking)}" }
                },
                Some(Ok(g)) => rsx! {
                    div { class: "explain-glue",
                        if g.unverified {
                            span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
                        }
                        div { class: "field",
                            span { class: "label", "{ui.t(Msg::InOtherWords)}" }
                            p { class: "big", "{g.answer.paraphrase}" }
                        }
                        p { "{g.answer.explanation}" }
                        p {
                            span { class: "label", "{ui.t(Msg::RegisterLabel)} " }
                            span { class: "chip", "{ui.t(register_msg(g.answer.register))}" }
                        }
                        if let Some(c) = g.answer.culture.as_deref().filter(|c| !c.trim().is_empty()) {
                            div { class: "field note",
                                span { class: "label", "{ui.t(Msg::CultureNote)}" }
                                p { "{c}" }
                            }
                        }
                    }
                },
                Some(Err(m)) => rsx! {
                    p { class: "muted small", "{ui.t(m)}" }
                },
            }
            h3 { class: "label", "{ui.t(Msg::GrammarInSentence)}" }
            if ordered.is_empty() {
                p { class: "muted", "{ui.t(Msg::NoGrammarMatched)}" }
            }
            for c in ordered {
                if let Some(p) = library().get(&c.id) {
                    GrammarEntry { key: "{c.id}", point: p.clone(), matched: c.matched.clone(), voice }
                }
            }
        }
    }
}

#[component]
fn GrammarEntry(point: GrammarPoint, matched: Vec<String>, voice: Voice) -> Element {
    let ui = use_context::<Ui>();
    let p = point;
    let lang = if p.language == "jv" { "jv" } else { "id" };
    let unverified = !p.review.verified || p.language == "jv";
    let ex = p.examples.first().cloned();
    let mistake = p.mistakes.first().cloned();
    rsx! {
        details { class: "grammar-entry",
            summary {
                bdi { class: "latin", dir: "ltr", lang, "{p.title.target}" }
                span { " · {title(&p.title, voice)}" }
                span { class: "chip level", "{p.level}" }
                if unverified {
                    span { class: "tag unverified", "{ui.t(Msg::Unverified)}" }
                }
            }
            p { "{three(&p.summary, voice)}" }
            if !matched.is_empty() {
                p { class: "muted small",
                    "{ui.t(Msg::FoundBy)} "
                    for m in matched {
                        bdi { class: "latin chip", dir: "ltr", lang, "{m}" }
                        " "
                    }
                }
            }
            if let Some(pat) = p.pattern.first() {
                p { class: "pattern", dir: "ltr", bdi { class: "latin", "{pat}" } }
            }
            p { "{three(&p.explanation, voice)}" }
            if let Some(e) = ex {
                div { class: "field",
                    span { class: "label", "{ui.t(Msg::Example)}" }
                    p { dir: "ltr", lang, bdi { class: "latin", "{e.text}" } }
                    p { class: "muted", "{example(&e, voice)}" }
                }
            }
            if let Some(m) = mistake {
                div { class: "field mistake",
                    span { class: "label", "{ui.t(Msg::CommonMistake)}" }
                    p { dir: "ltr",
                        s { bdi { class: "latin", "{m.wrong}" } }
                        " → "
                        bdi { class: "latin", "{m.right}" }
                    }
                    p { class: "muted", "{three(&m.why, voice)}" }
                }
            }
            if let Some(b) = &p.arabic_bridge {
                div { class: "field bridge",
                    span { class: "label", "{ui.t(Msg::RevFromArabic)}" }
                    p { "{three(b, voice)}" }
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
    fn every_embedded_grammar_file_loads() {
        let files: Vec<_> = embedded::all(GRAMMAR).collect();
        let (lib, problems) = Library::from_yaml(files.iter().map(|(id, yaml)| (*id, yaml.as_str())));
        assert!(problems.is_empty(), "{problems:#?}");
        assert_eq!(lib.len(), GRAMMAR.len());
        assert!(GRAMMAR.len() > 100);
    }

    #[test]
    fn the_sentence_around_a_word_finds_its_grammar() {
        let d = doc("Halo! Saya sedang membaca buku. Kesehatan itu penting.");
        let s = Sentence::around(&d, at(&d, "membaca")).unwrap();
        assert_eq!(s.text, "Saya sedang membaca buku.");
        assert_eq!(s.lang, Lang::Indonesian);
        let ids = s.ids();
        assert!(ids.iter().any(|i| i == "id.aspect.markers"), "{ids:?}");
        assert!(ids.len() <= MAX_CANDIDATES);
        assert!(Sentence::around(&d, 999).is_none());
    }

    #[test]
    fn javanese_chat_is_explained_as_javanese() {
        let d = doc("Sek tak lapor pusat");
        let s = Sentence::around(&d, at(&d, "lapor")).unwrap();
        assert_eq!(s.lang, Lang::Javanese);
    }

    #[test]
    fn javanese_answers_stay_unverified() {
        let fixture =
            include_str!("../../../../fixtures/ai/explain_sentence.saya_sedang_membaca_buku.workers_ai.json");
        let answer: ExplainSentence = serde_json::from_str(fixture).unwrap();
        let d = doc("Saya sedang membaca buku.");
        let s = Sentence::around(&d, at(&d, "buku")).unwrap();
        assert!(!accept(answer.clone(), &s, false).unverified);
        assert!(accept(answer.clone(), &s, true).unverified);
        let jv = doc("Sek tak lapor pusat");
        let s = Sentence::around(&jv, at(&jv, "lapor")).unwrap();
        assert!(accept(answer, &s, false).unverified);
    }
}
