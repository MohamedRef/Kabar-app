//! "Explain this sentence" (spec §11, §15): picks grammar-library entries for
//! a sentence on the device, then (when an engine is connected) asks the
//! `explain_sentence` task to choose among them and write the glue text.
//!
//! The library is data passed in (the app embeds `content/grammar`), so this
//! stays pure. Picking is deterministic: each entry's cues come from its
//! target forms, patterns and id (words like `sudah`, affixes like `meN-…-kan`),
//! and are matched against the analyzed sentence (surface words, normalized
//! forms, and the affixes the morphology engine found).

use std::collections::{BTreeMap, BTreeSet};

use kabar_protocol::grammar::GrammarPoint;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::lang::Lang;
use crate::lang::analyze::{AnalyzedToken, Morph};

/// Most candidates offered to the model (the golden set offers 4–6; a few
/// more cost little and catch what the cues rank lower).
pub const MAX_CANDIDATES: usize = 8;

/// What makes an entry relevant to a sentence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Cues {
    /// Single lowercase words ("sudah", "nya" is an affix, not here).
    words: BTreeSet<String>,
    /// Multi-word phrases, lowercase ("tidak ada", "oleh karena itu").
    phrases: Vec<Vec<String>>,
    /// Affix sets that must all be on one word, canonical lowercase
    /// ("men-", "-kan"), e.g. one set for `meN-…-kan`.
    affixes: Vec<Vec<String>>,
    /// Words and affixes the entry is named after (`id.demonstrative.ini-itu`,
    /// `id.prefix.ber.basic`): a match on these counts extra.
    namesake: BTreeSet<String>,
    /// A chat-spelling entry (`id.chat.*`): fits any sentence with
    /// abbreviations the normalizer expanded.
    chat: bool,
}

/// Id segments that describe, rather than name, a form.
const ID_LABELS: &[&str] = &[
    "basic",
    "types",
    "markers",
    "overview",
    "meanings",
    "words",
    "terms",
    "phrases",
    "expressions",
    "plural",
    "personal",
    "result",
    "agent",
    "causative",
    "advanced",
    "normalize",
    "vs",
    "indonesian",
    "ngoko",
    "krama",
    "number",
    "numbers",
    "question",
    "questions",
    "time",
    "aspect",
    "prefix",
    "suffix",
    "confix",
    "clitic",
    "proclitic",
    "particle",
    "pronoun",
    "preposition",
    "conjunction",
    "demonstrative",
    "negation",
    "chat",
];

/// The grammar library, indexed for matching.
#[derive(Clone, Debug, Default)]
pub struct Library {
    points: BTreeMap<String, GrammarPoint>,
    cues: BTreeMap<String, Cues>,
    /// How many entries share each cue: a cue on one entry says much more
    /// than `di` or `-nya`, which half the library mentions.
    df: BTreeMap<String, u32>,
}

/// Weight of a cue shared by `df` entries.
fn weight(df: u32) -> u32 {
    (12 / df.max(1)).max(1)
}

/// One entry that fits the sentence, with the words that matched it.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub score: u32,
    /// What matched, for the UI ("sedang", "meN-").
    pub matched: Vec<String>,
}

/// Placeholder words in patterns, not cues ("Subject + sudah + Verb").
const PLACEHOLDERS: &[&str] = &[
    "subject",
    "verb",
    "noun",
    "adjective",
    "adj",
    "object",
    "place",
    "time",
    "number",
    "name",
    "person",
    "something",
    "someone",
    "clause",
    "sentence",
    "word",
    "root",
    "base",
    "x",
    "y",
    "n",
    "v",
    "s",
    "o",
    "predicate",
    "phrase",
    "question",
    "answer",
    "title",
    "adverb",
    "amount",
    "unit",
    "date",
    "day",
    "month",
    "year",
    "pronoun",
    "possessor",
    "thing",
    "action",
    "doer",
    "agent",
    "patient",
    "quantity",
];

/// English filler that shows up in patterns.
const STOP: &[&str] = &["a", "the", "or", "of", "to", "and", "is", "in", "on", "with", "for", "not"];

fn is_latin_word(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| c.is_ascii_alphabetic() || c == '\'')
}

