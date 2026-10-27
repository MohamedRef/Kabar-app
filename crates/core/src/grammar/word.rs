//! What kind of word this is and how it is built (spec §11), for the colour
//! system and the word card's "breakdown" (تفصيل).
//!
//! Everything is derived from the morphology engine (`morph_id`, `morph_jv`)
//! plus the dictionary's part of speech, so it stays offline and pure. Each
//! [`WordClass`] and [`PartKind`] has one fixed colour (theme tokens
//! `--wc-*` / `--wp-*`, see `theme::WordColors`) used everywhere a word is
//! shown: the text, the breakdown and the grammar notes.

use std::collections::BTreeSet;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use super::{Candidate, Library, Voice, weight, word_affixes};
use crate::lang::Lang;
use crate::lang::analyze::{AnalyzedToken, Morph};
use crate::lang::token::Reduplication;

/// A label in the three explanation voices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    pub en: &'static str,
    pub ar: &'static str,
    pub arz: &'static str,
}

impl Label {
    pub fn get(&self, voice: Voice) -> &'static str {
        match voice {
            Voice::En => self.en,
            Voice::Ar => self.ar,
            Voice::Arz => self.arz,
        }
    }
}

/// The kind of word, for its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WordClass {
    Verb,
    Noun,
    Adjective,
    /// Pronouns, prepositions, conjunctions, particles, question words,
    /// and the time / negation words around verbs (sudah, tidak, arep).
    Function,
}

impl WordClass {
    pub const ALL: [Self; 4] = [Self::Verb, Self::Noun, Self::Adjective, Self::Function];

    /// CSS class; its colour token is `--wc-…` with the same suffix.
    pub fn css(self) -> &'static str {
        match self {
            Self::Verb => "wc-verb",
            Self::Noun => "wc-noun",
            Self::Adjective => "wc-adj",
            Self::Function => "wc-func",
        }
    }

    pub fn label(self) -> Label {
        match self {
            Self::Verb => Label { en: "verb", ar: "فعل", arz: "فعل" },
            Self::Noun => Label { en: "noun", ar: "اسم", arz: "اسم" },
            Self::Adjective => Label { en: "adjective", ar: "صفة", arz: "صفة" },
            Self::Function => Label { en: "little word", ar: "أداة", arz: "أداة" },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartKind {
    Prefix,
    Root,
    Suffix,
    /// Wraps the root: ke-…-an, peN-…-an.
    Confix,
    /// -nya, -ku, -lah, Javanese -e/-ne.
    Clitic,
}

impl PartKind {
    pub const ALL: [Self; 5] = [Self::Prefix, Self::Root, Self::Suffix, Self::Confix, Self::Clitic];

    /// CSS class; its colour token is `--wp-…` with the same suffix. A root
    /// takes its word's class colour when the word's `wc-*` class is set.
    pub fn css(self) -> &'static str {
        match self {
            Self::Prefix => "wp-prefix",
            Self::Root => "wp-root",
            Self::Suffix => "wp-suffix",
            Self::Confix => "wp-confix",
            Self::Clitic => "wp-clitic",
        }
    }

    pub fn label(self) -> Label {
        match self {
            Self::Prefix => Label { en: "prefix", ar: "سابقة", arz: "سابقة (أول الكلمة)" },
            Self::Root => Label { en: "root", ar: "الجذر", arz: "الأصل" },
            Self::Suffix => Label { en: "suffix", ar: "لاحقة", arz: "لاحقة (آخر الكلمة)" },
            Self::Confix => Label {
                en: "wrap-around affix", ar: "زائدة مزدوجة", arz: "حاجة بتلف الكلمة"
            },
            Self::Clitic => Label {
                en: "attached word", ar: "ضمير أو أداة متصلة", arz: "ضمير أو أداة لازقة"
            },
        }
    }
}

/// One entry of the colour legend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Class(WordClass),
    Part(PartKind),
}

impl Mark {
    pub fn css(self) -> &'static str {
        match self {
            Self::Class(c) => c.css(),
            Self::Part(p) => p.css(),
        }
    }

    pub fn label(self) -> Label {
        match self {
            Self::Class(c) => c.label(),
            Self::Part(p) => p.label(),
        }
    }
}

