//! Indonesian affix decomposition for display: "dibersihkan" → di- + bersih + -kan.
//!
//! sastrawi-rs finds the dictionary-validated root; this module explains how
//! the surface word was built from it, including meN-/peN- allomorphs with
//! nasal replacement (menulis = meN- + tulis) and confixes (ke-an, per-an …).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Affix {
    /// Canonical label: "meN-", "di-", "-kan", "ke-an", "-nya".
    pub label: String,
    /// How it appears in this word: "mem", "kan", "ke…an".
    pub surface: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdAnalysis {
    pub root: String,
    /// Outermost first.
    pub prefixes: Vec<Affix>,
    pub confix: Option<Affix>,
    pub suffix: Option<Affix>,
    pub clitic: Option<Affix>,
    /// The root's first sound was replaced by the nasal (menulis ← tulis).
    pub nasal_replaced: bool,
}

impl IdAnalysis {
    pub fn is_bare(&self) -> bool {
        self.prefixes.is_empty() && self.confix.is_none() && self.suffix.is_none() && self.clitic.is_none()
    }

    /// "di- + bersih + -kan", "ke-an + sehat".
    pub fn display(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(c) = &self.confix {
            parts.push(c.label.clone());
        }
        parts.extend(self.prefixes.iter().map(|p| p.label.clone()));
        parts.push(self.root.clone());
        parts.extend(self.suffix.iter().map(|s| s.label.clone()));
        parts.extend(self.clitic.iter().map(|c| c.label.clone()));
        parts.join(" + ")
    }

    pub fn labels(&self) -> Vec<&str> {
        let mut v: Vec<&str> = Vec::new();
        v.extend(self.confix.iter().map(|a| a.label.as_str()));
        v.extend(self.prefixes.iter().map(|a| a.label.as_str()));
        v.extend(self.suffix.iter().map(|a| a.label.as_str()));
        v.extend(self.clitic.iter().map(|a| a.label.as_str()));
        v
    }
}

/// (surface, label), longest surfaces first.
const PREFIXES: &[(&str, &str)] = &[
    ("menge", "meN-"),
    ("penge", "peN-"),
    ("meng", "meN-"),
    ("meny", "meN-"),
    ("peng", "peN-"),
    ("peny", "peN-"),
    ("mem", "meN-"),
    ("men", "meN-"),
    ("pem", "peN-"),
    ("pen", "peN-"),
    ("ber", "ber-"),
    ("bel", "ber-"),
    ("ter", "ter-"),
    ("per", "per-"),
    ("kau", "kau-"),
    ("me", "meN-"),
    ("pe", "peN-"),
    ("be", "ber-"),
    ("te", "ter-"),
    ("di", "di-"),
    ("ke", "ke-"),
    ("se", "se-"),
    ("ku", "ku-"),
];

const SUFFIXES: &[&str] = &["kan", "an", "i"];
const CLITICS: &[&str] = &["nya", "lah", "kah", "pun", "tah", "ku", "mu"];

pub struct IndonesianMorph {
    dict: sastrawi::Dictionary,
}

impl Default for IndonesianMorph {
    fn default() -> Self {
        Self { dict: sastrawi::Dictionary::new() }
    }
}

impl IndonesianMorph {
    /// Uses a custom root list (e.g. from the Wiktionary import).
    pub fn with_roots(roots: &[&str]) -> Self {
        Self { dict: sastrawi::Dictionary::custom(roots) }
    }

    pub fn is_root(&self, word: &str) -> bool {
        self.dict.find(word)
    }

    /// The dictionary-validated root, if sastrawi finds one.
    pub fn root(&self, word: &str) -> Option<String> {
        let lower = word.to_lowercase();
        let stem = sastrawi::Stemmer::new(&self.dict).stem_word(&lower).into_owned();
        self.dict.find(&stem).then_some(stem)
    }

    pub fn analyze(&self, word: &str) -> Option<IdAnalysis> {
        let lower = word.to_lowercase();
        let root = self.root(&lower)?;
        segment(&lower, &root)
    }
}

