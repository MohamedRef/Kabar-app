//! Annotated text: how every Indonesian or Javanese sentence outside the
//! Reader is shown. The words are tappable (meaning, breakdown and Bridge
//! from local data, as in the Reader), the pronunciation and the translation
//! sit under the sentence, and all of it follows one set of settings.
//!
//! Prompts and the production side of an exercise use `plain`: the Latin
//! text alone, so nothing gives the answer away (same rule as `crate::pron`).

mod text;

use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::lang::analyze::Morph;
use kabar_core::learn::PersonalDictionary;
use kabar_core::settings::TranslationMode;
use kabar_protocol::content::Lang;

use self::text::{T, tt};
use crate::lesson::Rich;
use crate::pron::Pron;
use crate::reader::card::InfoView;
use crate::reader::doc::{Lexicon, ReadDoc, Tag, Word};
use crate::teach::{TeachInbox, TeachSeed};
use crate::{Db, Screen, Ui};

pub const CSS: &str = include_str!("../../assets/annotated.css");

/// What every annotated sentence shares: whether the glossary is parsed yet
/// (it is ~100 ms of work, done off the UI thread at start-up) and the words
/// the learner taught Kabar (read once, refreshed when they change).
#[derive(Clone, Copy)]
pub struct Words {
    ready: Signal<bool>,
    taught: Signal<Rc<PersonalDictionary>>,
}

impl Words {
    /// Re-reads the taught words after Teach saves or sync brings new ones.
    pub fn refresh(mut self, db: &Db) {
        self.taught.set(Rc::new(taught(db)));
    }
}

fn taught(db: &Db) -> PersonalDictionary {
    PersonalDictionary::new(db.0.borrow().taught_entries().unwrap_or_default())
}

/// Provides [`Words`] to the app and parses the glossary in the background;
/// sentences show plain text until it is ready.
pub fn use_words(db: &Db) -> Words {
    let words = use_context_provider(|| Words {
        ready: Signal::new(false),
        taught: Signal::new(Rc::new(taught(db))),
    });
    use_hook(move || {
        let (tx, rx) = futures_channel::oneshot::channel::<()>();
        std::thread::spawn(move || {
            crate::reader::glossary();
            let _ = tx.send(());
        });
        spawn(async move {
            let _ = rx.await;
            let mut ready = words.ready;
            ready.set(true);
        });
    });
    words
}

/// One run of the sentence: text between words, or a word (token index).
#[derive(Clone, Debug, PartialEq)]
struct Piece {
    text: String,
    word: Option<usize>,
    bridge: bool,
    marked: bool,
}

/// The word is one of `marked` (a story's target words) or a form of one.
fn is_marked(w: &Word, surface: &str, marked: &[String]) -> bool {
    let root = match &w.analyzed.morph {
        Some(Morph::Indonesian(a)) => Some(a.root.as_str()),
        Some(Morph::Javanese(a)) => Some(a.root.as_str()),
        None => None,
    };
    let base = w.analyzed.token.redup.as_ref().map(|(_, b)| b.as_str());
    let surface = surface.to_lowercase();
    marked.iter().any(|m| {
        let m = m.to_lowercase();
        crate::play::logic::is_form_of(&surface, &m) || root == Some(m.as_str()) || base == Some(m.as_str())
    })
}

fn pieces(doc: &ReadDoc, marked: &[String]) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut at = 0;
    for (i, t) in doc.tokens.iter().enumerate() {
        if t.start > at {
            out.push(Piece {
                text: doc.text(at, t.start).to_string(),
                word: None,
                bridge: false,
                marked: false,
            });
        }
        let word = t.word.as_ref().filter(|w| w.key.is_some() || w.has_gloss());
        let text = doc.text(t.start, t.end).to_string();
        out.push(Piece {
            word: word.map(|_| i),
            bridge: word.is_some_and(|w| w.is_bridge()),
            marked: !marked.is_empty() && t.word.as_ref().is_some_and(|w| is_marked(w, &text, marked)),
            text,
        });
        at = t.end;
    }
    if at < doc.body.len() {
        out.push(Piece {
            text: doc.text(at, doc.body.len()).to_string(),
            word: None,
            bridge: false,
            marked: false,
        });
    }
    out
}

