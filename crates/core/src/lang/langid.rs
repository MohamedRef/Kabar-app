//! Language ID per token: slang table → marker words → dictionaries →
//! sentence context. Tokens nothing can place stay `Unknown` for the AI
//! fallback instead of being guessed.

use serde::{Deserialize, Serialize};

use super::Lang;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Slang,
    Marker,
    Dictionary,
    Context,
    Capitalized,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LangGuess {
    pub lang: Lang,
    pub confidence: f32,
    pub source: Source,
}

impl LangGuess {
    const fn new(lang: Lang, confidence: f32, source: Source) -> Self {
        Self { lang, confidence, source }
    }

    pub const UNKNOWN: Self = Self::new(Lang::Unknown, 0.0, Source::None);
}

const JV_MARKERS: &[&str] = &[
    "ora",
    "opo",
    "piye",
    "arep",
    "wis",
    "durung",
    "karo",
    "kowe",
    "sing",
    "iki",
    "iku",
    "kae",
    "ning",
    "nang",
    "neng",
    "sampun",
    "nggih",
    "mboten",
    "panjenengan",
    "sampeyan",
    "koen",
    "rek",
    "wes",
    "uwis",
    "wae",
    "ae",
    "lho",
    "ta",
    "ngene",
    "ngono",
    "pira",
    "piro",
    "durung",
    "dhahar",
    "nedha",
    "mangan",
    "sek",
    "sik",
    "mas",
    "mbak",
];
const ID_MARKERS: &[&str] = &[
    "tidak",
    "apa",
    "bagaimana",
    "mau",
    "sudah",
    "belum",
    "dengan",
    "kamu",
    "yang",
    "ini",
    "itu",
    "adalah",
    "saya",
    "akan",
    "tetapi",
    "juga",
    "karena",
    "sangat",
    "banget",
    "gue",
    "gua",
    "lo",
    "lu",
    "dan",
    "di",
    "ke",
    "dari",
    "untuk",
    "sedang",
    "kemarin",
    "nggak",
    "enggak",
];
const EN_WORDS: &[&str] = &[
    "the",
    "is",
    "are",
    "was",
    "which",
    "literally",
    "and",
    "of",
    "to",
    "in",
    "for",
    "with",
    "you",
    "it",
    "that",
    "this",
    "so",
    "but",
    "really",
    "very",
    "ok",
    "okay",
    "please",
    "thanks",
    "sorry",
    "actually",
    "basically",
    "anyway",
    "btw",
    "gb",
    "tb",
    "pro",
    "max",
    "mini",
    "plus",
    "ultra",
    "no",
    "minus",
];
const BRANDS: &[&str] =
    &["iphone", "ibox", "samsung", "galaxy", "whatsapp", "tokopedia", "shopee", "gojek", "grab"];
/// Words common to both languages (or different words with the same spelling).
const SHARED: &[&str] = &["tak", "aku", "apa", "ana", "lapor"];

/// What the dictionaries say about one word.
#[derive(Clone, Copy, Debug, Default)]
pub struct DictHits {
    pub id: bool,
    pub jv: bool,
}

/// One word-like token as seen by the identifier.
pub struct Item<'a> {
    pub lower: &'a str,
    /// Slang table language, if the token was normalized.
    pub slang: Option<Lang>,
    pub dict: DictHits,
    pub capitalized: bool,
    pub sentence_start: bool,
}

pub fn identify(items: &[Item<'_>]) -> Vec<LangGuess> {
    let mut out: Vec<LangGuess> = items.iter().map(first_pass).collect();

    // Context: the dominant of Indonesian vs Javanese among confident tokens.
    let (mut id, mut jv) = (0.0f32, 0.0f32);
    for g in &out {
        if g.confidence >= 0.85 {
            match g.lang {
                Lang::Indonesian => id += g.confidence,
                Lang::Javanese => jv += g.confidence,
                _ => {}
            }
        }
    }
    let dominant = if jv > id { Lang::Javanese } else { Lang::Indonesian };

    for (g, item) in out.iter_mut().zip(items) {
        let ambiguous = (item.dict.id && item.dict.jv) || SHARED.contains(&item.lower);
        if g.lang == Lang::Unknown && ambiguous {
            *g = LangGuess::new(dominant, 0.6, Source::Context);
        }
    }
    out
}

fn first_pass(item: &Item<'_>) -> LangGuess {
    let w = item.lower;
    if let Some(lang) = item.slang {
        return LangGuess::new(lang, 0.85, Source::Slang);
    }
    if SHARED.contains(&w) {
        return LangGuess::UNKNOWN;
    }
    if JV_MARKERS.contains(&w) {
        return LangGuess::new(Lang::Javanese, 0.95, Source::Marker);
    }
    if ID_MARKERS.contains(&w) {
        return LangGuess::new(Lang::Indonesian, 0.95, Source::Marker);
    }
    if BRANDS.contains(&w) {
        return LangGuess::new(Lang::Proper, 0.9, Source::Marker);
    }
    if EN_WORDS.contains(&w) {
        return LangGuess::new(Lang::English, 0.9, Source::Marker);
    }
    match (item.dict.id, item.dict.jv) {
        (true, false) => return LangGuess::new(Lang::Indonesian, 0.75, Source::Dictionary),
        (false, true) => return LangGuess::new(Lang::Javanese, 0.75, Source::Dictionary),
        (true, true) => return LangGuess::UNKNOWN,
        (false, false) => {}
    }
    if item.capitalized && !item.sentence_start {
        return LangGuess::new(Lang::Proper, 0.6, Source::Capitalized);
    }
    LangGuess::UNKNOWN
}