/// Every colour, in the order a legend shows them. The root has no colour
/// of its own (it takes its word's), so it isn't listed.
pub const LEGEND: [Mark; 8] = [
    Mark::Class(WordClass::Verb),
    Mark::Class(WordClass::Noun),
    Mark::Class(WordClass::Adjective),
    Mark::Class(WordClass::Function),
    Mark::Part(PartKind::Prefix),
    Mark::Part(PartKind::Suffix),
    Mark::Part(PartKind::Confix),
    Mark::Part(PartKind::Clitic),
];

/// One piece of the word, in reading order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    pub kind: PartKind,
    /// As written in this word ("mem", "kan", the root). For a root changed
    /// at the join this is the dictionary form ("tulis" in menulis).
    pub form: String,
    /// Canonical label ("meN-", "-kan", "ke-an"); the root itself for roots.
    pub label: String,
}

/// What an affix does, in a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffixNote {
    pub meaning: Label,
    /// The class of word it usually makes, when it decides one.
    pub makes: Option<WordClass>,
    /// Javanese notes stay unverified until the learner approves them.
    pub verified: bool,
}

/// A time or aspect word in the same sentence as a verb (sudah, wis, arep).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AspectWord {
    /// As written ("udah").
    pub surface: String,
    /// The form the library knows ("sudah").
    pub form: String,
    pub grammar_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WordDetail {
    pub root: String,
    pub parts: Vec<Part>,
    /// The root's first sound changed at the join: "t→n" (menulis ← tulis).
    pub sound_change: Option<String>,
    /// anak-anak: the kind and the repeated base.
    pub redup: Option<(Reduplication, String)>,
    pub class: Option<WordClass>,
    /// Javanese roots the lexicon doesn't know are a structural guess.
    pub root_verified: bool,
    /// Library entries for the affixes and reduplication, best first.
    pub grammar: Vec<Candidate>,
    /// Verbs only: the sentence's time words, in order.
    pub aspect: Vec<AspectWord>,
}

impl WordDetail {
    /// Nothing beyond the bare word to show.
    pub fn is_empty(&self) -> bool {
        self.parts.len() <= 1 && self.redup.is_none() && self.aspect.is_empty() && self.grammar.is_empty()
    }
}

/// Most affix entries one word links to.
const MAX_AFFIX_ENTRIES: usize = 4;

// ---------------------------------------------------------------- classes

/// Indonesian words whose class the affixes would get wrong (mengapa is
/// "why", not a verb) or that are function words anyway. Lowercase.
#[rustfmt::skip]
const ID_FUNCTION: &[&str] = &[
    "yang", "dan", "atau", "tetapi", "tapi", "namun", "serta", "sedangkan", "sehingga", "karena", "karna",
    "sebab", "kalau", "jika", "jikalau", "apabila", "bila", "agar", "supaya", "bahwa", "meskipun",
    "walaupun", "biarpun", "ketika", "saat", "sambil", "selagi", "sementara", "di", "ke", "dari", "pada",
    "kepada", "daripada", "untuk", "bagi", "buat", "oleh", "dengan", "dalam", "antara", "tentang",
    "mengenai", "terhadap", "menurut", "melalui", "sampai", "hingga", "sejak", "tanpa", "seperti",
    "sebagai", "sesudah", "setelah", "sebelum", "selama", "bersama", "per", "ini", "itu", "tersebut",
    "para", "si", "sang", "sebuah", "seorang", "sesuatu", "setiap", "tiap", "semua", "segala", "seluruh",
    "berbagai", "beberapa", "banyak", "sedikit", "saya", "aku", "kamu", "engkau", "anda", "dia", "ia",
    "beliau", "kami", "kita", "mereka", "gue", "gua", "gw", "lo", "lu", "elo", "sendiri", "apa", "siapa",
    "mana", "di mana", "bagaimana", "gimana", "mengapa", "kenapa", "kapan", "berapa", "apakah", "sudah",
    "udah", "telah", "belum", "sedang", "lagi", "akan", "bakal", "mau", "masih", "pernah", "tidak", "nggak",
    "gak", "enggak", "ga", "tak", "bukan", "jangan", "ya", "tidaklah", "adalah", "ialah", "merupakan",
    "juga", "saja", "aja", "hanya", "cuma", "cuman", "pun", "lah", "kah", "dong", "kok", "sih", "deh",
    "kan", "nih", "tuh", "loh", "lho", "kek", "sangat", "banget", "amat", "paling", "lebih", "kurang",
    "cukup", "terlalu", "sekali", "agak", "terus", "lalu", "kemudian", "selalu", "sering", "kadang",
    "jarang", "sekarang", "nanti", "tadi", "kemarin", "besok", "segera", "terutama", "bahkan", "malah",
    "justru", "hampir", "tentu", "mungkin", "pasti", "harus", "boleh", "bisa", "dapat", "perlu", "ada",
];

