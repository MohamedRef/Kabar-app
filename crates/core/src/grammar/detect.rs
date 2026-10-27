//! Spotting grammar points in running text, offline (Reader, spec §11).
//!
//! Each grammar-library entry can carry `cues`: small patterns that say where
//! the point shows up in a sentence, with a short note on what the words do
//! there. `Library::spot` runs them over one analyzed sentence and returns
//! hits as token spans; `Syllabus` says where the learner stands with each
//! point (learned in a lesson, at their level, or still ahead), so the Reader
//! can show what they know first and mark the rest as coming later.
//!
//! The pattern language, kept tiny so the grammar files stay readable:
//! - words separated by spaces must follow each other: `tidak ada`;
//! - `a|b` is one word, either spelling: `sudah|udah`;
//! - `…` skips up to `MAX_GAP` words inside the sentence: `lebih … daripada|dari`;
//! - `{di-}` is a word carrying that affix (as `lang::analyze` found it),
//!   `{men-|-kan}` one carrying both.
//!
//! Words match the surface form or the normalized one, so `gk` meets `tidak`.
//! Chat spellings the normalizer expanded are hits of the language's chat
//! entry (`id.chat.abbreviations`, `jv.chat.normalize`) with the full form.

use std::collections::{BTreeMap, BTreeSet};

use kabar_protocol::grammar::{GrammarCue, GrammarPoint, Three};

use super::word::WordClass;
use super::{Library, canonical_affix, word_affixes};
use crate::journey::{Curriculum, LessonState, Progress};
use crate::lang::Lang;
use crate::lang::analyze::{AnalyzedToken, Morph};

/// Most words a `…` skips.
pub const MAX_GAP: usize = 8;

/// How a hit is coloured: the word class of what matched (the word-colour
/// tokens, `word::WordClass`), or a phrase of several words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Class(WordClass),
    /// Several words that work together (`tidak ada`, `lebih … daripada`).
    Phrase,
}

