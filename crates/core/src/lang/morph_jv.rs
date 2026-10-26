//! Javanese affix decomposition: "diculke" → di- + cul + -ke.
//!
//! Generates every structural reading (prefix × root × suffixes, with the
//! sound changes Javanese makes at the joins) and keeps those whose root the
//! lexicon knows. Fewer affixes win. With no known root, the best structural
//! guess is returned flagged `verified: false`, so the UI can badge it.

use serde::{Deserialize, Serialize};

use super::Lexicon;
use super::morph_id::Affix;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JvAnalysis {
    pub root: String,
    pub prefix: Option<Affix>,
    /// Inner to outer: derivational suffix first, then -e/-ne.
    pub suffixes: Vec<Affix>,
    /// Root changed at the join: nulis ← tulis, tukokke ← tuku.
    pub sound_change: Option<String>,
    /// The root is in the lexicon.
    pub verified: bool,
}

impl JvAnalysis {
    pub fn display(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.extend(self.prefix.iter().map(|p| p.label.clone()));
        parts.push(self.root.clone());
        parts.extend(self.suffixes.iter().map(|s| s.label.clone()));
        parts.join(" + ")
    }

    fn affix_count(&self) -> usize {
        self.prefix.iter().count() + self.suffixes.len()
    }
}

/// (surface, label). Nasal prefixes are handled separately.
const PREFIXES: &[(&str, &str)] = &[
    ("dipun", "dipun-"),
    ("di", "di-"),
    ("ka", "ka-"),
    ("ke", "ke-"),
    ("sa", "sa-"),
    ("tak", "tak-"),
    ("dak", "tak-"),
    ("kok", "kok-"),
];

/// Derivational suffixes (surface, label), longest first.
const SUFFIXES: &[(&str, &str)] = &[
    ("akake", "-ake"),
    ("kake", "-ake"),
    ("aken", "-aken"),
    ("ake", "-ake"),
    ("ke", "-ke"),
    ("nana", "-ana"),
    ("ana", "-ana"),
    ("nen", "-en"),
    ("en", "-en"),
    ("ni", "-i"),
    ("an", "-an"),
    ("i", "-i"),
    ("a", "-a"),
];

/// Possessive / definite enclitics (outermost).
const ENCLITICS: &[(&str, &str)] =
    &[("ipun", "-ipun"), ("ne", "-ne"), ("e", "-e"), ("mu", "-mu"), ("ku", "-ku")];

pub fn analyze(word: &str, lex: &dyn Lexicon) -> Option<JvAnalysis> {
    let w = word.to_lowercase();
    if w.chars().count() < 2 {
        return None;
    }
    let mut candidates = Vec::new();

    for (body, encl) in strip_optional(&w, ENCLITICS) {
        for (stem, suffix) in strip_optional(body, SUFFIXES) {
            for (pre, root, change) in prefix_readings(stem) {
                for (root, alt_change) in suffix_join_variants(&root, suffix.is_some()) {
                    if root.chars().count() < 2 {
                        continue;
                    }
                    let mut suffixes = Vec::new();
                    if let Some((s, l)) = suffix {
                        suffixes.push(Affix { label: l.to_string(), surface: s.to_string() });
                    }
                    if let Some((s, l)) = encl {
                        suffixes.push(Affix { label: l.to_string(), surface: s.to_string() });
                    }
                    let verified = lex.is_jv_root(&root);
                    candidates.push(JvAnalysis {
                        root,
                        prefix: pre.clone(),
                        suffixes,
                        sound_change: change.clone().or(alt_change),
                        verified,
                    });
                }
            }
        }
    }

    // Verified first, then fewest affixes, then no sound change, then longer root.
    candidates.sort_by_key(|a| {
        (!a.verified, a.affix_count(), a.sound_change.is_some(), std::cmp::Reverse(a.root.len()))
    });
    candidates.into_iter().next()
}