/// Javanese function words (ngoko and krama). Lowercase.
#[rustfmt::skip]
const JV_FUNCTION: &[&str] = &[
    "aku", "kowe", "koe", "koen", "dheweke", "awakdhewe", "sampeyan", "panjenengan", "kula", "dalem",
    "piyambakipun", "iki", "iku", "kuwi", "kae", "niki", "niku", "punika", "menika", "sing", "ingkang",
    "lan", "karo", "kaliyan", "ing", "neng", "nang", "menyang", "dhateng", "saka", "seka", "saking",
    "kanggo", "kangge", "marang", "dening", "kanthi", "tumrap", "kaya", "kados", "ora", "gak", "nggak",
    "mboten", "boten", "dudu", "sanes", "durung", "dereng", "wis", "wes", "uwis", "sampun", "lagi", "nembe",
    "arep", "ape", "bakal", "badhe", "isih", "taksih", "uga", "ugi", "mung", "namung", "wae", "ae", "mawon",
    "kemawon", "banget", "sanget", "tenan", "yen", "nek", "menawa", "menawi", "amarga", "merga", "mergo",
    "amargi", "nanging", "ning", "utawa", "utawi", "supaya", "supados", "apa", "napa", "sapa", "sinten",
    "ngendi", "endi", "pundi", "piye", "kepiye", "pripun", "kapan", "pira", "piro", "pinten", "rek", "lho",
    "kok", "ta", "to", "ya", "yo", "nggih", "inggih", "dhewe", "kabeh", "sedaya", "saben", "tak", "dak",
    "sek", "dhisik", "disik", "jare", "lha", "wong",
];

/// The class a gloss's part of speech names. Pronouns, prepositions,
/// particles and the like are [`WordClass::Function`]; names have no class.
pub fn class_of_pos(pos: &str) -> Option<WordClass> {
    let p = pos.trim().to_lowercase().replace(' ', "_");
    match p.as_str() {
        "verb" => Some(WordClass::Verb),
        "noun" | "classifier" | "numeral" => Some(WordClass::Noun),
        "adjective" | "adj" => Some(WordClass::Adjective),
        "adverb" | "pronoun" | "preposition" | "conjunction" | "particle" | "determiner" | "interjection" => {
            Some(WordClass::Function)
        }
        "proper_noun" | "phrase" => None,
        _ if p.contains("adverb") || p.contains("pronoun") => Some(WordClass::Function),
        _ if p.contains("verb") => Some(WordClass::Verb),
        _ if p.contains("noun") && !p.contains("proper") => Some(WordClass::Noun),
        _ if p.starts_with("adj") => Some(WordClass::Adjective),
        _ => None,
    }
}

/// The class the affixes mark; `None` when they don't decide it.
fn class_of_morph(morph: &Morph) -> Option<WordClass> {
    match morph {
        Morph::Indonesian(a) => {
            if let Some(c) = &a.confix {
                return affix_note(Lang::Indonesian, &c.label).and_then(|n| n.makes);
            }
            let first = a.prefixes.first().map(|p| p.label.as_str());
            let suffix = a.suffix.as_ref().map(|s| s.label.as_str());
            match (first, suffix) {
                (Some(p), _) => affix_note(Lang::Indonesian, p).and_then(|n| n.makes),
                (None, Some(s)) => affix_note(Lang::Indonesian, s).and_then(|n| n.makes),
                _ => None,
            }
        }
        Morph::Javanese(a) => {
            if let Some(p) = &a.prefix {
                return affix_note(Lang::Javanese, &p.label).and_then(|n| n.makes);
            }
            // -e / -ne alone mark a possessed or definite noun (omahe).
            a.suffixes.first().and_then(|s| affix_note(Lang::Javanese, &s.label)).and_then(|n| n.makes)
        }
    }
}

fn is_function_word(form: &str, lang: Lang) -> bool {
    match lang {
        Lang::Indonesian => ID_FUNCTION.contains(&form),
        Lang::Javanese => JV_FUNCTION.contains(&form),
        _ => false,
    }
}

