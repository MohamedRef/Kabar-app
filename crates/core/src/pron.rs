//! Pronunciation in Arabic script (تعريب النطق) for Indonesian and Javanese.
//!
//! Words become phones first, then Arabic letters with full vowel marks, so
//! an Egyptian reader can say a word before hearing it. The scheme:
//!
//! - every full vowel is a mark plus a letter: a `ـَا`, i `ـِي`, u `ـُو`,
//!   é `ێ`, o `ۆ`. The letter only names the vowel; Indonesian vowels are short.
//! - the weak e (pepet, ə) is a bare kasra, nothing after it.
//! - letters read the Egyptian way: g is `ج` (the Egyptian g), and j is `چ`
//!   (dj, as Egyptian print writes "jacket" چاكيت). Sounds Arabic lacks get one
//!   fixed spelling each: p `پ`, c `تْش`, ng `ڠ`, ny `نْي`.
//! - a final or pre-consonant k is a glottal stop, written `ء`.
//! - Javanese dh/th are `ض`/`ط`, and a final open-syllable a reads `ۆ` (apa → أۆپۆ).
//!
//! Indonesian spelling is regular except for e (ə or é) and some k (k or ʔ).
//! Those come from a pronunciation lexicon (`content/pron/id.tsv`, from
//! g2p_id, Apache-2.0) and hand overrides (`content/pron/overrides.tsv`);
//! words outside both fall back to the rules, borrowing the e-pattern of a
//! known root when the word is an affixed form of it (kesorean ← sore).
//! Javanese has no lexicon yet, so it is rules plus overrides and stays
//! unverified.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::lang::Lang;

/// One sound. `Sep` joins the halves of a reduplication (anak-anak).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ph {
    A,
    I,
    U,
    /// é / è: the open e of "sore".
    E,
    /// ə: the weak e (pepet) of "besar".
    Schwa,
    O,
    /// ɔ: Javanese open o, also written a (apa = ɔpɔ).
    OpenO,
    B,
    P,
    T,
    D,
    K,
    G,
    Glottal,
    /// c, tʃ.
    C,
    /// j, dʒ.
    J,
    F,
    V,
    S,
    Z,
    /// sy, ʃ.
    Sy,
    /// kh, x.
    Kh,
    H,
    M,
    N,
    Ng,
    Ny,
    L,
    R,
    W,
    /// y, the glide j.
    Y,
    /// Javanese dh.
    Dh,
    /// Javanese th.
    Th,
    Sep,
}

/// One character per phone, as stored in `content/pron/id.tsv`.
const CODES: &[(Ph, char)] = &[
    (Ph::A, 'a'),
    (Ph::I, 'i'),
    (Ph::U, 'u'),
    (Ph::E, 'e'),
    (Ph::Schwa, 'ə'),
    (Ph::O, 'o'),
    (Ph::OpenO, 'ɔ'),
    (Ph::B, 'b'),
    (Ph::P, 'p'),
    (Ph::T, 't'),
    (Ph::D, 'd'),
    (Ph::K, 'k'),
    (Ph::G, 'g'),
    (Ph::Glottal, 'ʔ'),
    (Ph::C, 'c'),
    (Ph::J, 'j'),
    (Ph::F, 'f'),
    (Ph::V, 'v'),
    (Ph::S, 's'),
    (Ph::Z, 'z'),
    (Ph::Sy, 'ʃ'),
    (Ph::Kh, 'x'),
    (Ph::H, 'h'),
    (Ph::M, 'm'),
    (Ph::N, 'n'),
    (Ph::Ng, 'ŋ'),
    (Ph::Ny, 'ɲ'),
    (Ph::L, 'l'),
    (Ph::R, 'r'),
    (Ph::W, 'w'),
    (Ph::Y, 'y'),
    (Ph::Dh, 'ɖ'),
    (Ph::Th, 'ʈ'),
    (Ph::Sep, '-'),
];

impl Ph {
    pub fn is_vowel(self) -> bool {
        matches!(self, Ph::A | Ph::I | Ph::U | Ph::E | Ph::Schwa | Ph::O | Ph::OpenO)
    }