impl Role {
    pub fn css(self) -> &'static str {
        match self {
            Role::Class(c) => c.css(),
            Role::Phrase => "wc-phrase",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "function" => Role::Class(WordClass::Function),
            "verb" => Role::Class(WordClass::Verb),
            "noun" => Role::Class(WordClass::Noun),
            "adjective" => Role::Class(WordClass::Adjective),
            "phrase" => Role::Phrase,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Slot {
    /// One word, any of these spellings (lowercase).
    Word(Vec<String>),
    /// One word carrying all these affixes (canonical, see `canonical_affix`).
    Affix(Vec<String>),
    /// Up to `MAX_GAP` words.
    Gap,
}

/// A cue, compiled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Pattern {
    slots: Vec<Slot>,
    role: Role,
}

/// Compiles one cue; the error says what's wrong with it.
pub(super) fn compile(cue: &GrammarCue) -> Result<Pattern, String> {
    let mut slots = Vec::new();
    for piece in cue.pattern.split_whitespace() {
        if piece == "…" || piece == "..." {
            if matches!(slots.last(), None | Some(Slot::Gap)) {
                return Err(format!("“{}”: a gap must sit between words", cue.pattern));
            }
            slots.push(Slot::Gap);
        } else if let Some(inner) = piece.strip_prefix('{').and_then(|p| p.strip_suffix('}')) {
            let set: Vec<String> = inner.split('|').map(|a| canonical_affix(a.trim())).collect();
            if set.iter().any(|a| !(a.starts_with('-') || a.ends_with('-') || a == "red") || a.len() < 2) {
                return Err(format!("“{}”: {{…}} holds affixes like di- or -kan", cue.pattern));
            }
            slots.push(Slot::Affix(set));
        } else {
            let alts: Vec<String> = piece.split('|').map(|a| a.trim().to_lowercase()).collect();
            if alts
                .iter()
                .any(|a| a.is_empty() || a.chars().any(|c| !(c.is_alphabetic() || c == '-' || c == '\'')))
            {
                return Err(format!("“{}”: “{piece}” isn't a word", cue.pattern));
            }
            slots.push(Slot::Word(alts));
        }
    }
    if slots.is_empty() {
        return Err("empty cue".into());
    }
    if slots.last() == Some(&Slot::Gap) {
        return Err(format!("“{}”: a gap must sit between words", cue.pattern));
    }
    let words = slots.iter().filter(|s| **s != Slot::Gap).count();
    let role = match cue.class.as_deref() {
        Some(c) => Role::parse(c).ok_or_else(|| format!("“{}”: unknown class {c}", cue.pattern))?,
        None if words > 1 => Role::Phrase,
        None => Role::Class(WordClass::Function),
    };
    Ok(Pattern { slots, role })
}

/// One word of the sentence as the patterns see it.
struct W {
    token: usize,
    surface: String,
    norm: String,
    /// The root under the affixes (`asmanipun` → `asma`).
    root: Option<String>,
    affixes: BTreeSet<String>,
}

impl W {
    /// Content words also match through their affixes (`asmanipun` is
    /// `asma`); function words don't, or `terlalu` would be `lalu`.
    fn is(&self, alts: &[String], role: Role) -> bool {
        let root = self.root.as_ref().filter(|_| role != Role::Class(WordClass::Function));
        alts.iter().any(|a| *a == self.surface || *a == self.norm || Some(a) == root)
    }
}

impl Pattern {
    /// Word positions matched by each non-gap slot, from `start`.
    fn match_at(&self, words: &[W], start: usize) -> Option<Vec<usize>> {
        let role = self.role;
        let go_ =
            |slots: &[Slot], words: &[W], at: usize, out: &mut Vec<usize>| go(slots, words, at, out, role);
        fn go(slots: &[Slot], words: &[W], at: usize, out: &mut Vec<usize>, role: Role) -> bool {
            let Some((first, rest)) = slots.split_first() else { return true };
            match first {
                Slot::Gap => {
                    for skip in 0..=MAX_GAP {
                        if at + skip > words.len() {
                            break;
                        }
                        let mark = out.len();
                        if go(rest, words, at + skip, out, role) {
                            return true;
                        }
                        out.truncate(mark);
                    }
                    false
                }
                Slot::Word(alts) => {
                    let ok = words.get(at).is_some_and(|w| w.is(alts, role));
                    ok && {
                        out.push(at);
                        go(rest, words, at + 1, out, role)
                    }
                }
                Slot::Affix(set) => {
                    let ok = words.get(at).is_some_and(|w| set.iter().all(|a| w.affixes.contains(a)));
                    ok && {
                        out.push(at);
                        go(rest, words, at + 1, out, role)
                    }
                }
            }
        }
        let mut out = Vec::new();
        go_(&self.slots, words, start, &mut out).then_some(out)
    }
}

/// One place a grammar point shows up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub grammar: String,
    /// Which of the entry's cues matched; None for a chat spelling.
    pub cue: Option<usize>,
    /// The caller's token indices of the matched words, in order (a frame
    /// like `lebih … daripada` has gaps between them).
    pub tokens: Vec<usize>,
    pub role: Role,
    /// A chat spelling's full form (`yg` → `yang`).
    pub expands_to: Option<String>,
}

impl Hit {
    /// The matched words, `…` where the frame skips some.
    pub fn label(&self, word: impl Fn(usize) -> String) -> String {
        let mut out = String::new();
        for (i, &t) in self.tokens.iter().enumerate() {
            if i > 0 {
                out.push_str(if t == self.tokens[i - 1] + 1 { " " } else { " … " });
            }
            out.push_str(&word(t));
        }
        out
    }
}

