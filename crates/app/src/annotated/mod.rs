//! Annotated text: how every Indonesian or Javanese sentence outside the
//! Reader is shown. The words are tappable (meaning, breakdown and Bridge
//! from local data, as in the Reader), the pronunciation and the translation
//! sit under the sentence, and all of it follows one set of settings.
//!
//! Prompts and the production side of an exercise use `plain`: the Latin
//! text alone, so nothing gives the answer away (same rule as `crate::pron`).

pub mod parts;
pub mod phrases;
pub mod text;

use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::grammar::word::{self, PartKind, WordClass};
use kabar_core::lang::Lang as CoreLang;
use kabar_core::lang::analyze::Morph;
use kabar_core::learn::PersonalDictionary;
use kabar_core::settings::TranslationMode;
use kabar_protocol::content::Lang;

use self::parts::{ClassChip, Legend, PartsLine, SayWord};
use self::text::{T, tt};
use crate::embedded::{Css, css};
use crate::lesson::Rich;
use crate::pron::{Pron, Say};
use crate::reader::card::InfoView;
use crate::reader::doc::{Lexicon, ReadDoc, Tag, Word};
use crate::reader::explain::voice_of;
use crate::reader::glossary::{Gloss, WordLang};
use crate::reader::grammar::{DocGrammar, GrammarAt, syllabus};
use crate::sheet::Sheet;
use crate::teach::{TeachInbox, TeachSeed};
use crate::{Db, Screen, Ui};

pub const CSS: Css = css!("assets/annotated.css");

/// What every annotated sentence shares: whether the glossary is parsed yet
/// (~100 ms of work, done off the UI thread the first time a sentence needs
/// it) and the words the learner taught Kabar (read once, refreshed when
/// they change).
#[derive(Clone, Copy)]
pub struct Words {
    ready: Signal<bool>,
    started: CopyValue<bool>,
    taught: Signal<Rc<PersonalDictionary>>,
}

impl Words {
    /// Re-reads the taught words after Teach saves or sync brings new ones.
    pub fn refresh(mut self, db: &Db) {
        self.taught.set(Rc::new(taught(db)));
    }

    /// Parses the glossary in the background on first need; screens without
    /// target text never load it.
    fn load(mut self) {
        if *self.started.peek() {
            return;
        }
        self.started.set(true);
        let (tx, rx) = futures_channel::oneshot::channel::<()>();
        // The same OnceLock as `reader::warm_up`: whichever thread comes
        // second waits for the first parse instead of doing its own.
        std::thread::spawn(move || {
            crate::reader::glossary();
            phrases::warm_up();
            let _ = tx.send(());
        });
        spawn(async move {
            let _ = rx.await;
            self.ready.set(true);
        });
    }
}

fn taught(db: &Db) -> PersonalDictionary {
    PersonalDictionary::new(db.0.borrow().taught_entries().unwrap_or_default())
}

/// Provides [`Words`] to the app. Nothing is parsed until a sentence shows.
pub fn use_words(db: &Db) -> Words {
    use_context_provider(|| Words {
        ready: Signal::new(false),
        started: CopyValue::new(false),
        taught: Signal::new(Rc::new(taught(db))),
    })
}