    pub fn code(self) -> char {
        CODES.iter().find(|(p, _)| *p == self).map(|(_, c)| *c).expect("every phone has a code")
    }

    pub fn from_code(c: char) -> Option<Self> {
        CODES.iter().find(|(_, k)| *k == c).map(|(p, _)| *p)
    }

    /// Reads one IPA token as written by g2p_id (`tʃ`, `dʒ`, `ɡ`, `j` …).
    pub fn from_ipa(token: &str) -> Option<Self> {
        match token {
            "tʃ" => Some(Ph::C),
            "dʒ" => Some(Ph::J),
            "j" => Some(Ph::Y),
            "ɡ" => Some(Ph::G),
            _ => {
                let mut chars = token.chars();
                let c = chars.next()?;
                if chars.next().is_some() || matches!(c, 'c' | 'y') {
                    return None;
                }
                Self::from_code(c)
            }
        }
    }
}

/// Phones as their one-character codes ("besar" → "bəsar").
pub fn codes(phones: &[Ph]) -> String {
    phones.iter().map(|p| p.code()).collect()
}

fn parse_codes(s: &str) -> Option<Vec<Ph>> {
    s.chars().map(Ph::from_code).collect()
}

/// The pronunciation dictionary: the Indonesian lexicon plus hand overrides.
#[derive(Default)]
pub struct PronDict {
    id: HashMap<String, Vec<Ph>>,
    /// Hand overrides, as marked spellings (soré, dèrèng, ma'af).
    overrides: HashMap<(bool, String), String>,
}

const ID_LEXICON: &str = include_str!("../../../content/pron/id.tsv");
const OVERRIDES: &str = include_str!("../../../content/pron/overrides.tsv");