/// Analyzes `text` with the Reader's engine and glossary, words the learner
/// taught Kabar on top, and splits it into runs. Once per text: callers
/// memoize it.
fn analyze(text: &str, taught: &PersonalDictionary, marked: &[String]) -> (Rc<ReadDoc>, Rc<Vec<Piece>>) {
    let doc = ReadDoc::from_text(text, Lexicon { glossary: crate::reader::glossary(), taught });
    let parts = pieces(&doc, marked);
    (Rc::new(doc), Rc::new(parts))
}

/// A target-language sentence (or word) with its layers.
///
/// - `tag`: "id" or "jv".
/// - `meaning`: the translation in the UI language, shown under the text
///   following the translation setting. Leave it out where the screen shows
///   the meaning itself (a review card's answer, a word list).
/// - `plain`: the Latin text only, for prompts and answer options.
/// - `inline`: flows inside a sentence of the UI language; no line below.
/// - `class`: extra classes for the target-language text itself.
/// - `marked`: words to underline, e.g. the words a story is built on.
#[component]
pub fn Annotated(
    text: String,
    tag: &'static str,
    #[props(default)] meaning: Option<String>,
    #[props(default)] plain: bool,
    #[props(default)] inline: bool,
    #[props(default = true)] pron: bool,
    #[props(default)] class: String,
    #[props(default)] marked: Vec<String>,
) -> Element {
    let ui = use_context::<Ui>();
    let words = use_context::<Words>();
    let mut open = use_signal(|| None::<usize>);
    let mut revealed = use_signal(|| false);
    let analyzed = use_memo(use_reactive!(|text, marked| {
        let ready = !plain && *words.ready.read();
        ready.then(|| analyze(&text, &words.taught.read(), &marked))
    }));

    if plain {
        return rsx! {
            bdi { class: "latin annot plain {class}", lang: tag, dir: "ltr", "{text}" }
        };
    }
    let lang_class = if tag == "jv" { "jv" } else { "id" };
    let translation = ui.settings.read().translation;
    let meaning = meaning.filter(|m| !m.trim().is_empty() && !inline && translation != TranslationMode::Off);
    let kind = if inline { "annot inline" } else { "annot" };
    // Until the glossary is parsed (a moment after start-up) the words are
    // plain text; everything else already shows.
    let (doc, parts) = match analyzed() {
        Some((doc, parts)) => (Some(doc), parts),
        None => (None, Rc::new(vec![Piece { text: text.clone(), word: None, bridge: false, marked: false }])),
    };

    rsx! {
        span { class: "{kind} {lang_class}",
            bdi { class: "latin annot-text {class}", lang: tag, dir: "ltr",
                for (k , p) in parts.iter().cloned().enumerate() {
                    match p.word {
                        Some(i) => rsx! {
                            span {
                                key: "{k}",
                                class: "annot-w",
                                class: if open() == Some(i) { "on" },
                                class: if p.bridge { "bridge-word" },
                                class: if p.marked { "target" },
                                role: "button",
                                tabindex: "0",
                                // Annotated text often sits inside a button (examples, dialogue lines).
                                onclick: move |e| {
                                    e.stop_propagation();
                                    open.set(if open() == Some(i) { None } else { Some(i) });
                                },
                                onkeydown: move |e: KeyboardEvent| {
                                    if e.key() == Key::Enter {
                                        e.stop_propagation();
                                        open.set(Some(i));
                                    }
                                },
                                onpointerenter: move |e| {
                                    if e.data().pointer_type() == "pen" {
                                        open.set(Some(i));
                                    }
                                },
                                "{p.text}"
                            }
                        },
                        None => rsx! { span { key: "{k}", "{p.text}" } },
                    }
                }
            }
            if pron {
                Pron { text: text.clone(), tag, inline }
            }
            if let Some(m) = meaning {
                if translation == TranslationMode::Tap && !revealed() {
                    span {
                        class: "annot-reveal",
                        role: "button",
                        tabindex: "0",
                        onclick: move |e| {
                            e.stop_propagation();
                            revealed.set(true);
                        },
                        onkeydown: move |e: KeyboardEvent| {
                            if e.key() == Key::Enter {
                                e.stop_propagation();
                                revealed.set(true);
                            }
                        },
                        "{tt(ui.lang(), T::ShowTranslation)}"
                    }
                } else {
                    span { class: "annot-tr", Rich { text: m, target: tag } }
                }
            }
            if let (Some(doc), Some(i)) = (doc, open()) {
                WordSheet { doc, i, tag, on_close: move |_| open.set(None) }
            }
        }
    }
}