/// One run of the sentence: text between words, or a word (token index)
/// with its colours and short meaning.
#[derive(Clone, Debug, Default, PartialEq)]
struct Piece {
    text: String,
    word: Option<usize>,
    bridge: bool,
    marked: bool,
    /// The word class's CSS class (`wc-verb`, …).
    class: Option<&'static str>,
    /// The word as written, cut into prefix / root / suffix runs.
    spans: Vec<(String, &'static str)>,
    /// What goes under the word in the side-by-side view.
    gloss: Option<Gloss>,
    /// A cast member's name ("Gadis"): shown as a name, not glossed.
    name: bool,
    /// On the first word of a fixed expression: the index of the part that
    /// ends it and what the whole means ("Selamat pagi" = صباح الخير).
    phrase: Option<(usize, Gloss)>,
}

impl Piece {
    /// The word's classes: always `annot-w`, then each layer that is on.
    fn classes(&self, open: bool, glosses: bool) -> String {
        let mut c = String::from("annot-w");
        let mut add = |on: bool, name: &str| {
            if on {
                c.push(' ');
                c.push_str(name);
            }
        };
        add(open, "on");
        add(self.bridge, "bridge-word");
        add(self.marked, "target");
        add(glosses, "glossed");
        // Colours always go on; the Settings switch turns them off in CSS.
        if let Some(class) = self.class {
            add(true, "wc-line");
            add(true, class);
        }
        c
    }
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

/// The language a word is coloured in: its dictionary entry's, else the text's.
fn word_lang(w: &Word, text_lang: CoreLang) -> CoreLang {
    match w.tag {
        Tag::Javanese => CoreLang::Javanese,
        Tag::Indonesian => CoreLang::Indonesian,
        _ => text_lang,
    }
}

/// The first sense, short enough to sit under a word: "going to, will ·
/// to want" → "going to".
fn short(gloss: &str) -> &str {
    let first =
        gloss.split([';', '·', ',', '،', '؛', '(', '/']).map(str::trim).find(|s| !s.is_empty()).unwrap_or("");
    if first.chars().count() > 22 { "" } else { first }
}

fn pieces(doc: &ReadDoc, marked: &[String], text_lang: CoreLang) -> Vec<Piece> {
    let plain = |text: &str| Piece { text: text.to_string(), ..Piece::default() };
    let tag = if text_lang == CoreLang::Javanese { "jv" } else { "id" };
    let mut out = Vec::new();
    let mut at = 0;
    // The token that ends the open phrase, and where its first word sits in `out`.
    let mut phrase_end: Option<(usize, usize)> = None;
    for (i, t) in doc.tokens.iter().enumerate() {
        if t.start > at {
            out.push(plain(doc.text(at, t.start)));
        }
        let text = doc.text(t.start, t.end).to_string();
        let Some(w) = t.word.as_ref().filter(|w| w.key.is_some() || w.has_gloss()) else {
            out.push(plain(&text));
            close_phrase(&mut out, &mut phrase_end, i);
            at = t.end;
            continue;
        };
        let info = w.shown();
        let class = word::class_in(
            &w.analyzed,
            word_lang(w, text_lang),
            info.as_ref().and_then(|i| i.pos.as_deref()),
        );
        let spans = word::surface_spans(&text, &w.analyzed)
            .into_iter()
            .filter_map(|(r, kind)| Some((text.get(r)?.to_string(), kind.css())))
            .collect();
        let name = text.starts_with(char::is_uppercase) && crate::portraits::is_cast_name(&text);
        let phrase = match phrase_end {
            Some((end, _)) if i <= end => None,
            _ => phrases::at(doc, i, tag).map(|(end, p)| {
                phrase_end = Some((end, out.len()));
                (end, p.gloss.clone())
            }),
        };
        out.push(Piece {
            word: Some(i),
            bridge: w.is_bridge(),
            marked: !marked.is_empty() && is_marked(w, &text, marked),
            class: class.map(WordClass::css),
            spans,
            gloss: if name { None } else { info.map(|i| i.gloss) },
            name,
            phrase,
            text,
        });
        close_phrase(&mut out, &mut phrase_end, i);
        at = t.end;
    }
    if at < doc.body.len() {
        out.push(plain(doc.text(at, doc.body.len())));
    }
    out
}

/// At the phrase's last token, turn the token index its first word holds
/// into the index of the last run, which is what [`groups`] reads.
fn close_phrase(out: &mut [Piece], open: &mut Option<(usize, usize)>, token: usize) {
    if let Some((end, first)) = *open
        && token == end
    {
        let end = out.len() - 1;
        if let Some((last, _)) = out[first].phrase.as_mut() {
            *last = end;
        }
        *open = None;
    }
}

/// The runs grouped for drawing: a fixed expression's parts together with
/// its meaning, every other run on its own.
fn groups(parts: &[Piece]) -> Vec<(Option<Gloss>, Vec<Piece>)> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < parts.len() {
        match &parts[k].phrase {
            Some((last, gloss)) if *last > k && *last < parts.len() => {
                out.push((Some(gloss.clone()), parts[k..=*last].to_vec()));
                k = last + 1;
            }
            _ => {
                out.push((None, vec![parts[k].clone()]));
                k += 1;
            }
        }
    }
    out
}

/// Analyzes `text` with the Reader's engine and glossary, words the learner
/// taught Kabar on top, and splits it into runs. Once per text: callers
/// memoize it.
fn analyze(
    text: &str,
    lang: CoreLang,
    taught: &PersonalDictionary,
    marked: &[String],
) -> (Rc<ReadDoc>, Rc<Vec<Piece>>) {
    // The screen knows the language ("jv" on a J1 lesson): words both
    // dictionaries have take that sense, even standing alone.
    let hint = if lang == CoreLang::Javanese { WordLang::Javanese } else { WordLang::Indonesian };
    let doc =
        ReadDoc::from_text_in(text, Lexicon { glossary: crate::reader::glossary(), taught }, Some(hint));
    let parts = pieces(&doc, marked, lang);
    (Rc::new(doc), Rc::new(parts))
}

/// The language a `tag` ("id", "jv") names.
fn text_lang(tag: &str) -> CoreLang {
    if tag == "jv" { CoreLang::Javanese } else { CoreLang::Indonesian }
}

/// One tappable word: its colours, and its short meaning under it when
/// `glosses` is on (a name says it's a name).
#[component]
fn WordRun(p: Piece, i: usize, open: Signal<Option<usize>>, glosses: bool) -> Element {
    let ui = use_context::<Ui>();
    let voice = ui.lang();
    let under =
        if p.name { tt(voice, T::Name) } else { p.gloss.as_ref().map(|g| short(g.get(voice))).unwrap_or("") };
    rsx! {
        span {
            class: p.classes(open() == Some(i), glosses),
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
            span { class: "annot-word",
                if p.class.is_some() && !p.spans.is_empty() {
                    for (j , (run , css)) in p.spans.iter().enumerate() {
                        span { key: "{j}", class: *css, "{run}" }
                    }
                } else {
                    "{p.text}"
                }
            }
            if glosses {
                span {
                    class: if p.name { "annot-g annot-name" } else { "annot-g" },
                    lang: voice.tag(),
                    dir: voice.dir().as_str(),
                    "{under}"
                }
            }
        }
    }
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
/// - `say`: a speaker beside the text; off where the text already plays
///   when tapped (a dialogue bubble, an example button, a chat bubble).
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
    #[props(default = true)] say: bool,
) -> Element {
    let ui = use_context::<Ui>();
    let words = use_context::<Words>();
    let mut open = use_signal(|| None::<usize>);
    let mut revealed = use_signal(|| false);
    use_hook(move || {
        if !plain {
            words.load();
        }
    });
    let analyzed = use_memo(use_reactive!(|text, marked| {
        let ready = !plain && *words.ready.read();
        ready.then(|| analyze(&text, text_lang(tag), &words.taught.read(), &marked))
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
        None => (None, Rc::new(vec![Piece { text: text.clone(), ..Piece::default() }])),
    };
    // Side by side: each word's meaning under it, as in the Reader.
    let glosses = !inline && translation == TranslationMode::On;
    let voice = ui.lang();

    rsx! {
        span { class: "{kind} {lang_class}",
            bdi { class: "latin annot-text {class}", lang: tag, dir: "ltr",
                for (k , (phrase , group)) in groups(&parts).into_iter().enumerate() {
                    if let Some(g) = phrase {
                        // A fixed expression: its words keep their own taps
                        // and colours, and one meaning sits under all of them.
                        span { key: "{k}", class: if glosses { "annot-phrase glossed" } else { "annot-phrase" },
                            span { class: "annot-phrase-words",
                                for (m , p) in group.into_iter().enumerate() {
                                    match p.word {
                                        Some(i) => rsx! {
                                            WordRun { key: "{m}", p, i, open, glosses: false }
                                        },
                                        None => rsx! { span { key: "{m}", "{p.text}" } },
                                    }
                                }
                            }
                            if glosses {
                                span { class: "annot-g", lang: voice.tag(), dir: voice.dir().as_str(), "{short(g.get(voice))}" }
                            }
                        }
                    } else {
                        for (m , p) in group.into_iter().enumerate() {
                            match p.word {
                                Some(i) => rsx! {
                                    WordRun { key: "{k}.{m}", p, i, open, glosses }
                                },
                                None => rsx! { span { key: "{k}.{m}", "{p.text}" } },
                            }
                        }
                    }
                }
            }
            if say {
                Say { text: text.clone(), tag }
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

/// The word sheet for one tapped word (`crate::sheet`): what the Reader's
/// card says, without its status and highlight tools.
#[component]
fn WordSheet(doc: Rc<ReadDoc>, i: usize, tag: &'static str, on_close: EventHandler) -> Element {
    let ui = use_context::<Ui>();
    let teach = use_context::<TeachInbox>();
    let db = use_context::<Db>();
    let lang = ui.lang();
    // Spotted once per text, offline, when a word of it is first opened.
    let grammar = use_memo(use_reactive!(|doc| Rc::new(DocGrammar::new(&doc, &syllabus(&db)))));
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
        Sheet { label: surface.clone(), on_close: move |_| on_close.call(()),
            span { class: "annot-card-head",
                strong {
                    bdi { class: "latin", lang: word_tag, dir: "ltr", "{surface}" }
                    if let Some(l) = &lemma {
                        " ← "
                        bdi { class: "latin", lang: word_tag, dir: "ltr", "{l}" }
                    }
                }
                SayWord { text: surface.clone(), tag: word_tag }
            }
            Breakdown { word: w.clone(), tag }
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
            GrammarAt { doc: doc.clone(), grammar: grammar(), i, voice: voice_of(lang) }
        }
    }
}

/// What kind of word it is and how it is built, in the same colours as the
/// sentence: `peng + alam + an`, with a line on what each affix does. The
/// same pieces as the Reader's breakdown.
#[component]
fn Breakdown(word: Word, tag: &'static str) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let voice = voice_of(lang);
    let core = word_lang(&word, text_lang(tag));
    let info = word.shown();
    let class = word::class_in(&word.analyzed, core, info.as_ref().and_then(|i| i.pos.as_deref()));
    let parts = word::parts_of_token(&word.analyzed);
    let notes: Vec<(&'static str, String, word::AffixNote)> = parts
        .iter()
        .filter(|p| p.kind != PartKind::Root)
        .filter_map(|p| Some((p.kind.css(), p.label.clone(), word::affix_note(core, &p.label)?)))
        .fold(Vec::new(), |mut out, n| {
            // A confix (ke-an) is two pieces with one note.
            if !out.iter().any(|(_, l, _)| *l == n.1) {
                out.push(n);
            }
            out
        });
    let redup = word.analyzed.token.redup.as_ref().map(|(kind, _)| word::redup_note(*kind, class).get(voice));
    let word_tag = if core == CoreLang::Javanese { "jv" } else { "id" };
    if class.is_none() && parts.len() < 2 && redup.is_none() {
        return rsx! {};
    }
    rsx! {
        span { class: "field annot-parts",
            span { class: "label",
                "{tt(lang, T::Breakdown)}"
                if let Some(c) = class {
                    " "
                    ClassChip { class: c, voice: lang }
                }
            }
            if parts.len() > 1 {
                PartsLine { parts: parts.clone(), class, labels: true, voice: lang }
            }
            for (css , label , note) in notes {
                span { key: "{label}", class: "annot-note",
                    bdi { class: "latin piece {css}", lang: word_tag, dir: "ltr", "{label}" }
                    // Core notes isolate their Latin with U+2068/2069, which
                    // WebKitGTK ignores; Rich isolates it with markup instead.
                    Rich { text: note.meaning.get(voice).replace(['\u{2060}', '\u{2068}', '\u{2069}'], ""), target: word_tag }
                    if !note.verified {
                        span { class: "pill small", " {tt(lang, T::Unverified)}" }
                    }
                }
            }
            if let Some(r) = redup {
                span { class: "annot-note",
                    Rich { text: r.replace(['\u{2060}', '\u{2068}', '\u{2069}'], ""), target: word_tag }
                }
            }
            details { class: "annot-legend",
                summary { "{tt(lang, T::Legend)}" }
                Legend { voice: lang }
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
        let parts = pieces(&doc(text), &["baca".to_string()], CoreLang::Indonesian);
        assert_eq!(parts.iter().map(|p| p.text.as_str()).collect::<String>(), text);
        let marked: Vec<&str> = parts.iter().filter(|p| p.marked).map(|p| p.text.as_str()).collect();
        assert_eq!(marked, ["membaca"]);
        assert!(parts.iter().filter(|p| p.word.is_some()).count() >= 4, "every word is tappable");
    }

    #[test]
    fn greetings_names_and_titles_get_the_meaning_they_have_here() {
        let d = doc("Halo! Selamat pagi, Bu. Nama saya Gadis.");
        let parts = pieces(&d, &[], CoreLang::Indonesian);
        let word = |w: &str| parts.iter().find(|p| p.text == w).unwrap();
        // "Selamat pagi" is one greeting, not "congratulations" + "morning".
        let (last, gloss) = word("Selamat").phrase.clone().expect("a phrase");
        assert_eq!(parts[last].text, "pagi");
        assert_eq!(gloss.en, "good morning");
        let grouped = groups(&parts);
        let phrase = grouped.iter().find(|(g, _)| g.is_some()).unwrap();
        assert_eq!(phrase.1.iter().map(|p| p.text.as_str()).collect::<String>(), "Selamat pagi");
        // A cast member's name is a name; the same word in lowercase is "girl".
        assert!(word("Gadis").name && word("Gadis").gloss.is_none());
        let girl = pieces(&doc("Gadis itu tinggal di sebelah."), &[], CoreLang::Indonesian);
        assert!(girl[0].name, "capitalized at the start it still reads as the name");
        let girl = pieces(&doc("Ada gadis di sana."), &[], CoreLang::Indonesian);
        assert!(girl.iter().any(|p| p.text == "gadis" && !p.name && p.gloss.is_some()));
        // "Bu" in an Indonesian line is the Indonesian title, not Javanese "Mum".
        assert!(!word("Bu").gloss.as_ref().unwrap().arz.contains("ماما"));
    }

    #[test]
    fn a_javanese_screen_gives_shared_words_their_javanese_sense() {
        // #378: "awan" alone on a J1 end screen is Javanese "daytime", not
        // Indonesian "cloud"; the same word on an Indonesian screen stays "cloud".
        let taught = PersonalDictionary::new(Vec::new());
        let gloss = |lang| {
            let (_, parts) = analyze("awan", lang, &taught, &[]);
            parts.iter().find(|p| p.text == "awan").and_then(|p| p.gloss.clone()).expect("a gloss")
        };
        let jv = gloss(CoreLang::Javanese);
        assert!(jv.en.contains("midday") || jv.en.contains("daytime"), "{jv:?}");
        assert!(!jv.arz.contains("سحاب"), "{jv:?}");
        assert!(gloss(CoreLang::Indonesian).en.contains("cloud"));
    }

    #[test]
    fn the_word_card_finds_the_rules_a_word_takes_part_in() {
        let d = doc("Saya sedang membaca buku.");
        let grammar = DocGrammar::new(&d, &Default::default());
        let at = |w: &str| {
            let i = d.tokens.iter().position(|t| d.text(t.start, t.end) == w).unwrap();
            grammar.at(i).into_iter().map(|(f, _)| f.grammar).collect::<Vec<_>>()
        };
        assert_eq!(at("sedang"), ["id.aspect.markers"]);
        assert!(at("buku").is_empty());
    }

    #[test]
    fn words_carry_their_class_parts_and_short_meaning() {
        let parts = pieces(&doc("Rumahnya dibersihkan."), &[], CoreLang::Indonesian);
        let word = |w: &str| parts.iter().find(|p| p.text == w).unwrap();
        let cleaned = word("dibersihkan");
        assert_eq!(cleaned.class, Some("wc-verb"));
        let runs: Vec<(&str, &str)> = cleaned.spans.iter().map(|(t, c)| (t.as_str(), *c)).collect();
        assert_eq!(runs, [("di", "wp-prefix"), ("bersih", "wp-root"), ("kan", "wp-suffix")]);
        assert_eq!(word("Rumahnya").class, Some("wc-noun"));
        assert!(word("Rumahnya").gloss.is_some());
        assert_eq!(short("going to, will · to want"), "going to");
        assert_eq!(short("ده، دي، دول"), "ده");
        assert_eq!(short("مساء الخير بالليل / تصبح على خير"), "مساء الخير بالليل");
    }

    #[test]
    fn bridge_words_are_gold() {
        let parts = pieces(&doc("Selamat pagi!"), &[], CoreLang::Indonesian);
        let selamat = parts.iter().find(|p| p.text == "Selamat").unwrap();
        assert!(selamat.bridge);
        assert!(!parts.iter().find(|p| p.text == "pagi").unwrap().bridge);
    }
}