impl PronDict {
    /// The dictionary built into the app.
    pub fn builtin() -> &'static Self {
        static DICT: OnceLock<PronDict> = OnceLock::new();
        DICT.get_or_init(|| Self::parse(ID_LEXICON, OVERRIDES).expect("content/pron is valid"))
    }

    /// `id_lexicon`: `word<TAB>codes` lines. `overrides`: `lang<TAB>word<TAB>marked spelling[<TAB>note]`
    /// lines, lang `id` or `jv`. `#` starts a comment line.
    pub fn parse(id_lexicon: &str, overrides: &str) -> Result<Self, String> {
        let mut dict = Self::default();
        for (n, line) in data_lines(id_lexicon) {
            let (word, code) = line.split_once('\t').ok_or(format!("id.tsv:{n}: expected word<TAB>codes"))?;
            let phones = parse_codes(code).ok_or(format!("id.tsv:{n}: unknown phone code in {code:?}"))?;
            if dict.id.insert(word.to_string(), phones).is_some() {
                return Err(format!("id.tsv:{n}: duplicate word {word:?}"));
            }
        }
        for (n, line) in data_lines(overrides) {
            let cols: Vec<&str> = line.split('\t').collect();
            let [lang, word, marked, ..] = cols[..] else {
                return Err(format!("overrides.tsv:{n}: expected lang<TAB>word<TAB>spelling"));
            };
            let jv = match lang {
                "id" => false,
                "jv" => true,
                _ => return Err(format!("overrides.tsv:{n}: lang must be id or jv, not {lang:?}")),
            };
            if fold(marked) != word {
                return Err(format!("overrides.tsv:{n}: {marked:?} is not a marked spelling of {word:?}"));
            }
            if dict.overrides.insert((jv, word.to_string()), marked.to_string()).is_some() {
                return Err(format!("overrides.tsv:{n}: duplicate {lang} word {word:?}"));
            }
        }
        Ok(dict)
    }

    /// The phones of one word. Javanese uses the Javanese rules; every other
    /// language (proper names included) the Indonesian ones.
    pub fn phones(&self, word: &str, lang: Lang) -> Vec<Ph> {
        let jv = lang == Lang::Javanese;
        let word = word.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
        if word.is_empty() {
            return Vec::new();
        }
        if let Some(marked) = self.overrides.get(&(jv, word.clone())) {
            return rules(marked, jv, &HashMap::new());
        }
        if !jv && let Some(p) = self.id.get(&word) {
            return p.clone();
        }
        if word.contains('-') {
            let mut out = Vec::new();
            for (k, part) in word.split('-').filter(|p| !p.is_empty()).enumerate() {
                if k > 0 {
                    out.push(Ph::Sep);
                }
                out.extend(self.phones(part, lang));
            }
            return out;
        }
        rules(&word, jv, &self.root_e_pattern(&word, jv))
    }

    /// The word in Arabic script with full vowel marks.
    pub fn word(&self, word: &str, lang: Lang) -> String {
        arabic(&self.phones(word, lang))
    }

    /// A phrase or sentence: words transliterated, punctuation turned Arabic
    /// (، ؟ ؛), everything else kept.
    pub fn text(&self, text: &str, lang: Lang) -> String {
        let mut out = String::new();
        let mut word = String::new();
        let chars: Vec<char> = text.chars().collect();
        for (k, &c) in chars.iter().enumerate() {
            let inner = matches!(c, '-' | '\'' | '’')
                && !word.is_empty()
                && chars.get(k + 1).is_some_and(|n| n.is_alphabetic());
            if c.is_alphabetic() || inner {
                word.push(c);
                continue;
            }
            if !word.is_empty() {
                out.push_str(&self.word(&word, lang));
                word.clear();
            }
            out.push(match c {
                ',' => '،',
                '?' => '؟',
                ';' => '؛',
                c => c,
            });
        }
        if !word.is_empty() {
            out.push_str(&self.word(&word, lang));
        }
        out
    }

    /// For an affixed word missing from the lexicon: the é/ə of each `e`
    /// letter inside the longest known root (char index → phone).
    fn root_e_pattern(&self, word: &str, jv: bool) -> HashMap<usize, Ph> {
        let chars: Vec<char> = word.chars().collect();
        let mut best: Option<(usize, Vec<Ph>)> = None;
        for (start, root) in root_candidates(&chars, jv) {
            let root_str: String = root.iter().collect();
            if root.len() < 3 || best.as_ref().is_some_and(|(_, b)| b.len() >= root.len()) {
                continue;
            }
            let phones = if let Some(marked) = self.overrides.get(&(jv, root_str.clone())) {
                rules(marked, jv, &HashMap::new())
            } else if let Some(p) = self.id.get(&root_str).filter(|_| !jv) {
                p.clone()
            } else {
                continue;
            };
            let es: Vec<Ph> = phones.into_iter().filter(|p| matches!(p, Ph::E | Ph::Schwa)).collect();
            let letters = root.iter().filter(|c| **c == 'e').count();
            if es.len() == letters {
                best = Some((start, es));
            }
        }
        let mut out = HashMap::new();
        if let Some((start, es)) = best {
            let positions = (start..chars.len()).filter(|&i| chars[i] == 'e');
            for (i, p) in positions.zip(es) {
                out.insert(i, p);
            }
        }
        out
    }
}

fn data_lines(s: &str) -> impl Iterator<Item = (usize, &str)> {
    s.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
}

/// Strips the marks a marked spelling may carry: é è ê → e, ò â → o a, ' dropped.
fn fold(marked: &str) -> String {
    marked
        .chars()
        .filter(|c| !matches!(c, '\'' | '’'))
        .map(|c| match c {
            'é' | 'è' | 'ê' => 'e',
            'ò' => 'o',
            'â' => 'a',
            c => c,
        })
        .collect()
}

const ID_PREFIXES: &[&str] = &[
    "memper", "diper", "menge", "penge", "meng", "meny", "peng", "peny", "mem", "men", "pem", "pen", "ber",
    "ter", "per", "me", "pe", "be", "te", "di", "ke", "se",
];
const ID_SUFFIXES: &[&str] =
    &["kannya", "annya", "inya", "kan", "nya", "lah", "kah", "pun", "an", "ku", "mu", "i"];
const JV_PREFIXES: &[&str] = &["dipun", "ka", "di", "ke", "sa", "pa", "ng", "ny", "m", "n"];
const JV_SUFFIXES: &[&str] =
    &["ake", "aken", "kake", "ke", "ne", "e", "an", "en", "na", "a", "i", "mu", "ku"];