/// The word's class, for its colour. Known function words come first (so
/// `mengapa` "why" and `tersebut` "that" aren't taken for verbs), then the
/// affixes, then `pos`, the dictionary's part of speech for the word or its
/// root. ter-/se- on an adjective stay adjectives (terbesar, sebesar).
/// Names, English asides, numbers and punctuation have none.
pub fn class_of_token(t: &AnalyzedToken, pos: Option<&str>) -> Option<WordClass> {
    class_in(t, t.lang.lang, pos)
}

/// [`class_of_token`] when the caller knows the language better than the
/// token's guess (the Reader tags a word by its dictionary entry).
pub fn class_in(t: &AnalyzedToken, lang: Lang, pos: Option<&str>) -> Option<WordClass> {
    if !matches!(lang, Lang::Indonesian | Lang::Javanese) || !t.token.is_wordlike() {
        return None;
    }
    let form = t.lookup_form();
    if is_function_word(&form, lang) || is_function_word(&t.token.lower(), lang) {
        return Some(WordClass::Function);
    }
    let from_pos = pos.and_then(class_of_pos);
    if from_pos == Some(WordClass::Function) {
        return from_pos;
    }
    let from_affix = t.morph.as_ref().and_then(class_of_morph);
    let adjective_keeping = match &t.morph {
        Some(Morph::Indonesian(a)) => {
            a.confix.is_none() && matches!(a.prefixes.first().map(|p| p.label.as_str()), Some("ter-" | "se-"))
        }
        Some(Morph::Javanese(a)) => {
            matches!(a.prefix.as_ref().map(|p| p.label.as_str()), Some("ke-" | "sa-"))
        }
        None => false,
    };
    match (from_affix, from_pos) {
        (_, Some(WordClass::Adjective)) if adjective_keeping => Some(WordClass::Adjective),
        (Some(c), _) => Some(c),
        (None, p) => p,
    }
}

// ---------------------------------------------------------------- parts

fn part(kind: PartKind, form: &str, label: &str) -> Part {
    Part { kind, form: form.to_string(), label: label.to_string() }
}

const JV_CLITICS: &[&str] = &["-e", "-ne", "-ipun", "-mu", "-ku"];

/// Root, pieces in reading order, sound change, root known to the lexicon.
fn parts_of(morph: &Morph) -> (String, Vec<Part>, Option<String>, bool) {
    match morph {
        Morph::Indonesian(a) => {
            let mut parts = Vec::new();
            if let Some(c) = &a.confix {
                // "ke…an": the opening half before the prefixes, the closing after.
                let open = c.surface.split('…').next().unwrap_or_default();
                parts.push(part(PartKind::Confix, open, &c.label));
            }
            parts.extend(a.prefixes.iter().map(|p| part(PartKind::Prefix, &p.surface, &p.label)));
            parts.push(part(PartKind::Root, &a.root, &a.root));
            if let Some(c) = &a.confix {
                let close = c.surface.split('…').nth(1).unwrap_or_default();
                parts.push(part(PartKind::Confix, close, &c.label));
            }
            parts.extend(a.suffix.iter().map(|s| part(PartKind::Suffix, &s.surface, &s.label)));
            parts.extend(a.clitic.iter().map(|c| part(PartKind::Clitic, &c.surface, &c.label)));
            let change = a.nasal_replaced.then(|| {
                let first = a.root.chars().next().unwrap_or_default();
                let nasal = match first {
                    'p' => "m",
                    't' => "n",
                    's' => "ny",
                    _ => "ng",
                };
                format!("{first}→{nasal}")
            });
            (a.root.clone(), parts, change, true)
        }
        Morph::Javanese(a) => {
            let mut parts = Vec::new();
            parts.extend(a.prefix.iter().map(|p| part(PartKind::Prefix, &p.surface, &p.label)));
            parts.push(part(PartKind::Root, &a.root, &a.root));
            for s in &a.suffixes {
                let kind =
                    if JV_CLITICS.contains(&s.label.as_str()) { PartKind::Clitic } else { PartKind::Suffix };
                parts.push(part(kind, &s.surface, &s.label));
            }
            (a.root.clone(), parts, a.sound_change.clone(), a.verified)
        }
    }
}