/// The word itself plus every way to strip one item of `list` from the end.
fn strip_optional<'a>(
    w: &'a str,
    list: &'static [(&'static str, &'static str)],
) -> Vec<(&'a str, Option<(&'static str, &'static str)>)> {
    let mut out = vec![(w, None)];
    for &(s, l) in list {
        if let Some(rest) = w.strip_suffix(s)
            && rest.chars().count() >= 2
        {
            out.push((rest, Some((s, l))));
        }
    }
    out
}

type Reading = (Option<Affix>, String, Option<String>);

/// No prefix, each plain prefix, and nasal N- with root-initial replacement.
fn prefix_readings(stem: &str) -> Vec<Reading> {
    let mut out: Vec<Reading> = vec![(None, stem.to_string(), None)];
    let affix =
        |surface: &str, label: &str| Some(Affix { label: label.to_string(), surface: surface.to_string() });

    for &(s, l) in PREFIXES {
        if let Some(rest) = stem.strip_prefix(s) {
            out.push((affix(s, l), rest.to_string(), None));
        }
    }

    // N-: nasal + replaced or kept initial. ny←s/c, ng←k/vowel, n←t/th, m←p/w/b.
    let nasal: &[(&str, &[&str])] = &[
        ("nge", &[""]),
        ("ny", &["s", "c"]),
        ("ng", &["k", ""]),
        ("n", &["t", "th", "d"]),
        ("m", &["p", "w", "b", ""]),
    ];
    for &(n, restores) in nasal {
        if let Some(rest) = stem.strip_prefix(n) {
            if rest.is_empty() {
                continue;
            }
            for &r in restores {
                let root = format!("{r}{rest}");
                let change = (!r.is_empty()).then(|| format!("{r}→{n}"));
                out.push((affix(n, "N-"), root, change));
            }
        }
    }
    out
}

/// Before a suffix, vowel-final roots change: tuku → tukok-ke, tuku → tuko-ni.
fn suffix_join_variants(root: &str, had_suffix: bool) -> Vec<(String, Option<String>)> {
    let mut out = vec![(root.to_string(), None)];
    if !had_suffix {
        return out;
    }
    let mut push_vowel_restore = |r: &str| {
        if let Some(base) = r.strip_suffix('o') {
            out.push((format!("{base}u"), Some("u→o".into())));
        }
        if let Some(base) = r.strip_suffix('e') {
            out.push((format!("{base}i"), Some("i→e".into())));
        }
    };
    push_vowel_restore(root);
    // Inserted k before -ke/-ake: tukok-ke → tuko → tuku.
    if let Some(base) = root.strip_suffix('k') {
        push_vowel_restore(base);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::BuiltinLexicon;

    fn show(w: &str) -> String {
        let lex = BuiltinLexicon::default();
        analyze(w, &lex).map(|a| a.display()).unwrap_or_default()
    }

    #[test]
    fn test_text_words() {
        assert_eq!(show("diculke"), "di- + cul + -ke");
        assert_eq!(show("tukokke"), "tuku + -ke");
        assert_eq!(show("ditukoni"), "di- + tuku + -i");
        assert_eq!(show("omahe"), "omah + -e");
        assert_eq!(show("mangan"), "mangan");
        assert_eq!(show("piro"), "piro");
    }

    #[test]
    fn nasal_prefix() {
        let lex = BuiltinLexicon::default();
        let a = analyze("nulis", &lex).unwrap();
        assert_eq!((a.display().as_str(), a.sound_change.as_deref()), ("N- + tulis", Some("t→n")));
        assert_eq!(show("maca"), "N- + waca");
        assert_eq!(show("nyapu"), "N- + sapu");
        assert_eq!(show("ngombe"), "N- + ombe");
        assert_eq!(show("mlaku"), "N- + laku");
    }

    #[test]
    fn unknown_roots_are_flagged() {
        let lex = BuiltinLexicon::default();
        let a = analyze("dikrumpyuki", &lex).unwrap();
        assert!(!a.verified);
    }
}