/// Every (start index, root) the word could be built on, the word itself
/// included. Nasal replacement is undone (menulis → tulis, at the same start).
fn root_candidates(chars: &[char], jv: bool) -> Vec<(usize, Vec<char>)> {
    let (prefixes, suffixes) = if jv { (JV_PREFIXES, JV_SUFFIXES) } else { (ID_PREFIXES, ID_SUFFIXES) };
    let word: String = chars.iter().collect();
    let mut out = Vec::new();
    let mut heads: Vec<usize> = vec![0];
    heads.extend(prefixes.iter().filter(|p| word.starts_with(*p)).map(|p| p.chars().count()));
    let mut tails: Vec<usize> = vec![chars.len()];
    tails.extend(suffixes.iter().filter(|s| word.ends_with(*s)).map(|s| chars.len() - s.chars().count()));
    for &h in &heads {
        for &t in &tails {
            if h + 2 >= t {
                continue;
            }
            let core = &chars[h..t];
            out.push((h, core.to_vec()));
            // meN-/peN- + vowel: the root's first consonant was dropped.
            if h > 0 && core.first().is_some_and(|c| "aiueo".contains(*c)) {
                let nasal = &word[..word.char_indices().nth(h).map_or(word.len(), |(i, _)| i)];
                let lost = if nasal.ends_with("ng") || (jv && nasal == "ng") {
                    Some('k')
                } else if nasal.ends_with("ny") {
                    Some('s')
                } else if nasal.ends_with('m') {
                    Some('p')
                } else if nasal.ends_with('n') {
                    Some('t')
                } else {
                    None
                };
                if let Some(l) = lost {
                    let mut r = vec![l];
                    r.extend_from_slice(core);
                    // The restored letter sits before `h`; e letters start at `h`.
                    out.push((h.saturating_sub(1), r));
                }
            }
        }
    }
    out
}

/// The rules alone, without lexicon or overrides: what a word would get if
/// nobody had listed it. `kabar-tools pron import` keeps only lexicon entries
/// the rules get wrong (or that carry an e a root lookup may need).
pub fn rule_phones(word: &str, lang: Lang) -> Vec<Ph> {
    rules(&word.to_lowercase(), lang == Lang::Javanese, &HashMap::new())
}

fn is_vowel_letter(c: char) -> bool {
    "aiueoéèêòâ".contains(c)
}