/// The word's pieces in reading order: `dibersihkan` → di- · bersih · -kan.
/// A word without affixes is one root piece; `kuasa-Mu` ends in a clitic.
pub fn parts_of_token(t: &AnalyzedToken) -> Vec<Part> {
    if let Some(m) = &t.morph {
        return parts_of(m).1;
    }
    let lower = t.token.lower();
    if let Some(c) = &t.token.hyphen_clitic
        && let Some((base, _)) = lower.rsplit_once('-')
    {
        return vec![part(PartKind::Root, base, base), part(PartKind::Clitic, c, &format!("-{c}"))];
    }
    let root = t.token.redup.as_ref().map_or_else(|| t.lookup_form(), |(_, base)| base.clone());
    vec![part(PartKind::Root, &root, &root)]
}

/// Byte ranges of `surface` (the word as written) and the piece each one is,
/// for colouring affixes inside the text: `Dibersihkan` → 0..2 prefix,
/// 2..8 root, 8..11 suffix. A changed root keeps the written letters
/// (`men` + `ulis`); a shortened chat form keeps the pieces it shows
/// (`dicul` → di + cul). Other spellings (arp for arep) and doubled words
/// are one root range.
pub fn surface_spans(surface: &str, t: &AnalyzedToken) -> Vec<(Range<usize>, PartKind)> {
    let whole = vec![(0..surface.len(), PartKind::Root)];
    let lower = surface.to_lowercase();
    if lower.len() != surface.len() || surface.is_empty() {
        return whole;
    }
    if let Some(c) = &t.token.hyphen_clitic
        && t.morph.is_none()
        && let Some(i) = lower.rfind('-')
        && lower[i + 1..] == *c
    {
        return vec![(0..i, PartKind::Root), (i..surface.len(), PartKind::Clitic)];
    }
    let Some(morph) = &t.morph else { return whole };
    // A shortened chat form (dicul for diculke) keeps the pieces it shows.
    let form = t.lookup_form();
    if t.token.redup.is_some() || !form.starts_with(&lower) {
        return whole;
    }
    let parts = parts_of(morph).1;
    let Some(root_at) = parts.iter().position(|p| p.kind == PartKind::Root) else { return whole };
    let (before, after) = (&parts[..root_at], &parts[root_at + 1..]);

    let mut out = Vec::new();
    let mut at = 0;
    for p in before {
        if !form[at..].starts_with(&p.form) {
            return whole;
        }
        out.push((at..at + p.form.len(), p.kind));
        at += p.form.len();
    }
    let tail: usize = after.iter().map(|p| p.form.len()).sum();
    if at + tail >= form.len() {
        return whole;
    }
    let root_end = form.len() - tail;
    out.push((at..root_end, PartKind::Root));
    let mut at = root_end;
    for p in after {
        if !form[at..].starts_with(&p.form) {
            return whole;
        }
        out.push((at..at + p.form.len(), p.kind));
        at += p.form.len();
    }
    let len = surface.len();
    for (r, _) in &mut out {
        *r = r.start.min(len)..r.end.min(len);
    }
    out.retain(|(r, _)| !r.is_empty());
    if out.iter().any(|(r, _)| !surface.is_char_boundary(r.start) || !surface.is_char_boundary(r.end)) {
        return whole;
    }
    out
}

// ---------------------------------------------------------------- notes

const fn note(
    en: &'static str,
    ar: &'static str,
    arz: &'static str,
    makes: Option<WordClass>,
) -> (Label, Option<WordClass>) {
    (Label { en, ar, arz }, makes)
}

const V: Option<WordClass> = Some(WordClass::Verb);
const N: Option<WordClass> = Some(WordClass::Noun);