impl Library {
    /// Grammar points in one sentence. `words` are the sentence's word tokens
    /// with the caller's own indices (consecutive words have consecutive
    /// indices when nothing but spaces and punctuation sits between them).
    /// `lang` picks the cues: a Javanese sentence is read with Javanese cues.
    pub fn spot(&self, words: &[(usize, &AnalyzedToken)], lang: Lang) -> Vec<Hit> {
        let want = if lang == Lang::Javanese { "jv" } else { "id" };
        // A normalized form can be several words ("gpp" → "tidak apa-apa").
        let mut ws: Vec<W> = Vec::new();
        for &(i, t) in words {
            let surface = t.token.lower();
            let norm = if t.lang.lang == Lang::English { surface.clone() } else { t.lookup_form() };
            let affixes = word_affixes(t);
            let root = match &t.morph {
                Some(Morph::Indonesian(a)) => Some(a.root.clone()),
                Some(Morph::Javanese(a)) => Some(a.root.clone()),
                None => None,
            };
            let mut parts = norm.split_whitespace();
            let first = parts.next().unwrap_or_default().to_string();
            ws.push(W { token: i, surface: surface.clone(), norm: first, root, affixes });
            for p in parts {
                ws.push(W {
                    token: i,
                    surface: String::new(),
                    norm: p.to_string(),
                    root: None,
                    affixes: BTreeSet::new(),
                });
            }
        }

        let mut hits: Vec<Hit> = Vec::new();
        for (id, patterns) in &self.patterns {
            if self.points[id].language != want {
                continue;
            }
            for (ci, p) in patterns.iter().enumerate() {
                let Some(p) = p else { continue };
                let mut start = 0;
                while start < ws.len() {
                    match p.match_at(&ws, start) {
                        // A word written short for several ("gpp" → "tidak apa-apa")
                        // only counts when the cue covers all of them.
                        Some(at)
                            if at.iter().all(|&w| {
                                ws.iter()
                                    .enumerate()
                                    .filter(|(_, x)| x.token == ws[w].token)
                                    .all(|(j, _)| at.contains(&j))
                            }) =>
                        {
                            let mut tokens: Vec<usize> = at.iter().map(|&w| ws[w].token).collect();
                            tokens.dedup();
                            hits.push(Hit {
                                grammar: id.clone(),
                                cue: Some(ci),
                                tokens,
                                role: p.role,
                                expands_to: None,
                            });
                            start = at.last().map_or(start, |&l| l.max(start)) + 1;
                        }
                        _ => start += 1,
                    }
                }
            }
        }

        let chat = if lang == Lang::Javanese { "jv.chat.normalize" } else { "id.chat.abbreviations" };
        if self.points.contains_key(chat) {
            for &(i, t) in words {
                let Some(n) = &t.normalized else { continue };
                let surface = t.token.lower();
                // Brand and English expansions ("pm" → "Pro Max") aren't chat Indonesian.
                let english = t.lang.lang == Lang::English || n.full.chars().any(char::is_uppercase);
                if n.full.to_lowercase() != surface && !english {
                    hits.push(Hit {
                        grammar: chat.to_string(),
                        cue: None,
                        tokens: vec![i],
                        role: Role::Class(WordClass::Function),
                        expands_to: Some(n.full.clone()),
                    });
                }
            }
        }

        // A longer match covers a shorter one: `tidak ada` covers `ada`, and
        // `bukan hanya … tetapi juga` covers `bukan` and `tetapi`, which mean
        // something else inside the frame. A chat spelling is always kept.
        let covered = |h: &Hit, by: &Hit| {
            h.cue.is_some()
                && by.cue.is_some()
                && by.tokens.len() > h.tokens.len()
                && (by.grammar == h.grammar || h.tokens.len() == 1)
                && h.tokens.iter().all(|t| by.tokens.contains(t))
        };
        let keep: Vec<bool> = hits.iter().map(|h| !hits.iter().any(|o| covered(h, o))).collect();
        let mut out: Vec<Hit> = hits.into_iter().zip(keep).filter_map(|(h, k)| k.then_some(h)).collect();
        out.sort_by(|a, b| a.tokens.cmp(&b.tokens).then_with(|| a.grammar.cmp(&b.grammar)));
        out.dedup();
        out
    }

    /// The note of the cue that made a hit, if the entry wrote one.
    pub fn cue_note(&self, hit: &Hit) -> Option<&Three> {
        self.points.get(&hit.grammar)?.cues.get(hit.cue?)?.note.as_ref()
    }