/// Spelling → phones. `forced_e` fixes the vowel of plain `e` letters by
/// char index; the rest follow the rules.
fn rules(word: &str, jv: bool, forced_e: &HashMap<usize, Ph>) -> Vec<Ph> {
    let chars: Vec<char> = word.to_lowercase().chars().collect();
    let mut out: Vec<Ph> = Vec::new();
    let mut open_e: Vec<usize> = Vec::new();
    let mut plain_a: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let mut step = 1;
        let ph = match (c, next) {
            ('a', _) => {
                plain_a.push(out.len());
                Some(Ph::A)
            }
            ('â', _) => Some(Ph::A),
            ('i', _) => Some(Ph::I),
            ('u', _) => Some(Ph::U),
            ('o', _) => Some(Ph::O),
            ('ò', _) => Some(Ph::OpenO),
            ('é' | 'è', _) => Some(Ph::E),
            ('ê', _) => Some(Ph::Schwa),
            ('e', _) => match forced_e.get(&i) {
                Some(p) => Some(*p),
                None => {
                    open_e.push(out.len());
                    Some(Ph::Schwa)
                }
            },
            ('n', Some('g')) => {
                step = 2;
                Some(Ph::Ng)
            }
            ('n', Some('y')) if chars.get(i + 2).is_some_and(|c| is_vowel_letter(*c)) => {
                step = 2;
                Some(Ph::Ny)
            }
            ('s', Some('y')) if chars.get(i + 2).is_some_and(|c| is_vowel_letter(*c)) => {
                step = 2;
                Some(Ph::Sy)
            }
            ('k', Some('h')) => {
                step = 2;
                Some(Ph::Kh)
            }
            ('d', Some('h')) => {
                step = 2;
                Some(if jv { Ph::Dh } else { Ph::D })
            }
            ('t', Some('h')) => {
                step = 2;
                Some(if jv { Ph::Th } else { Ph::T })
            }
            ('g', Some('h')) => {
                step = 2;
                Some(Ph::G)
            }
            ('p', Some('h')) => {
                step = 2;
                Some(Ph::F)
            }
            ('c', Some('h')) => {
                step = 2;
                Some(Ph::C)
            }
            ('x', _) => {
                if i > 0 {
                    out.push(Ph::K);
                }
                Some(Ph::S)
            }
            ('b', _) => Some(Ph::B),
            ('p', _) => Some(Ph::P),
            ('t', _) => Some(Ph::T),
            ('d', _) => Some(Ph::D),
            ('k' | 'q', _) => Some(Ph::K),
            ('g', _) => Some(Ph::G),
            ('c', _) => Some(Ph::C),
            ('j', _) => Some(Ph::J),
            ('f', _) => Some(Ph::F),
            ('v', _) => Some(Ph::V),
            ('s', _) => Some(Ph::S),
            ('z', _) => Some(Ph::Z),
            ('h', _) => Some(Ph::H),
            ('m', _) => Some(Ph::M),
            ('n', _) => Some(Ph::N),
            ('l', _) => Some(Ph::L),
            ('r', _) => Some(Ph::R),
            ('w', _) => Some(Ph::W),
            ('y', _) => Some(Ph::Y),
            ('\'' | '’', _) => Some(Ph::Glottal),
            ('-', _) => Some(Ph::Sep),
            _ => None,
        };
        out.extend(ph);
        i += step;
    }

    let seg_end = |out: &[Ph], k: usize| out.get(k + 1).is_none_or(|p| *p == Ph::Sep);
    // A final e is open (soré, kowé), except the ə of -isme; Javanese e
    // before a final h is open too (kabèh, akèh).
    let long = chars.len() > 2;
    let isme = word.ends_with("isme");
    for &k in &open_e {
        let final_e = seg_end(&out, k) && long && !isme;
        let before_final_h = jv && out.get(k + 1) == Some(&Ph::H) && seg_end(&out, k + 1);
        if final_e || before_final_h {
            out[k] = Ph::E;
        }
    }
    // Javanese a in a final open syllable is ɔ, and so is an a just before it.
    if jv {
        let mut k = 0;
        while k < out.len() {
            if out[k] == Ph::A && plain_a.contains(&k) && seg_end(&out, k) {
                out[k] = Ph::OpenO;
                let prev_vowel = (0..k).rev().take_while(|&j| out[j] != Ph::Sep).find(|&j| out[j].is_vowel());
                if let Some(j) = prev_vowel.filter(|&j| out[j] == Ph::A && plain_a.contains(&j)) {
                    out[j] = Ph::OpenO;
                }
            }
            k += 1;
        }
    }
    // k after a vowel and before a consonant or the end is a glottal stop
    // (tidak, rakyat), except before s, r, l (taksi, sakral).
    for k in 0..out.len() {
        if out[k] != Ph::K || k == 0 || !out[k - 1].is_vowel() {
            continue;
        }
        match out.get(k + 1) {
            None | Some(Ph::Sep) => out[k] = Ph::Glottal,
            Some(n) if !n.is_vowel() && !matches!(n, Ph::S | Ph::R | Ph::L) => out[k] = Ph::Glottal,
            _ => {}
        }
    }
    // Two equal vowels are split by a glottal stop (maaf = ma'af).
    let mut k = 1;
    while k < out.len() {
        if out[k].is_vowel() && out[k] == out[k - 1] {
            out.insert(k, Ph::Glottal);
            k += 1;
        }
        k += 1;
    }
    out
}

const FATHA: char = '\u{064E}';
const DAMMA: char = '\u{064F}';
const KASRA: char = '\u{0650}';
const SUKUN: char = '\u{0652}';
const SHADDA: char = '\u{0651}';