fn id_note(label: &str) -> Option<(Label, Option<WordClass>)> {
    Some(match label {
        "meN-" => note(
            "active verb: the subject does it",
            "فعل مبني للمعلوم: الفاعل يقوم بالفعل",
            "فعل عادي: الفاعل هو اللي بيعمله",
            V,
        ),
        "di-" => note(
            "passive: it is done (to the subject)",
            "مبني للمجهول: وقع الفعل على الفاعل",
            "مبني للمجهول: الفعل اتعمل في الحاجة",
            V,
        ),
        "ber-" => note(
            "verb of having, doing or being (no object)",
            "فعل لازم: يملك أو يفعل أو يكون في حالة",
            "فعل من غير مفعول: عنده، أو بيعمل، أو في حالة",
            V,
        ),
        "ter-" => note(
            "happened by itself or by accident; with adjectives, 'the most'",
            "حدث من تلقاء نفسه أو دون قصد؛ ومع الصفة: الأكثر",
            "حصل لوحده أو من غير قصد؛ ومع الصفة معناها «الأكتر»",
            V,
        ),
        "per-" => note("make (it) more, turn into", "يجعله أكثر، أو يحوّله إلى", "يخلّيه أكتر، أو يحوّله لـ", V),
        "peN-" => note(
            "the person or tool that does it",
            "الفاعل أو الأداة",
            "اللي بيعمل الحاجة أو الأداة اللي بتعملها",
            N,
        ),
        "ke-" => note(
            "ordinal (kedua 'second') or 'all … together' (ketiga 'all three')",
            "عدد ترتيبي (kedua: الثاني) أو المجموعة كلها",
            "ترتيب (kedua: التاني) أو «التلاتة كلهم»",
            None,
        ),
        "se-" => note(
            "one; the same; as … as",
            "واحد، نفس، أو «مثل» في المقارنة",
            "واحد، نفس، أو «قد» في المقارنة (sebesar: قد كده في الكبر)",
            None,
        ),
        "kau-" => note("you do it to it (passive with 'you')", "أنت تفعله به", "إنت اللي بتعمله فيه", V),
        "ku-" => note("I do it to it (passive with 'I')", "أنا أفعله به", "أنا اللي بعمله فيه", V),
        "-kan" => note(
            "makes it happen, or does it for someone",
            "يجعله يحدث، أو يفعله لأجل أحد",
            "يخلّي الحاجة تحصل، أو يعمله لحد",
            V,
        ),
        "-i" => note(
            "does it to or at something, or again and again",
            "يوقع الفعل على الشيء أو مكانه، أو يكرره",
            "يعمل الفعل على الحاجة أو في مكانها، أو يكرره",
            V,
        ),
        "-an" => {
            note("the thing or the result", "الشيء أو النتيجة (اسم)", "الحاجة نفسها أو النتيجة (اسم)", N)
        }
        "ke-an" => note(
            "an idea or state (like Arabic ـية), or being hit by something",
            "اسم معنى (مثل ـية)، أو التعرّض لشيء",
            "اسم معنى (زي ـية)، أو إن حاجة حصلتلك",
            N,
        ),
        "peN-an" => {
            note("the act or process of doing it", "عملية الفعل (مصدر)", "عملية الفعل نفسها (مصدر)", N)
        }
        "per-an" => note("the field or matter of", "المجال أو الشأن", "المجال أو الموضوع", N),
        "ber-an" => note(
            "many doing it, or doing it to each other",
            "يفعله كثيرون، أو يتبادلونه",
            "ناس كتير بيعملوه، أو بيعملوه مع بعض",
            V,
        ),
        "-nya" => note(
            "his / her / its, or 'the' (that one)",
            "ـه / ـها، أو أداة تعريف",
            "بتاعه / بتاعها، أو بمعنى «الـ»",
            None,
        ),
        "-ku" => note("my", "ـي (ملكي)", "بتاعي", None),
        "-mu" => note("your", "ـك (ملكك)", "بتاعك", None),
        "-lah" => note(
            "softens or stresses (do it!, it's …)",
            "للتأكيد أو تلطيف الأمر",
            "للتأكيد أو عشان الأمر يبقى ألطف",
            None,
        ),
        "-kah" => note("marks the question", "علامة سؤال", "علامة إن ده سؤال", None),
        "-pun" => note("even, also", "حتى، أيضًا", "حتى، كمان", None),
        "-tah" => note("I wonder (old-fashioned question)", "ترى؟ (سؤال أدبي قديم)", "يا ترى؟ (قديمة)", None),
        _ => return None,
    })
}

