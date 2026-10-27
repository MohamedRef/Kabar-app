use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::*;
use crate::lang::analyze::Engine;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// word → first sense's part of speech, from the gloss batches.
fn glossary(lang: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(repo().join("content/glosses").join(lang)).unwrap() {
        let path = e.unwrap().path();
        if path.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        for line in std::fs::read_to_string(&path).unwrap().lines().filter(|l| !l.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            let (Some(word), Some(pos)) = (v["word"].as_str(), v["senses"][0]["pos"].as_str()) else {
                continue;
            };
            out.entry(word.to_string()).or_insert_with(|| pos.to_string());
        }
    }
    if lang == "jv" {
        // The Reader also glosses every form in the speech-level table.
        let levels = std::fs::read_to_string(repo().join("content/javanese/speech-levels.jsonl")).unwrap();
        for line in levels.lines().filter(|l| !l.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            let Some(pos) = v["pos"].as_str() else { continue };
            for level in ["ngoko", "krama", "krama_inggil", "krama_andhap"] {
                for w in v[level].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
                    out.entry(w.to_string()).or_insert_with(|| pos.to_string());
                }
            }
        }
    }
    out
}

struct Fixture {
    engine: Engine,
    id: BTreeMap<String, String>,
    jv: BTreeMap<String, String>,
}

impl Fixture {
    fn new() -> Self {
        Self { engine: Engine::default(), id: glossary("id"), jv: glossary("jv") }
    }

    /// The dictionary pos the way the Reader finds it: the form, then the root.
    fn pos(&self, t: &AnalyzedToken) -> Option<&str> {
        let dict = if t.lang.lang == Lang::Javanese { &self.jv } else { &self.id };
        let root = parts_of_token(t).into_iter().find(|p| p.kind == PartKind::Root).map(|p| p.form);
        dict.get(&t.lookup_form()).or_else(|| root.and_then(|r| dict.get(&r))).map(String::as_str)
    }

    /// "word:class" for each word of `text`, "-" for no class.
    fn classes(&self, text: &str) -> Vec<String> {
        self.engine
            .analyze(text)
            .iter()
            .filter(|t| t.token.is_wordlike())
            .map(|t| {
                let c = class_of_token(t, self.pos(t)).map_or("-", |c| c.label().en);
                format!("{}:{c}", t.token.text)
            })
            .collect()
    }
}

fn token(engine: &Engine, text: &str, word: &str) -> AnalyzedToken {
    engine.analyze(text).into_iter().find(|t| t.token.text == word).unwrap()
}

