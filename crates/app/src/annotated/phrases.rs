//! Fixed expressions glossed as one: "Selamat pagi" is صباح الخير, not
//! "congratulations" + "morning". The phrases are every multi-word new word
//! of the lessons (reviewed curriculum, with its meanings) plus a small
//! unverified seed of everyday greetings the lessons teach inside slides.

use std::collections::HashMap;
use std::sync::LazyLock;

use kabar_core::lesson::parse_lesson;

use crate::embedded::{self, LESSONS};
use crate::reader::doc::ReadDoc;
use crate::reader::glossary::Gloss;

/// One expression and what it means as a whole.
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    /// Lowercase words, e.g. ["selamat", "pagi"].
    pub words: Vec<String>,
    /// "id" or "jv".
    pub lang: &'static str,
    pub gloss: Gloss,
}

/// (lang, phrase, [arz, ar, en]). Lesson entries of the same phrase win.
const SEED: &[(&str, &str, [&str; 3])] = &[
    ("id", "selamat pagi", ["صباح الخير", "صباح الخير", "good morning"]),
    ("id", "selamat siang", ["نهارك سعيد", "نهارك سعيد", "good day"]),
    ("id", "selamat sore", ["مسا الخير", "مساء الخير", "good afternoon"]),
    ("id", "selamat malam", ["مسا الخير", "مساء الخير", "good evening"]),
    ("id", "selamat datang", ["أهلًا وسهلًا", "أهلًا وسهلًا", "welcome"]),
    ("id", "selamat jalan", ["مع السلامة", "مع السلامة", "goodbye (to who leaves)"]),
    ("id", "selamat tinggal", ["مع السلامة", "مع السلامة", "goodbye (to who stays)"]),
    ("id", "selamat ulang tahun", ["كل سنة وإنت طيب", "عيد ميلاد سعيد", "happy birthday"]),
    ("id", "terima kasih", ["شكرًا", "شكرًا", "thank you"]),
    ("id", "sampai jumpa", ["أشوفك بعدين", "إلى اللقاء", "see you"]),
    ("id", "apa kabar", ["إزيك؟", "كيف حالك؟", "how are you?"]),
    ("jv", "sugeng enjing", ["صباح الخير", "صباح الخير", "good morning"]),
    ("jv", "sugeng siang", ["نهارك سعيد", "نهارك سعيد", "good day"]),
    ("jv", "sugeng sonten", ["مسا الخير", "مساء الخير", "good afternoon"]),
    ("jv", "sugeng dalu", ["تصبح على خير", "تصبح على خير", "good night"]),
    ("jv", "sugeng rawuh", ["أهلًا وسهلًا", "أهلًا وسهلًا", "welcome"]),
    ("jv", "matur nuwun", ["شكرًا", "شكرًا", "thank you"]),
];

/// Phrases by first word, longest first.
static PHRASES: LazyLock<HashMap<String, Vec<Phrase>>> = LazyLock::new(|| {
    let mut by_text: HashMap<(&'static str, String), Phrase> = HashMap::new();
    for &(lang, text, [arz, ar, en]) in SEED {
        let gloss = Gloss { arz: arz.into(), ar: ar.into(), en: en.into() };
        by_text.insert((lang, text.to_string()), phrase(lang, text, gloss));
    }
    for (_, yaml) in embedded::all(LESSONS) {
        let Ok(lesson) = parse_lesson(&yaml) else { continue };
        let lang = if lesson.language == "jv" { "jv" } else { "id" };
        for w in &lesson.new_words {
            let text = w.word.trim().to_lowercase();
            let n = text.split_whitespace().count();
            // Plain multi-word expressions only, not patterns ("ber- + …").
            if !(2..=4).contains(&n) || !text.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '-') {
                continue;
            }
            let m = &w.meaning;
            let gloss = Gloss { arz: m.ar_eg.clone(), ar: m.ar.clone(), en: m.en.clone() };
            by_text.insert((lang, text.clone()), phrase(lang, &text, gloss));
        }
    }
    let mut out: HashMap<String, Vec<Phrase>> = HashMap::new();
    for p in by_text.into_values() {
        out.entry(p.words[0].clone()).or_default().push(p);
    }
    for list in out.values_mut() {
        list.sort_by(|a, b| b.words.len().cmp(&a.words.len()).then_with(|| a.words.cmp(&b.words)));
    }
    out
});

fn phrase(lang: &'static str, text: &str, gloss: Gloss) -> Phrase {
    Phrase { words: text.split_whitespace().map(str::to_string).collect(), lang, gloss }
}

/// Builds the table ahead of the first sentence (called off the UI thread).
pub fn warm_up() {
    LazyLock::force(&PHRASES);
}

/// The phrase that starts at token `i` of `doc`, with the index of its last
/// token: the words must follow each other with only spaces between. A
/// phrase of the text's language (`lang`, "id" or "jv") wins over the other.
pub fn at(doc: &ReadDoc, i: usize, lang: &str) -> Option<(usize, &'static Phrase)> {
    let first = doc.tokens.get(i)?;
    let list = PHRASES.get(&doc.text(first.start, first.end).to_lowercase())?;
    let fits = |p: &Phrase| -> Option<usize> {
        let mut last = i;
        for (k, word) in p.words.iter().enumerate().skip(1) {
            let prev = &doc.tokens[i + k - 1];
            let t = doc.tokens.get(i + k)?;
            let gap = doc.text(prev.end, t.start);
            if t.word.is_none() || !gap.chars().all(char::is_whitespace) || gap.is_empty() {
                return None;
            }
            if doc.text(t.start, t.end).to_lowercase() != *word {
                return None;
            }
            last = i + k;
        }
        Some(last)
    };
    let mine = list.iter().filter(|p| p.lang == lang).find_map(|p| fits(p).map(|l| (l, p)));
    mine.or_else(|| list.iter().find_map(|p| fits(p).map(|l| (l, p))))
}