fn jv_note(label: &str) -> Option<(Label, Option<WordClass>)> {
    Some(match label {
        "N-" => note(
            "active verb (like Indonesian meN-)",
            "فعل مبني للمعلوم (مثل meN- الإندونيسية)",
            "فعل عادي (زي meN- في الإندونيسي)",
            V,
        ),
        "di-" => note("passive: it is done", "مبني للمجهول", "مبني للمجهول", V),
        "dipun-" => note(
            "passive, polite (krama)",
            "مبني للمجهول بصيغة مهذبة (كراما)",
            "مبني للمجهول بالأدب (كراما)",
            V,
        ),
        "ka-" => note("passive, formal", "مبني للمجهول، رسمي", "مبني للمجهول، رسمي", V),
        "ke-" => {
            note("happened by accident (like ter-)", "حدث دون قصد (مثل ter-)", "حصل من غير قصد (زي ter-)", V)
        }
        "sa-" => note("one; as … as", "واحد، أو «مثل» في المقارنة", "واحد، أو «قد» في المقارنة", None),
        "tak-" => note("I do it (or: let me)", "أنا أفعله (أو: دعني)", "أنا اللي بعمله (أو: سيبني أعمله)", V),
        "kok-" => note("you do it", "أنت تفعله", "إنت اللي بتعمله", V),
        "-ake" | "-aken" | "-ke" => note(
            "makes it happen, or does it for someone (like -kan)",
            "يجعله يحدث أو لأجل أحد (مثل -kan)",
            "يخلّيه يحصل أو يعمله لحد (زي -kan)",
            V,
        ),
        "-i" => note(
            "does it to or at something (like -i)",
            "يوقعه على الشيء (مثل -i)",
            "يعمله على الحاجة (زي -i)",
            V,
        ),
        "-ana" => note("do it to it! (command of -i)", "أمر من صيغة -i", "أمر: اعمله فيه!", V),
        "-en" => note(
            "do it! (command), or 'too …'",
            "أمر، أو «زيادة عن اللزوم»",
            "أمر: اعمله!، أو «زيادة عن اللزوم»",
            V,
        ),
        "-a" => note("command, or 'even if'", "أمر، أو «حتى لو»", "أمر، أو «حتى لو»", V),
        "-an" => note(
            "the thing, or doing it to each other",
            "الشيء، أو التبادل",
            "الحاجة نفسها، أو بيعملوه مع بعض",
            N,
        ),
        "-e" | "-ne" => note(
            "his / her / its, or 'the' (like -nya)",
            "ـه / ـها، أو أداة تعريف (مثل -nya)",
            "بتاعه، أو بمعنى «الـ» (زي -nya)",
            N,
        ),
        "-ipun" => note(
            "'the' / his, polite (krama of -e)",
            "أداة تعريف مهذبة (كراما لـ -e)",
            "«الـ» بالأدب (كراما بتاعة -e)",
            N,
        ),
        "-mu" => note("your", "ـك", "بتاعك", None),
        "-ku" => note("my", "ـي", "بتاعي", None),
        _ => return None,
    })
}

/// What an affix (by its canonical label, "meN-", "-kan", "ke-an") does.
pub fn affix_note(lang: Lang, label: &str) -> Option<AffixNote> {
    let (meaning, makes) = match lang {
        Lang::Javanese => jv_note(label)?,
        _ => id_note(label)?,
    };
    Some(AffixNote { meaning, makes, verified: lang != Lang::Javanese })
}

/// What a doubled word means.
pub fn redup_note(kind: Reduplication, class: Option<WordClass>) -> Label {
    match (kind, class) {
        (Reduplication::Imitative, _) => Label {
            en: "echo pair: all kinds of …",
            ar: "تكرار بتغيير صوت: كل أنواع …",
            arz: "تكرار بصوت متغيّر: كل أنواع …",
        },
        (_, Some(WordClass::Verb)) => Label {
            en: "doubled verb: again and again, or casually",
            ar: "فعل مكرر: مرارًا، أو بلا هدف محدد",
            arz: "فعل متكرر: كذا مرة، أو على راحتك",
        },
        (_, Some(WordClass::Adjective)) => Label {
            en: "doubled adjective: very, or all of them are",
            ar: "صفة مكررة: جدًا، أو كلهم كذلك",
            arz: "صفة متكررة: أوي، أو كلهم كده",
        },
        _ => Label {
            en: "doubled: more than one, or all kinds",
            ar: "تكرار: جمع أو تنوّع",
            arz: "تكرار: جمع أو أنواع كتير",
        },
    }
}

// ---------------------------------------------------------------- library

/// Reduplication isn't spelled as an affix in the library: these entries name it.
fn redup_entries(lang: Lang, kind: Reduplication, class: Option<WordClass>) -> &'static [&'static str] {
    match (lang, kind) {
        (Lang::Javanese, _) => &["jv.reduplication"],
        (_, Reduplication::Full) if class != Some(WordClass::Verb) => {
            &["id.reduplication.plural", "id.reduplication.types"]
        }
        (_, Reduplication::Prefixed) => &["id.reduplication.types", "id.reduplication.ber-an"],
        _ => &["id.reduplication.types"],
    }
}