/// "meN-" → "men-"; "-nya" stays; "ke-…-an" → ["ke-", "-an"].
fn affix_set(form: &str) -> Option<Vec<String>> {
    let f = form.trim().to_lowercase().replace(['…', '.'], "");
    if !f.contains('-') || f.starts_with('-') && f.len() < 2 {
        return None;
    }
    let mut out = Vec::new();
    let bytes: Vec<&str> = f.split('-').collect();
    // "men-kan" (after dropping …) = prefix "men-" + suffix "-kan".
    match bytes.as_slice() {
        ["", s] if is_latin_word(s) => out.push(format!("-{s}")),
        [p, ""] if is_latin_word(p) => out.push(format!("{p}-")),
        [p, "", s] if is_latin_word(p) && is_latin_word(s) => {
            out.push(format!("{p}-"));
            out.push(format!("-{s}"));
        }
        [p, s] if is_latin_word(p) && is_latin_word(s) && p != s && p.len() <= 4 && s.len() <= 4 => {
            out.push(format!("{p}-"));
            out.push(format!("-{s}"));
        }
        _ => return None,
    }
    Some(out)
}

/// The nasal prefix is written meN-/men-/mem-/meng-… and peN- likewise.
fn canonical_affix(a: &str) -> String {
    let a = a.to_lowercase();
    match a.as_str() {
        "me-" | "mem-" | "meng-" | "menge-" | "meny-" | "men-" => "men-".into(),
        "pe-" | "pem-" | "peng-" | "penge-" | "peny-" | "pen-" => "pen-".into(),
        "be-" | "bel-" | "ber-" => "ber-".into(),
        "te-" | "ter-" => "ter-".into(),
        "pel-" | "per-" => "per-".into(),
        _ => a,
    }
}

impl Cues {
    /// One key per cue, for counting how many entries share it.
    fn keys(&self) -> Vec<String> {
        let mut out: Vec<String> = self.words.iter().map(|w| format!("w:{w}")).collect();
        out.extend(self.phrases.iter().map(|p| format!("p:{}", p.join(" "))));
        out.extend(self.affixes.iter().map(|a| format!("a:{}", a.join("…"))));
        if self.chat {
            out.push(CHAT_KEY.to_string());
        }
        out.sort();
        out.dedup();
        out
    }

    fn of(p: &GrammarPoint) -> Self {
        let mut cues = Cues::default();
        let parts: Vec<&str> = p.id.split('.').collect();
        cues.chat = parts.get(1) == Some(&"chat");
        // Aksara entries list syllables ("ha na ca … wa"), not words a Latin
        // sentence could contain: only their id names them.
        let mut sources: Vec<String> = Vec::new();
        if parts.get(1) != Some(&"aksara") {
            sources.push(p.title.target.clone());
            sources.extend(p.pattern.iter().cloned());
        }
        for src in &sources {
            // Drop parenthesized English hints: "(there isn't, none)".
            let mut clean = String::new();
            let mut depth = 0;
            for c in src.chars() {
                match c {
                    '(' => depth += 1,
                    ')' => depth = (depth - 1).max(0),
                    _ if depth == 0 => clean.push(c),
                    _ => {}
                }
            }
            let pieces = clean.split([',', '·', '—', '→', '?', '!', ':', ';', '=', '"', '«', '»']);
            for chunk in pieces.flat_map(expand_alternatives) {
                let chunk = chunk.trim().trim_matches(['…', '.', '\'']).trim();
                if chunk.is_empty() {
                    continue;
                }
                let words: Vec<&str> = chunk.split_whitespace().collect();
                // Affix forms: "meN-…-kan", "-nya", "ber-".
                if words.len() == 1
                    && let Some(set) = affix_set(words[0])
                {
                    cues.affixes.push(set.iter().map(|a| canonical_affix(a)).collect());
                    continue;
                }
                // Placeholders are capitalized English ("Subject", "Verb") or listed.
                let lower: Vec<String> = words
                    .iter()
                    .map(|w| w.trim_matches(['…', '.', '\'', '*']).to_lowercase())
                    .filter(|w| is_latin_word(w) && !PLACEHOLDERS.contains(&w.as_str()))
                    .collect();
                let all_plain = words.iter().all(|w| {
                    let t = w.trim_matches(['…', '.', '\'', '*']);
                    is_latin_word(t) && !PLACEHOLDERS.contains(&t.to_lowercase().as_str())
                });
                match lower.len() {
                    0 => {}
                    1 => {
                        if !STOP.contains(&lower[0].as_str()) {
                            cues.words.insert(lower[0].clone());
                        }
                    }
                    2..=4 if all_plain => cues.phrases.push(lower),
                    _ => {
                        // A longer formula: keep its distinctive words.
                        for w in lower {
                            if w.len() > 2 && !STOP.contains(&w.as_str()) {
                                cues.words.insert(w);
                            }
                        }
                    }
                }
            }
        }
        // The id names the point too: id.clitic.nya, id.prefix.ber.basic,
        // id.demonstrative.ini-itu, jv.aspect.wis-durung-lagi-arep.
        if !matches!(parts.get(1), Some(&("prefix" | "suffix" | "confix" | "clitic" | "reduplication"))) {
            for seg in parts.iter().skip(1) {
                for w in seg.split('-') {
                    if w.len() >= 2 && is_latin_word(w) && !ID_LABELS.contains(&w) {
                        cues.namesake.insert(w.to_string());
                        cues.words.insert(w.to_string());
                    }
                }
            }
        }
        if parts.len() >= 3 && matches!(parts[1], "prefix" | "suffix" | "confix" | "clitic") {
            let seg = parts[2];
            let set = match parts[1] {
                "prefix" => vec![canonical_affix(&format!("{seg}-"))],
                "suffix" | "clitic" => vec![format!("-{seg}")],
                _ => {
                    affix_set(seg).map(|s| s.iter().map(|a| canonical_affix(a)).collect()).unwrap_or_default()
                }
            };
            if !set.is_empty() {
                cues.namesake.insert(set.join("…"));
                if !cues.affixes.contains(&set) {
                    cues.affixes.push(set);
                }
            }
        }
        cues
    }
}