/// The card under the sentence for one tapped word: what the Reader's card
/// says, without its status and highlight tools.
#[component]
fn WordSheet(doc: Rc<ReadDoc>, i: usize, tag: &'static str, on_close: EventHandler) -> Element {
    let ui = use_context::<Ui>();
    let teach = use_context::<TeachInbox>();
    let lang = ui.lang();
    let Some(t) = doc.tokens.get(i) else { return rsx! {} };
    let Some(w) = t.word.clone() else { return rsx! {} };
    let surface = doc.text(t.start, t.end).to_string();
    let lemma = w.shown().map(|info| info.lemma).filter(|l| !l.eq_ignore_ascii_case(&surface));
    let word_tag = if w.tag == Tag::Javanese || tag == "jv" { "jv" } else { "id" };
    let seed = (w.shown().is_none() && w.key.is_some() && w.taught_draft().is_none()).then(|| TeachSeed {
        lang: if word_tag == "jv" { Lang::Jv } else { Lang::Id },
        word: w.analyzed.lookup_form(),
        context: Some(doc.sentence_of(i).to_string()),
    });

    rsx! {
        span {
            class: "annot-card",
            role: "dialog",
            aria_label: "{surface}",
            // Taps inside the card must not reach a button around the sentence.
            onclick: move |e| e.stop_propagation(),
            span { class: "annot-card-head",
                strong {
                    bdi { class: "latin", lang: word_tag, dir: "ltr", "{surface}" }
                    if let Some(l) = &lemma {
                        " ← "
                        bdi { class: "latin", lang: word_tag, dir: "ltr", "{l}" }
                    }
                }
                Pron { text: surface.clone(), tag: word_tag }
                button { class: "quiet small", onclick: move |_| on_close.call(()), "{tt(lang, T::Close)}" }
            }
            if let Some(b) = w.breakdown() {
                span { class: "field annot-parts",
                    span { class: "label", "{tt(lang, T::Breakdown)}" }
                    bdi { class: "latin", lang: word_tag, dir: "ltr", "{b}" }
                }
            }
            match w.shown() {
                Some(info) => rsx! {
                    InfoView { info, voice: lang }
                },
                None => rsx! {
                    span { class: "muted", "{tt(lang, T::NotInDictionary)}" }
                    if let Some(seed) = seed {
                        button {
                            class: "ghost small",
                            onclick: move |_| {
                                let mut inbox = teach.0;
                                inbox.set(Some(seed.clone()));
                                let mut screen = ui.screen;
                                screen.set(Screen::Teach);
                            },
                            "{tt(lang, T::TeachIt)}"
                        }
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> ReadDoc {
        let taught = PersonalDictionary::new(Vec::new());
        ReadDoc::from_text(text, Lexicon { glossary: crate::reader::glossary(), taught: &taught })
    }

    #[test]
    fn pieces_keep_the_text_and_mark_story_words() {
        let text = "Saya sedang membaca buku.";
        let parts = pieces(&doc(text), &["baca".to_string()]);
        assert_eq!(parts.iter().map(|p| p.text.as_str()).collect::<String>(), text);
        let marked: Vec<&str> = parts.iter().filter(|p| p.marked).map(|p| p.text.as_str()).collect();
        assert_eq!(marked, ["membaca"]);
        assert!(parts.iter().filter(|p| p.word.is_some()).count() >= 4, "every word is tappable");
    }

    #[test]
    fn bridge_words_are_gold() {
        let parts = pieces(&doc("Selamat pagi!"), &[]);
        let selamat = parts.iter().find(|p| p.text == "Selamat").unwrap();
        assert!(selamat.bridge);
        assert!(!parts.iter().find(|p| p.text == "pagi").unwrap().bridge);
    }
}
