//! Core vocabulary is taught before it is used (#136): a word in the top
//! [`CORE_RANK`] of `content/freq/<lang>.tsv` that a lesson's exercises use
//! must already be a `new_words` entry in that lesson or an earlier one (in
//! curriculum order). Review cards come only from `new_words`, so a core word
//! that is never taught never reaches the review deck.
//!
//! A taught phrase (`rumah sakit`) does not teach its parts, but its parts
//! don't count as uses inside it either. A suffixed (`rumahnya`, `omahe`) or
//! reduplicated (`baik-baik`) form is covered by its taught root.
//!
//! `content/freq/<lang>.core-exclude.tsv` lists words that are not
//! vocabulary to teach (names, particles taught through grammar slides);
//! `content/freq/<lang>.core-pending.tsv` lists words still being moved
//! into a lesson. Both are `word<TAB>reason`; a pending word that is already
//! taught in time must be dropped from the pending list.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use crate::model::Curriculum;
use crate::retrieval::{forms, target_text};

/// Frequency ranks that count as core vocabulary.
pub const CORE_RANK: usize = 300;

pub(crate) fn suffixes(lang: &str) -> &'static [&'static str] {
    match lang {
        "id" => &["nya", "ku", "mu", "lah", "kah", "pun"],
        "jv" => &["nipun", "ipun", "ne", "e", "ku", "mu"],
        _ => &[],
    }
}

/// The first column of a `word<TAB>reason` list, skipping comments.
pub(crate) fn word_list(path: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split('\t').next())
        .map(|w| w.trim().to_lowercase())
        .collect()
}

/// Core words with their rank, from a `rank<TAB>word<TAB>…` frequency list.
fn core_words(path: &Path) -> BTreeMap<String, usize> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let rows = text.lines().filter(|l| !l.starts_with('#')).filter_map(|l| {
        let mut cols = l.split('\t');
        Some((cols.next()?.trim().parse::<usize>().ok()?, cols.next()?.trim().to_lowercase()))
    });
    rows.take_while(|(rank, _)| *rank <= CORE_RANK).map(|(rank, w)| (w, rank)).collect()
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Words in `text`, keeping hyphenated compounds (`kupu-kupu`) whole.
pub(crate) fn tokens(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        let joins = c == '-' && !cur.is_empty() && chars.get(i + 1).is_some_and(|&n| is_word_char(n));
        if is_word_char(c) || joins {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `text` with every whole-word occurrence of `phrase` blanked out.
pub(crate) fn mask(text: &str, phrase: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(phrase) {
        let before = rest[..at].chars().next_back().or_else(|| out.chars().next_back());
        let after = rest[at + phrase.len()..].chars().next();
        let bounded = !before.is_some_and(|c| is_word_char(c) || c == '-')
            && !after.is_some_and(|c| is_word_char(c) || c == '-');
        out.push_str(&rest[..at]);
        out.push_str(if bounded { " # " } else { phrase });
        rest = &rest[at + phrase.len()..];
    }
    out.push_str(rest);
    out
}

pub fn check(
    root: &Path,
    curricula: &[Curriculum],
    lessons: &BTreeMap<String, Value>,
    problems: &mut Vec<String>,
) {
    for c in curricula {
        let lang = c.language.as_str();
        let freq = root.join(format!("content/freq/{lang}.tsv"));
        if !freq.exists() {
            continue;
        }
        let core = core_words(&freq);
        let exclude = word_list(&root.join(format!("content/freq/{lang}.core-exclude.tsv")));
        let pending = word_list(&root.join(format!("content/freq/{lang}.core-pending.tsv")));
        let order: Vec<&str> = c
            .levels
            .iter()
            .flat_map(|l| &l.units)
            .flat_map(|u| &u.lessons)
            .map(|l| l.id.as_str())
            .filter(|id| lessons.contains_key(*id))
            .collect();

        let mut taught: BTreeMap<String, usize> = BTreeMap::new();
        let mut early_pending = BTreeSet::new();
        for (pos, id) in order.iter().enumerate() {
            let lesson = &lessons[*id];
            let new_words = lesson.get("new_words").and_then(Value::as_array).into_iter().flatten();
            for w in new_words.filter_map(|w| w.get("word").and_then(Value::as_str)) {
                for f in forms(w) {
                    taught.entry(f).or_insert(pos);
                }
            }
            let mut phrases: Vec<&str> =
                taught.keys().filter(|w| w.contains(' ')).map(String::as_str).collect();
            phrases.sort_by_key(|p| std::cmp::Reverse(p.len()));
            let covered = |w: &str| -> bool {
                let at = |w: &str| taught.get(w).is_some_and(|&p| p <= pos);
                if at(w) {
                    return true;
                }
                if let Some((a, b)) = w.split_once('-')
                    && a == b
                    && at(a)
                {
                    return true;
                }
                suffixes(lang).iter().any(|s| w.len() > s.len() + 2 && w.strip_suffix(s).is_some_and(at))
            };
            for e in lesson.get("exercises").and_then(Value::as_array).into_iter().flatten() {
                let ex = e.get("id").and_then(Value::as_str).unwrap_or("?");
                let text = phrases.iter().fold(target_text(e, false), |t, p| mask(&t, p));
                let tags =
                    e.get("words").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
                let mut used: BTreeSet<String> = tokens(&text).into_iter().collect();
                used.extend(tags.map(str::to_lowercase).filter(|t| !t.contains(' ')));
                for w in used {
                    let Some(&rank) = core.get(&w) else { continue };
                    if exclude.contains(&w) || covered(&w) {
                        continue;
                    }
                    if pending.contains(&w) {
                        early_pending.insert(w);
                        continue;
                    }
                    problems.push(format!(
                        "{id} {ex}: uses core word '{w}' (frequency rank {rank}) before any lesson teaches it; \
                         add it to new_words here or in an earlier lesson"
                    ));
                }
            }
        }
        for w in pending.difference(&early_pending) {
            problems.push(format!(
                "content/freq/{lang}.core-pending.tsv: '{w}' is now taught before every use; drop it from the list"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_keep_compounds() {
        assert_eq!(tokens("kupu-kupu, rumahnya -nya!"), ["kupu-kupu", "rumahnya", "nya"]);
    }

    #[test]
    fn mask_blanks_whole_phrases_only() {
        assert_eq!(mask("ke rumah sakit. sakit?", "rumah sakit"), "ke  # . sakit?");
        assert_eq!(mask("serumah sakit", "rumah sakit"), "serumah sakit");
    }
}