/// Explains `word` as affixes around `root`. `None` when the pieces don't add up.
pub fn segment(word: &str, root: &str) -> Option<IdAnalysis> {
    let (prefix_part, suffix_part, nasal_replaced) = split_around_root(word, root)?;
    let mut prefixes = parse_prefixes(prefix_part)?;
    let (suffix, clitic) = parse_suffixes(suffix_part)?;

    // ke-/peN-/per-/ber- (outermost) with -an form a confix.
    let mut suffix = suffix;
    let mut confix = None;
    if suffix.as_deref() == Some("an")
        && let Some(first) = prefixes.first()
        && matches!(first.label.as_str(), "ke-" | "peN-" | "per-" | "ber-")
    {
        let p = prefixes.remove(0);
        let base = p.label.trim_end_matches('-').to_string();
        confix = Some(Affix { label: format!("{base}-an"), surface: format!("{}…an", p.surface) });
        suffix = None;
    }

    Some(IdAnalysis {
        root: root.to_string(),
        prefixes,
        confix,
        suffix: suffix.map(|s| Affix { label: format!("-{s}"), surface: s }),
        clitic: clitic.map(|c| Affix { label: format!("-{c}"), surface: c }),
        nasal_replaced,
    })
}

fn split_around_root<'a>(word: &'a str, root: &str) -> Option<(&'a str, &'a str, bool)> {
    if let Some(i) = word.find(root) {
        return Some((&word[..i], &word[i + root.len()..], false));
    }
    // Nasal replacement: p→m, t→n, s→ny, k→ng (menulis ← tulis, mengirim ← kirim).
    let first = root.chars().next()?;
    if !matches!(first, 'p' | 't' | 's' | 'k') {
        return None;
    }
    let rest = &root[first.len_utf8()..];
    let i = word.find(rest)?;
    let before = &word[..i];
    let nasal_ok = match first {
        'p' => before.ends_with('m'),
        't' => before.ends_with('n'),
        's' => before.ends_with("ny"),
        'k' => before.ends_with("ng"),
        _ => false,
    };
    nasal_ok.then(|| (before, &word[i + rest.len()..], true))
}

fn parse_prefixes(mut s: &str) -> Option<Vec<Affix>> {
    let mut out = Vec::new();
    while !s.is_empty() {
        let &(surface, label) = PREFIXES.iter().find(|(p, _)| s.starts_with(p))?;
        out.push(Affix { label: label.to_string(), surface: surface.to_string() });
        s = &s[surface.len()..];
        if out.len() > 3 {
            return None;
        }
    }
    Some(out)
}

fn parse_suffixes(s: &str) -> Option<(Option<String>, Option<String>)> {
    if s.is_empty() {
        return Some((None, None));
    }
    for c in CLITICS {
        if let Some(rest) = s.strip_suffix(c) {
            if rest.is_empty() {
                return Some((None, Some(c.to_string())));
            }
            if SUFFIXES.contains(&rest) {
                return Some((Some(rest.to_string()), Some(c.to_string())));
            }
        }
    }
    SUFFIXES.contains(&s).then(|| (Some(s.to_string()), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(m: &IndonesianMorph, w: &str) -> String {
        m.analyze(w).map(|a| a.display()).unwrap_or_else(|| format!("? {w}"))
    }

    #[test]
    fn spec_morphology() {
        let m = IndonesianMorph::default();
        assert_eq!(show(&m, "membaca"), "meN- + baca");
        assert_eq!(show(&m, "dibersihkan"), "di- + bersih + -kan");
        assert_eq!(show(&m, "Rumahnya"), "rumah + -nya");
        assert_eq!(show(&m, "bermain"), "ber- + main");
        assert_eq!(show(&m, "Kesehatan"), "ke-an + sehat");
        assert_eq!(show(&m, "buku"), "buku");
    }

    #[test]
    fn nasal_replacement_and_stacks() {
        let m = IndonesianMorph::default();
        let a = m.analyze("menulis").unwrap();
        assert_eq!((a.display().as_str(), a.nasal_replaced), ("meN- + tulis", true));
        assert_eq!(show(&m, "mengirim"), "meN- + kirim");
        assert_eq!(show(&m, "menyapu"), "meN- + sapu");
        assert_eq!(show(&m, "memperbaiki"), "meN- + per- + baik + -i");
        assert_eq!(show(&m, "keberuntunganmu"), "ke-an + ber- + untung + -mu");
        assert_eq!(show(&m, "pertanian"), "per-an + tani");
        assert_eq!(show(&m, "penjualan"), "peN-an + jual");
        assert_eq!(show(&m, "belajar"), "ber- + ajar");
        assert_eq!(show(&m, "dibersihkannya"), "di- + bersih + -kan + -nya");
    }

    #[test]
    fn surfaces_are_kept() {
        let a = IndonesianMorph::default().analyze("membersihkan").unwrap();
        assert_eq!(a.prefixes[0].surface, "mem");
        assert_eq!(a.suffix.unwrap().surface, "kan");
    }
}