/// "di|bersih|kan" with the kind of each piece.
fn spans(engine: &Engine, text: &str, word: &str) -> String {
    let t = token(engine, text, word);
    surface_spans(word, &t)
        .into_iter()
        .map(|(r, k)| format!("{}={}", &word[r], k.label().en))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn spec_texts_get_their_classes() {
    let f = Fixture::new();
    let golden: &[(&str, &[&str])] = &[
        (
            "Saya sedang membaca buku.",
            &["Saya:little word", "sedang:little word", "membaca:verb", "buku:noun"],
        ),
        ("Rumahnya dibersihkan kemarin.", &["Rumahnya:noun", "dibersihkan:verb", "kemarin:little word"]),
        ("Anak-anak bermain di taman.", &["Anak-anak:noun", "bermain:verb", "di:little word", "taman:noun"]),
        ("Kesehatan itu penting.", &["Kesehatan:noun", "itu:little word", "penting:adjective"]),
        ("Apa kabar?", &["Apa:little word", "kabar:noun"]),
        ("arp dicul piro", &["arp:little word", "dicul:verb", "piro:little word"]),
        ("Kowe wis mangan?", &["Kowe:little word", "wis:little word", "mangan:verb"]),
        ("Sampeyan sampun nedha?", &["Sampeyan:little word", "sampun:little word", "nedha:verb"]),
    ];
    let mut misses = Vec::new();
    for (text, want) in golden {
        let got = f.classes(text);
        if got != *want {
            misses.push(format!("{text}\n  want {want:?}\n  got  {got:?}"));
        }
    }
    assert!(misses.is_empty(), "{}", misses.join("\n"));
}

#[test]
fn affixes_decide_before_the_dictionary() {
    let e = Engine::default();
    let class = |text: &str, word: &str, pos: Option<&str>| class_of_token(&token(&e, text, word), pos);
    // The prefix makes the verb even when the root is a noun or adjective.
    assert_eq!(class("Dia membersihkan rumah.", "membersihkan", Some("adjective")), Some(WordClass::Verb));
    assert_eq!(class("Pembaca suka buku.", "Pembaca", None), Some(WordClass::Noun));
    assert_eq!(class("Kesehatan itu penting.", "Kesehatan", Some("adjective")), Some(WordClass::Noun));
    // ter-/se- on an adjective stay adjectives.
    assert_eq!(class("Rumah itu terbesar.", "terbesar", Some("adjective")), Some(WordClass::Adjective));
    assert_eq!(class("Rumah itu terbesar.", "terbesar", None), Some(WordClass::Verb));
    // Function words the affixes would get wrong.
    assert_eq!(class("Mengapa kamu pergi?", "Mengapa", None), Some(WordClass::Function));
    assert_eq!(class("Buku tersebut bagus.", "tersebut", None), Some(WordClass::Function));
    // The dictionary's function classes win over affixes.
    assert_eq!(class("Saya belajar.", "belajar", Some("verb")), Some(WordClass::Verb));
    assert_eq!(class("Dia di rumah.", "rumah", Some("pronoun")), Some(WordClass::Function));
    // Names have no class.
    assert_eq!(class("Saya ke Jakarta.", "Jakarta", Some("proper_noun")), None);
}

#[test]
fn pos_names_map_to_classes() {
    for (pos, want) in [
        ("verb", Some(WordClass::Verb)),
        ("noun", Some(WordClass::Noun)),
        ("adjective", Some(WordClass::Adjective)),
        ("adverb", Some(WordClass::Function)),
        ("pronoun", Some(WordClass::Function)),
        ("particle", Some(WordClass::Function)),
        ("classifier", Some(WordClass::Noun)),
        ("proper_noun", None),
        ("Phrase", None),
        ("transitive verb", Some(WordClass::Verb)),
    ] {
        assert_eq!(class_of_pos(pos), want, "{pos}");
    }
}

#[test]
fn spans_colour_the_written_word() {
    let e = Engine::default();
    assert_eq!(spans(&e, "Rumahnya dibersihkan kemarin.", "Rumahnya"), "Rumah=root nya=attached word");
    assert_eq!(spans(&e, "Rumahnya dibersihkan kemarin.", "dibersihkan"), "di=prefix bersih=root kan=suffix");
    assert_eq!(
        spans(&e, "Kesehatan itu penting.", "Kesehatan"),
        "Ke=wrap-around affix sehat=root an=wrap-around affix"
    );
    assert_eq!(spans(&e, "Saya sedang membaca buku.", "membaca"), "mem=prefix baca=root");
    // A changed root keeps the written letters.
    assert_eq!(spans(&e, "Dia menulis surat.", "menulis"), "men=prefix ulis=root");
    assert_eq!(spans(&e, "Anak-anak bermain di taman.", "bermain"), "ber=prefix main=root");
    // Doubled words and chat spellings stay whole.
    assert_eq!(spans(&e, "Anak-anak bermain di taman.", "Anak-anak"), "Anak-anak=root");
    assert_eq!(spans(&e, "arp dicul piro", "arp"), "arp=root");
    assert_eq!(spans(&e, "Saya sedang membaca buku.", "buku"), "buku=root");
}

#[test]
fn javanese_parts() {
    let e = Engine::default();
    let parts = |text: &str, word: &str| {
        parts_of_token(&token(&e, text, word))
            .into_iter()
            .map(|p| format!("{}:{}", p.label, p.kind.label().en))
            .collect::<Vec<_>>()
            .join(" ")
    };
    // "dicul" is read as its full form, diculke.
    assert_eq!(parts("arp dicul piro", "dicul"), "di-:prefix cul:root -ke:suffix");
    assert_eq!(spans(&e, "arp dicul piro", "dicul"), "di=prefix cul=root");
    assert_eq!(parts("Omahe adoh.", "Omahe"), "omah:root -e:attached word");
    assert_eq!(spans(&e, "Omahe adoh.", "Omahe"), "Omah=root e=attached word");
    assert_eq!(parts("Aku tuku klambi.", "tuku"), "tuku:root");
}

#[test]
fn every_affix_the_engine_finds_has_a_note() {
    // Run every glossed word through the engine and collect the labels.
    let f = Fixture::new();
    let mut missing = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for (lang, dict) in [(Lang::Indonesian, &f.id), (Lang::Javanese, &f.jv)] {
        for word in dict.keys().filter(|w| !w.contains(' ')) {
            let morph = match lang {
                Lang::Javanese => {
                    crate::lang::morph_jv::analyze(word, &crate::lang::BuiltinLexicon::default())
                        .map(Morph::Javanese)
                }
                _ => crate::lang::morph_id::IndonesianMorph::default().analyze(word).map(Morph::Indonesian),
            };
            let Some(morph) = morph else { continue };
            for p in parts_of(&morph).1.into_iter().filter(|p| p.kind != PartKind::Root) {
                seen.insert((lang == Lang::Javanese, p.label.clone()));
                if affix_note(lang, &p.label).is_none() {
                    missing.insert(format!("{lang:?} {}", p.label));
                }
            }
        }
    }
    assert!(seen.len() > 20, "{seen:?}");
    assert!(missing.is_empty(), "no note for {missing:?}");
}

#[test]
fn javanese_notes_stay_unverified() {
    assert!(affix_note(Lang::Indonesian, "meN-").unwrap().verified);
    assert!(!affix_note(Lang::Javanese, "N-").unwrap().verified);
    assert_eq!(affix_note(Lang::Javanese, "-ake").unwrap().makes, Some(WordClass::Verb));
    assert_eq!(affix_note(Lang::Indonesian, "nonsense"), None);
}

#[test]
fn every_mark_has_a_theme_token_and_labels() {
    let css = crate::theme::stylesheet();
    let mut names = BTreeSet::new();
    for m in LEGEND.into_iter().chain([Mark::Part(PartKind::Root)]) {
        assert!(css.contains(&format!("--{}:", m.css())), "no --{} in the theme", m.css());
        assert!(names.insert(m.css()), "{} twice", m.css());
        let l = m.label();
        for voice in [Voice::En, Voice::Ar, Voice::Arz] {
            assert!(!l.get(voice).is_empty());
        }
    }
}

fn library() -> Library {
    let mut files: Vec<(String, String)> = Vec::new();
    for lang in ["id", "jv"] {
        for e in std::fs::read_dir(repo().join("content/grammar").join(lang)).unwrap() {
            let path = e.unwrap().path();
            if path.extension().is_some_and(|x| x == "yaml") {
                files.push((path.display().to_string(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    Library::from_yaml(files.iter().map(|(p, t)| (p.as_str(), t.as_str()))).0
}

#[test]
fn word_detail_links_grammar_and_time_words() {
    let lib = library();
    let e = Engine::default();
    let detail = |text: &str, word: &str, pos: Option<&str>| {
        let sentence = e.analyze(text);
        let w = sentence.iter().find(|t| t.token.text == word).unwrap().clone();
        lib.word_detail(&w, &sentence, Lang::Indonesian, pos)
    };

    let d = detail("Saya sedang membaca buku.", "membaca", None);
    assert_eq!((d.root.as_str(), d.class), ("baca", Some(WordClass::Verb)));
    assert!(d.grammar.iter().any(|c| c.id.starts_with("id.prefix.men")), "{:?}", d.grammar);
    assert_eq!(d.aspect.iter().map(|a| a.form.as_str()).collect::<Vec<_>>(), ["sedang"]);

    let d = detail("Menulis surat.", "Menulis", None);
    assert_eq!(d.sound_change.as_deref(), Some("t→n"));

    let d = detail("Kesehatan itu penting.", "Kesehatan", None);
    assert!(d.grammar.iter().any(|c| c.id == "id.confix.ke-an"), "{:?}", d.grammar);
    assert!(d.aspect.is_empty());

    let d = detail("Anak-anak bermain di taman.", "Anak-anak", Some("noun"));
    assert!(d.redup.is_some());
    assert!(d.grammar.iter().any(|c| c.id == "id.reduplication.plural"), "{:?}", d.grammar);

    let d = detail("Saya sedang membaca buku.", "buku", Some("noun"));
    assert!(d.is_empty());
}

#[test]
fn latin_inside_arabic_notes_is_isolated() {
    // Without isolates, "(زي -kan)" reorders and wraps after the hyphen in RTL.
    let labels = ["meN-", "di-", "ke-", "se-", "-kan", "-i", "ke-an", "-nya"]
        .into_iter()
        .filter_map(|l| affix_note(Lang::Indonesian, l))
        .chain(["N-", "-ake", "-i", "-e", "-ipun"].into_iter().filter_map(|l| affix_note(Lang::Javanese, l)))
        .map(|n| n.meaning);
    for l in labels {
        for text in [l.ar, l.arz] {
            let bare: String = text
                .split('\u{2068}')
                .map(|piece| piece.split_once('\u{2069}').map_or(piece, |(_, rest)| rest))
                .collect();
            assert!(!bare.chars().any(|c| c.is_ascii_alphabetic()), "{text}");
        }
    }
}