impl Library {
    /// Entries spelled as affixes this word carries (`meN-…-kan`, `-nya`),
    /// in `lang`'s library only, best first.
    pub fn affix_entries(&self, word: &AnalyzedToken, lang: Lang, limit: usize) -> Vec<Candidate> {
        let mut have = word_affixes(word);
        have.remove("red");
        if have.is_empty() {
            return Vec::new();
        }
        let want = if lang == Lang::Javanese { "jv." } else { "id." };
        let mut out: Vec<Candidate> = Vec::new();
        for (id, cues) in &self.cues {
            if !id.starts_with(want) {
                continue;
            }
            let mut score = 0;
            let mut matched = Vec::new();
            for set in &cues.affixes {
                if set.iter().all(|a| have.contains(a)) {
                    let key = set.join("…");
                    // A whole confix outranks one of its halves; a namesake most.
                    score += weight(self.df(&format!("a:{key}"))) + 3 * set.len() as u32;
                    if cues.namesake.contains(&key) {
                        score += 8;
                    }
                    matched.push(key);
                }
            }
            if score > 0 {
                matched.dedup();
                out.push(Candidate { id: id.clone(), score, matched });
            }
        }
        out.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| self.points[&a.id].level.cmp(&self.points[&b.id].level))
                .then_with(|| a.id.cmp(&b.id))
        });
        out.truncate(limit);
        out
    }

    /// Single words the library's aspect entries are about, with the entry
    /// (`sudah` → `id.aspect.markers`).
    pub fn aspect_words(&self, lang: Lang) -> Vec<(String, String)> {
        let want = if lang == Lang::Javanese { "jv" } else { "id" };
        let mut out = Vec::new();
        for (id, cues) in &self.cues {
            let mut parts = id.split('.');
            if parts.next() != Some(want) || parts.next() != Some("aspect") {
                continue;
            }
            out.extend(cues.words.iter().map(|w| (w.clone(), id.clone())));
        }
        out
    }

    /// Everything the word card's breakdown shows. `sentence` is the
    /// analyzed sentence around the word (for time words); `pos` is the
    /// dictionary's part of speech for the word or its root.
    pub fn word_detail(
        &self,
        word: &AnalyzedToken,
        sentence: &[AnalyzedToken],
        lang: Lang,
        pos: Option<&str>,
    ) -> WordDetail {
        let redup = word.token.redup.clone();
        let (root, parts, sound_change, root_verified) = match &word.morph {
            Some(m) => parts_of(m),
            None => {
                let parts = parts_of_token(word);
                let root = parts.first().map_or_else(|| word.lookup_form(), |p| p.form.clone());
                (root, parts, None, true)
            }
        };
        let class = class_of_token(word, pos);

        let mut grammar = self.affix_entries(word, lang, MAX_AFFIX_ENTRIES);
        if let Some((kind, _)) = &redup {
            for id in redup_entries(lang, *kind, class) {
                if self.points.contains_key(*id) && !grammar.iter().any(|c| c.id == *id) {
                    grammar.push(Candidate {
                        id: id.to_string(),
                        score: 0,
                        matched: vec![word.token.lower()],
                    });
                }
            }
        }

        let aspect = if class == Some(WordClass::Verb) {
            let known = self.aspect_words(lang);
            let mut seen = BTreeSet::new();
            sentence
                .iter()
                .filter(|t| t.token.is_wordlike() && t.token.start != word.token.start)
                .filter_map(|t| {
                    let form = t.lookup_form();
                    let (w, id) = known.iter().find(|(w, _)| *w == form || *w == t.token.lower())?;
                    seen.insert(w.clone()).then(|| AspectWord {
                        surface: t.token.text.clone(),
                        form: w.clone(),
                        grammar_id: id.clone(),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        WordDetail { root, parts, sound_change, redup, class, root_verified, grammar, aspect }
    }

    /// Entries the sentence's word cues point at through this one word
    /// ("yang", "sudah", "tidak ada"): shown on its card.
    pub fn word_entries(&self, sentence: &[Candidate], word: &AnalyzedToken) -> Vec<Candidate> {
        let mut forms = vec![word.token.lower(), word.lookup_form()];
        let affixes = word_affixes(word);
        forms.extend(affixes.iter().cloned());
        sentence
            .iter()
            .filter(|c| {
                c.matched.iter().any(|m| {
                    forms.contains(m)
                        || m.split(' ').any(|piece| forms.contains(&piece.to_string()))
                        || m.split('…').all(|a| affixes.contains(a))
                })
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests;