/// The df key of the chat-spelling cue.
const CHAT_KEY: &str = "s:chat";

/// Splits "selamat + datang / jalan" into the phrases it stands for
/// ("selamat datang", "selamat jalan"), so "jalan" alone isn't a cue.
/// Anything else ("meN- + root + -kan", "gk / ga") splits into its pieces.
fn expand_alternatives(piece: &str) -> Vec<String> {
    let parts: Vec<Vec<&str>> = piece
        .split('+')
        .map(|p| p.split('/').map(str::trim).filter(|a| !a.is_empty()).collect::<Vec<_>>())
        .filter(|alts| !alts.is_empty())
        .collect();
    let plain = |a: &str| {
        a.split_whitespace().all(|w| {
            let t = w.trim_matches(['…', '.', '\'', '*']);
            is_latin_word(t) && !PLACEHOLDERS.contains(&t.to_lowercase().as_str())
        })
    };
    let combos: usize = parts.iter().map(Vec::len).product();
    if parts.len() >= 2 && combos <= 16 && parts.iter().flatten().all(|a| plain(a)) {
        let mut out = vec![String::new()];
        for alts in &parts {
            out = out.iter().flat_map(|head| alts.iter().map(move |a| format!("{head} {a}"))).collect();
        }
        return out.into_iter().map(|p| p.trim_start().to_string()).collect();
    }
    parts.into_iter().flatten().map(str::to_string).collect()
}

/// Affixes on one analyzed word, canonical ("men-", "-kan", "-nya").
fn word_affixes(t: &AnalyzedToken) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    match &t.morph {
        Some(Morph::Indonesian(a)) => {
            for p in &a.prefixes {
                out.insert(canonical_affix(&p.label));
            }
            if let Some(c) = &a.confix {
                // "ke-an" → "ke-", "-an".
                if let Some(set) = affix_set(&c.label) {
                    out.extend(set.iter().map(|x| canonical_affix(x)));
                }
            }
            for s in [&a.suffix, &a.clitic].into_iter().flatten() {
                out.insert(canonical_affix(&s.label));
            }
        }
        Some(Morph::Javanese(a)) => {
            if let Some(p) = &a.prefix {
                out.insert(canonical_affix(&p.label));
            }
            for s in &a.suffixes {
                out.insert(canonical_affix(&s.label));
            }
        }
        None => {}
    }
    if t.token.redup.is_some() {
        out.insert("red".into());
    }
    out
}