fn letter(p: Ph) -> &'static str {
    match p {
        Ph::B => "ب",
        Ph::P => "پ",
        Ph::T => "ت",
        Ph::D => "د",
        Ph::K => "ك",
        Ph::G => "ج",
        Ph::Glottal => "ء",
        Ph::C => "تْش",
        Ph::J => "چ",
        Ph::F | Ph::V => "ف",
        Ph::S => "س",
        Ph::Z => "ز",
        Ph::Sy => "ش",
        Ph::Kh => "خ",
        Ph::H => "ه",
        Ph::M => "م",
        Ph::N => "ن",
        Ph::Ng => "ڠ",
        Ph::Ny => "نْي",
        Ph::L => "ل",
        Ph::R => "ر",
        Ph::W => "و",
        Ph::Y => "ي",
        Ph::Dh => "ض",
        Ph::Th => "ط",
        _ => "",
    }
}

/// The vowel after a consonant (or carrier). `glide`: another vowel follows,
/// so i and u leave their letter to the glide (dia → دِيَا).
fn vowel_marks(v: Ph, glide: bool) -> String {
    match v {
        Ph::A => format!("{FATHA}ا"),
        Ph::I if glide => KASRA.to_string(),
        Ph::I => format!("{KASRA}ي"),
        Ph::U if glide => DAMMA.to_string(),
        Ph::U => format!("{DAMMA}و"),
        Ph::Schwa => KASRA.to_string(),
        Ph::E => "ێ".to_string(),
        Ph::O | Ph::OpenO => "ۆ".to_string(),
        _ => String::new(),
    }
}

/// Phones → Arabic script with full vowel marks.
pub fn arabic(phones: &[Ph]) -> String {
    phones.split(|p| *p == Ph::Sep).map(arabic_segment).collect::<Vec<_>>().join("-")
}

fn arabic_segment(seg: &[Ph]) -> String {
    let mut s = String::new();
    let mut k = 0;
    while k < seg.len() {
        let p = seg[k];
        let next = seg.get(k + 1).copied();
        if !p.is_vowel() {
            s.push_str(letter(p));
            k += 1;
            if next == Some(p) {
                // Doubled consonant (Allah): one letter with shadda.
                s.push(SHADDA);
                k += 1;
            }
            match seg.get(k).copied() {
                Some(v) if v.is_vowel() => {
                    k += write_vowel(&mut s, seg, k);
                }
                _ => s.push(SUKUN),
            }
            continue;
        }
        // A vowel with no consonant before it needs a carrier.
        if k == 0 {
            if p == Ph::A {
                s.push('آ');
                k += 1;
                k += diphthong(&mut s, seg, k);
                continue;
            }
            s.push(match p {
                Ph::U | Ph::O | Ph::OpenO => 'أ',
                _ => 'إ',
            });
        } else {
            let prev = seg[k - 1];
            s.push(match prev {
                Ph::I | Ph::E => 'ي',
                Ph::U | Ph::O | Ph::OpenO => 'و',
                _ => match p {
                    Ph::U | Ph::O | Ph::OpenO => 'ؤ',
                    Ph::A => 'ء',
                    _ => 'ئ',
                },
            });
        }
        k += write_vowel(&mut s, seg, k);
    }
    s
}

/// Writes the vowel at `k` (and a final diphthong glide); returns phones used.
fn write_vowel(s: &mut String, seg: &[Ph], k: usize) -> usize {
    let v = seg[k];
    // i before y and u before w lend their letter too (piye → پِيێ, nuwun → نُوُونْ).
    let glide =
        seg.get(k + 1).is_some_and(|&n| n.is_vowel() || (v, n) == (Ph::I, Ph::Y) || (v, n) == (Ph::U, Ph::W));
    let is_diphthong = diphthong_at(seg, k);
    s.push_str(&vowel_marks(v, glide && !is_diphthong));
    1 + diphthong(s, seg, k + 1)
}

/// A word-final ai / au / oi / ei is one syllable (pantai, pulau, amboi).
fn diphthong_at(seg: &[Ph], k: usize) -> bool {
    k + 2 == seg.len() && matches!(seg[k], Ph::A | Ph::O | Ph::E) && matches!(seg[k + 1], Ph::I | Ph::U)
}

fn diphthong(s: &mut String, seg: &[Ph], k: usize) -> usize {
    if k >= 1 && diphthong_at(seg, k - 1) {
        s.push(if seg[k] == Ph::I { 'ي' } else { 'و' });
        s.push(SUKUN);
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests;