    /// Hits grouped by grammar point, the learner's own first (see `Standing`),
    /// then by level and by where they start.
    pub fn found(&self, hits: &[Hit], syllabus: &Syllabus) -> Vec<Found> {
        let mut by_id: BTreeMap<&str, Vec<Hit>> = BTreeMap::new();
        for h in hits {
            by_id.entry(h.grammar.as_str()).or_default().push(h.clone());
        }
        let mut out: Vec<Found> = by_id
            .into_iter()
            .filter_map(|(id, hits)| {
                let point = self.points.get(id)?;
                let (standing, place) = syllabus.standing(point);
                Some(Found { grammar: id.to_string(), level: point.level.clone(), standing, place, hits })
            })
            .collect();
        out.sort_by(|a, b| {
            a.standing
                .cmp(&b.standing)
                .then_with(|| level_key(&a.level).cmp(&level_key(&b.level)))
                .then_with(|| a.hits[0].tokens.cmp(&b.hits[0].tokens))
        });
        out
    }
}

/// One grammar point found in a sentence or text, with every place it shows up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub grammar: String,
    pub level: String,
    pub standing: Standing,
    /// The lesson that teaches it: the one done, or the next one to do.
    pub place: Option<Place>,
    pub hits: Vec<Hit>,
}

/// Where the learner stands with a grammar point. Ordered: what they know
/// comes first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Standing {
    /// A lesson that teaches it is done.
    Learned,
    /// Taught at the learner's level or before, not done yet: worth a look now.
    AtLevel,
    /// Taught at a later level: shown, marked "not taught yet".
    Ahead,
    /// No route progress to compare with (or a point no level teaches).
    Unknown,
}

/// The lesson that teaches a grammar point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub lesson: String,
    pub title: String,
    pub level: String,
    pub done: bool,
}

/// What the learner's routes teach, and how far they've got.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Syllabus {
    /// Every lesson teaching each id, in route order.
    teaches: BTreeMap<String, Vec<Place>>,
    /// The level the learner is at, per language (`id`, `jv`).
    current: BTreeMap<String, String>,
}

/// `bipa3` → (0, 3), `j2` → (1, 2); unknown levels sort last.
fn level_key(level: &str) -> (u8, u32) {
    let num = |s: &str| s.parse::<u32>().ok();
    if let Some(n) = level.strip_prefix("bipa").and_then(num) {
        (0, n)
    } else if let Some(n) = level.strip_prefix('j').and_then(num) {
        (1, n)
    } else {
        (2, 0)
    }
}

impl Syllabus {
    /// Adds a route and the learner's progress on it.
    pub fn add(&mut self, curriculum: &Curriculum, progress: &Progress) {
        for level in &curriculum.levels {
            for lesson in level.lessons() {
                let done = progress.lesson_state(&lesson.id) == LessonState::Done;
                for g in &lesson.grammar {
                    self.teaches.entry(g.clone()).or_default().push(Place {
                        lesson: lesson.id.clone(),
                        title: lesson.title.clone(),
                        level: level.id.clone(),
                        done,
                    });
                }
            }
        }
        let current = progress
            .current()
            .and_then(|c| curriculum.levels.iter().find(|l| l.lessons().any(|x| x.id == c)))
            .or(curriculum.levels.last());
        if let Some(l) = current {
            self.current.insert(curriculum.language.clone(), l.id.clone());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    /// The learner's level in a language, if a route for it was added.
    pub fn level(&self, language: &str) -> Option<&str> {
        self.current.get(language).map(String::as_str)
    }

    pub fn standing(&self, point: &GrammarPoint) -> (Standing, Option<Place>) {
        let places = self.teaches.get(&point.id).map(Vec::as_slice).unwrap_or_default();
        if let Some(p) = places.iter().find(|p| p.done) {
            return (Standing::Learned, Some(p.clone()));
        }
        let next = places.first().cloned();
        let Some(current) = self.current.get(&point.language) else { return (Standing::Unknown, next) };
        let level = next.as_ref().map_or(point.level.as_str(), |p| p.level.as_str());
        let standing =
            if level_key(level) <= level_key(current) { Standing::AtLevel } else { Standing::Ahead };
        (standing, next)
    }
}

#[cfg(test)]
mod tests;