impl Library {
    /// Parses grammar files (`(path, yaml)`); a broken file is skipped and
    /// reported, never fatal.
    pub fn from_yaml<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> (Self, Vec<String>) {
        let mut lib = Self::default();
        let mut problems = Vec::new();
        for (path, text) in files {
            match serde_yaml_ng::from_str::<GrammarPoint>(text) {
                Ok(p) => lib.insert(p),
                Err(e) => problems.push(format!("{path}: {e}")),
            }
        }
        (lib, problems)
    }

    pub fn insert(&mut self, p: GrammarPoint) {
        let cues = Cues::of(&p);
        if let Some(old) = self.cues.insert(p.id.clone(), cues.clone()) {
            for k in old.keys() {
                *self.df.entry(k).or_default() -= 1;
            }
        }
        for k in cues.keys() {
            *self.df.entry(k).or_default() += 1;
        }
        self.points.insert(p.id.clone(), p);
    }

    fn df(&self, key: &str) -> u32 {
        self.df.get(key).copied().unwrap_or(1)
    }

    pub fn get(&self, id: &str) -> Option<&GrammarPoint> {
        self.points.get(id)
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Entries that fit the sentence, best first, at most `limit`.
    /// `lang` picks the library (`Lang::Javanese` → `jv` entries first, then
    /// Indonesian parallels).
    pub fn candidates(&self, tokens: &[AnalyzedToken], lang: Lang, limit: usize) -> Vec<Candidate> {
        let words: Vec<(String, String, BTreeSet<String>)> = tokens
            .iter()
            .filter(|t| t.token.is_wordlike())
            .map(|t| {
                // "wa" → "WhatsApp" is a brand, not a grammar cue.
                let norm = if t.lang.lang == Lang::English { t.token.lower() } else { t.lookup_form() };
                (t.token.lower(), norm, word_affixes(t))
            })
            .collect();
        let expanded: Vec<String> =
            tokens.iter().filter(|t| t.normalized.is_some()).map(|t| t.token.lower()).collect();
        let forms: Vec<&str> = words.iter().map(|(_, n, _)| n.as_str()).collect();
        let want = if lang == Lang::Javanese { "jv" } else { "id" };

        let mut out: Vec<Candidate> = Vec::new();
        for (id, cues) in &self.cues {
            let point = &self.points[id];
            let mut score = 0;
            let mut matched: Vec<String> = Vec::new();
            for phrase in &cues.phrases {
                if forms.windows(phrase.len()).any(|w| w.iter().zip(phrase).all(|(a, b)| *a == b)) {
                    let key = phrase.join(" ");
                    score += weight(self.df(&format!("p:{key}"))) + 2 * phrase.len() as u32;
                    matched.push(key);
                }
            }
            for (surface, norm, _) in &words {
                for w in [norm, surface] {
                    if cues.words.contains(w) && !matched.contains(w) {
                        score += weight(self.df(&format!("w:{w}")));
                        if cues.namesake.contains(w) {
                            score += 4;
                        }
                        matched.push(w.clone());
                    }
                }
            }
            for set in &cues.affixes {
                if words.iter().any(|(_, _, have)| set.iter().all(|a| have.contains(a))) {
                    let key = set.join("…");
                    score += weight(self.df(&format!("a:{key}"))) + set.len() as u32;
                    if cues.namesake.contains(&key) {
                        score += 4;
                    }
                    matched.push(key);
                }
            }
            if cues.chat && !expanded.is_empty() {
                score += weight(self.df(CHAT_KEY)) + 2 * expanded.len().min(3) as u32;
                matched.extend(expanded.iter().cloned());
            }
            if score == 0 {
                continue;
            }
            if point.language == want {
                score += 1;
            }
            let mut seen = BTreeSet::new();
            matched.retain(|m| seen.insert(m.clone()));
            out.push(Candidate { id: id.clone(), score, matched });
        }
        // Best first; on ties the easier level, then the id, so it's stable.
        out.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| self.points[&a.id].level.cmp(&self.points[&b.id].level))
                .then_with(|| a.id.cmp(&b.id))
        });
        out.truncate(limit);
        out
    }
}

/// Which explanation voice the answer is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    Arz,
    Ar,
    En,
}

/// The `explain_sentence` input, exactly as the golden set shapes it.
pub fn explain_input(text: &str, lang: Lang, voice: Voice, candidates: &[Candidate]) -> Value {
    json!({
        "text": text,
        "lang": if lang == Lang::Javanese { "jv" } else { "id" },
        "voice": voice,
        "grammar_candidates": candidates.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests;
