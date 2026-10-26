//! Minimal Pairs (spec §13): Javanese d/dh and t/th. Javanese d and t are
//! dental (close to Arabic د and ت); dh and th are made with the tongue tip
//! curled back. Two kinds of pair:
//! - `minimal`: two Javanese words that differ only in that sound (wedi
//!   "afraid" / wedhi "sand");
//! - `cross`: an Indonesian word and its Javanese cousin, where Javanese
//!   spells and says the curled sound (Indonesian dada / Javanese dhadha).
//!
//! Pairs are build-time content in `content/games/minimal-pairs.toml`, all
//! unverified until the learner approves them. A round shows one word's
//! meaning (or plays it, when a Javanese voice is available) and asks which
//! spelling it is.

use std::collections::HashSet;

use kabar_protocol::content::{Confidence, Trilingual};
use serde::Deserialize;

use super::Rng;

/// Rounds in one game.
pub const ROUND_LEN: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize)]
pub enum Contrast {
    #[serde(rename = "d_dh")]
    DDh,
    #[serde(rename = "t_th")]
    TTh,
}

impl Contrast {
    pub const ALL: [Self; 2] = [Self::DDh, Self::TTh];

    /// The plain (dental) letter: `d` or `t`.
    pub fn plain(self) -> &'static str {
        match self {
            Self::DDh => "d",
            Self::TTh => "t",
        }
    }

    /// The curled (retroflex) spelling: `dh` or `th`.
    pub fn curled(self) -> &'static str {
        match self {
            Self::DDh => "dh",
            Self::TTh => "th",
        }
    }

    /// The activity-log topic: `d-dh` or `t-th`.
    pub fn topic(self) -> &'static str {
        match self {
            Self::DDh => "d-dh",
            Self::TTh => "t-th",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairKind {
    Minimal,
    Cross,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairWord {
    pub word: String,
    /// `jv` or `id`.
    pub lang: String,
    pub meaning: Trilingual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    /// `mp.wedi-wedhi`
    pub id: String,
    pub contrast: Contrast,
    pub kind: PairKind,
    /// The plain (dental) side: d or t.
    pub plain: PairWord,
    /// The curled (retroflex) side: dh or th.
    pub curled: PairWord,
    /// Javanese track level it fits: `j1`, `j2` or `j3`.
    pub level: String,
    pub confidence: Confidence,
    pub note: Option<Trilingual>,
}

impl Pair {
    pub fn word(&self, side: Side) -> &PairWord {
        match side {
            Side::Plain => &self.plain,
            Side::Curled => &self.curled,
        }
    }

    /// `side`'s spelling cut into runs, `true` for the letters that carry
    /// the contrast: each `dh`/`th` of the curled word and the matching
    /// `d`/`t` of the plain one. `dhadha` gives `[dh] a [dh] a` and `dada`
    /// gives `[d] a [d] a`. Relies on the parse rule that the plain word is
    /// the curled one with every `dh`/`th` written `d`/`t`.
    pub fn segments(&self, side: Side) -> Vec<(String, bool)> {
        let curled = self.curled.word.as_str();
        let digraph = self.contrast.curled();
        let single = self.contrast.plain();
        let mut out: Vec<(String, bool)> = Vec::new();
        let mut push = |text: &str, mark: bool| match out.last_mut() {
            Some((t, false)) if !mark => t.push_str(text),
            _ => out.push((text.to_string(), mark)),
        };
        let mut rest = curled;
        while let Some(at) = rest.find(digraph) {
            push(&rest[..at], false);
            push(if side == Side::Curled { digraph } else { single }, true);
            rest = &rest[at + digraph.len()..];
        }
        push(rest, false);
        out.retain(|(t, _)| !t.is_empty());
        out
    }
}

/// How to make each sound, for an Egyptian Arabic speaker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    pub contrast: Contrast,
    pub text: Trilingual,
}

#[derive(Clone, Debug, Default)]
pub struct PairsData {
    pub pairs: Vec<Pair>,
    pub tips: Vec<Tip>,
    pub reviewed: bool,
    pub version: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileToml {
    reviewed: bool,
    version: u32,
    #[serde(default)]
    tips: Vec<TipToml>,
    #[serde(default)]
    pairs: Vec<PairToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TipToml {
    contrast: Contrast,
    arz: String,
    ar: String,
    en: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairToml {
    id: String,
    contrast: Contrast,
    kind: PairKind,
    plain: WordToml,
    curled: WordToml,
    level: String,
    confidence: Confidence,
    #[serde(default)]
    note: Option<Trilingual>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WordToml {
    word: String,
    lang: String,
    meaning: Trilingual,
}

fn check_text(what: &str, t: &Trilingual) -> Result<(), String> {
    for (lang, s) in [("arz", &t.arz), ("ar", &t.ar), ("en", &t.en)] {
        if s.trim().is_empty() {
            return Err(format!("{what}: empty {lang} text"));
        }
    }
    Ok(())
}

impl WordToml {
    fn into_word(self, id: &str, side: &str) -> Result<PairWord, String> {
        let word = self.word;
        if word.trim().is_empty() || word != word.trim().to_lowercase() {
            return Err(format!(
                "{id}: the {side} word {word:?} must be lowercase, with no spaces around it"
            ));
        }
        if !matches!(self.lang.as_str(), "jv" | "id") {
            return Err(format!("{id}: the {side} word's lang must be jv or id, found {:?}", self.lang));
        }
        check_text(&format!("{id} {side} meaning"), &self.meaning)?;
        Ok(PairWord { word, lang: self.lang, meaning: self.meaning })
    }
}

impl PairToml {
    fn into_pair(self) -> Result<Pair, String> {
        let id = self.id;
        let c = self.contrast;
        let plain = self.plain.into_word(&id, "plain")?;
        let curled = self.curled.into_word(&id, "curled")?;
        let expected = format!("mp.{}-{}", plain.word, curled.word);
        if id != expected {
            return Err(format!("{id}: the id must be {expected}"));
        }
        if !curled.word.contains(c.curled()) || curled.word.replace(c.curled(), c.plain()) != plain.word {
            return Err(format!(
                "{id}: {:?} must be {:?} with the {} written {}",
                curled.word,
                plain.word,
                c.plain(),
                c.curled()
            ));
        }
        if curled.lang != "jv" {
            return Err(format!("{id}: the curled side is always Javanese"));
        }
        match self.kind {
            PairKind::Minimal if plain.lang != "jv" => {
                return Err(format!("{id}: a minimal pair is two Javanese words"));
            }
            PairKind::Cross if plain.lang != "id" => {
                return Err(format!("{id}: a cross pair's plain side is Indonesian"));
            }
            _ => {}
        }
        if !matches!(self.level.as_str(), "j1" | "j2" | "j3") {
            return Err(format!("{id}: level must be j1, j2 or j3, found {:?}", self.level));
        }
        if let Some(note) = &self.note {
            check_text(&format!("{id} note"), note)?;
        }
        Ok(Pair {
            id,
            contrast: c,
            kind: self.kind,
            plain,
            curled,
            level: self.level,
            confidence: self.confidence,
            note: self.note,
        })
    }
}

impl PairsData {
    /// Parses `content/games/minimal-pairs.toml`. Rejects duplicate ids, ids
    /// not of the form `mp.<plain>-<curled>`, a side whose spelling doesn't
    /// show the contrast (the curled side must contain `dh`/`th` where the
    /// plain side has `d`/`t`), and a `minimal` pair whose sides aren't both
    /// Javanese.
    ///
    /// Also: the curled side is Javanese and a `cross` pair's plain side is
    /// Indonesian; words are lowercase; levels are `j1`–`j3`; every text has
    /// all three languages; each contrast that has pairs has exactly one tip.
    pub fn parse(toml: &str) -> Result<Self, String> {
        let file: FileToml = toml::from_str(toml).map_err(|e| format!("minimal-pairs.toml: {e}"))?;
        let mut ids = HashSet::new();
        let mut pairs: Vec<Pair> = Vec::with_capacity(file.pairs.len());
        for p in file.pairs {
            if !ids.insert(p.id.clone()) {
                return Err(format!("minimal-pairs.toml: duplicate pair {}", p.id));
            }
            pairs.push(p.into_pair()?);
        }
        let mut tips: Vec<Tip> = Vec::with_capacity(file.tips.len());
        for t in file.tips {
            if tips.iter().any(|x| x.contrast == t.contrast) {
                return Err(format!("minimal-pairs.toml: two tips for {}", t.contrast.topic()));
            }
            let text = Trilingual { arz: t.arz, ar: t.ar, en: t.en };
            check_text(&format!("tip {}", t.contrast.topic()), &text)?;
            tips.push(Tip { contrast: t.contrast, text });
        }
        for c in Contrast::ALL {
            if pairs.iter().any(|p| p.contrast == c) && !tips.iter().any(|t| t.contrast == c) {
                return Err(format!("minimal-pairs.toml: no tip for {}", c.topic()));
            }
        }
        Ok(Self { pairs, tips, reviewed: file.reviewed, version: file.version })
    }

    pub fn tip(&self, contrast: Contrast) -> Option<&Tip> {
        self.tips.iter().find(|t| t.contrast == contrast)
    }

    pub fn pair(&self, id: &str) -> Option<&Pair> {
        self.pairs.iter().find(|p| p.id == id)
    }
}

/// Which side of the pair the round is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Plain,
    Curled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round {
    pub pair: Pair,
    pub target: Side,
    /// The two spellings in display order.
    pub options: [Side; 2],
}

impl Round {
    pub fn word(&self, side: Side) -> &PairWord {
        self.pair.word(side)
    }

    pub fn target_word(&self) -> &PairWord {
        self.word(self.target)
    }

    pub fn check(&self, picked: Side) -> bool {
        picked == self.target
    }
}

/// Up to `n` rounds, optionally for one contrast, deterministic for the seed.
/// Each pair appears at most once per game; target side and option order
/// are random. `minimal` pairs are used first; `cross` pairs fill the rest.
/// The chosen rounds are then played in shuffled order, so the kinds mix.
pub fn rounds(data: &PairsData, contrast: Option<Contrast>, n: usize, rng: &mut Rng) -> Vec<Round> {
    let of_kind = |kind: PairKind| -> Vec<&Pair> {
        data.pairs.iter().filter(|p| p.kind == kind && contrast.is_none_or(|c| p.contrast == c)).collect()
    };
    let (mut minimal, mut cross) = (of_kind(PairKind::Minimal), of_kind(PairKind::Cross));
    rng.shuffle(&mut minimal);
    rng.shuffle(&mut cross);
    let mut chosen: Vec<&Pair> = minimal.into_iter().chain(cross).take(n).collect();
    rng.shuffle(&mut chosen);
    chosen
        .into_iter()
        .map(|pair| {
            let target = if rng.below(2) == 0 { Side::Plain } else { Side::Curled };
            let options =
                if rng.below(2) == 0 { [Side::Plain, Side::Curled] } else { [Side::Curled, Side::Plain] };
            Round { pair: pair.clone(), target, options }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("../../../../content/games/minimal-pairs.toml");

    fn data() -> PairsData {
        PairsData::parse(SOURCE).expect("content/games/minimal-pairs.toml parses")
    }

    const TIP: &str = r#"
[[tips]]
contrast = "d_dh"
arz = "ا"
ar = "ا"
en = "a"
"#;

    fn pair(id: &str, contrast: &str, kind: &str, plain: (&str, &str), curled: (&str, &str)) -> String {
        format!(
            r#"
[[pairs]]
id = "{id}"
contrast = "{contrast}"
kind = "{kind}"
plain = {{ word = "{}", lang = "{}", meaning = {{ arz = "س", ar = "س", en = "x" }} }}
curled = {{ word = "{}", lang = "{}", meaning = {{ arz = "س", ar = "س", en = "y" }} }}
level = "j1"
confidence = "high"
"#,
            plain.0, plain.1, curled.0, curled.1
        )
    }

    fn file(pairs: &[String]) -> String {
        format!("reviewed = false\nversion = 1\n{TIP}{}", pairs.concat())
    }

    fn wedi() -> String {
        pair("mp.wedi-wedhi", "d_dh", "minimal", ("wedi", "jv"), ("wedhi", "jv"))
    }

    #[test]
    fn parse_accepts_a_good_file() {
        let d = PairsData::parse(&file(&[
            wedi(),
            pair("mp.dada-dhadha", "d_dh", "cross", ("dada", "id"), ("dhadha", "jv")),
        ]))
        .unwrap();
        assert_eq!(d.pairs.len(), 2);
        assert_eq!(d.pairs[0].kind, PairKind::Minimal);
        assert!(d.tip(Contrast::DDh).is_some());
        assert!(d.tip(Contrast::TTh).is_none());
        assert!(!d.reviewed);
    }

    #[test]
    fn parse_rejects_bad_content() {
        let bad = |pairs: &[String]| PairsData::parse(&file(pairs)).unwrap_err();
        // Unknown fields and syntax.
        assert!(PairsData::parse("nope = 1").is_err());
        assert!(
            PairsData::parse(&file(&[wedi().replace("level = \"j1\"", "level = \"j1\"\nextra = 1")]))
                .is_err()
        );
        assert!(PairsData::parse(&file(&[wedi().replace("\"minimal\"", "\"near\"")])).is_err());
        // Duplicate id.
        assert!(bad(&[wedi(), wedi()]).contains("duplicate"));
        // The id must name both words.
        assert!(
            bad(&[pair("mp.wedhi-wedi", "d_dh", "minimal", ("wedi", "jv"), ("wedhi", "jv"))]).contains("id")
        );
        assert!(
            bad(&[pair("wedi-wedhi", "d_dh", "minimal", ("wedi", "jv"), ("wedhi", "jv"))]).contains("id")
        );
        // The spellings must show the contrast.
        assert!(!bad(&[pair("mp.wedi-wedi", "d_dh", "minimal", ("wedi", "jv"), ("wedi", "jv"))]).is_empty());
        assert!(
            !bad(&[pair("mp.wedi-wedhu", "d_dh", "minimal", ("wedi", "jv"), ("wedhu", "jv"))]).is_empty()
        );
        assert!(
            !bad(&[pair("mp.wedi-wedhi", "t_th", "minimal", ("wedi", "jv"), ("wedhi", "jv"))]).is_empty()
        );
        // A minimal pair is two Javanese words; the curled side is Javanese.
        assert!(
            bad(&[pair("mp.wedi-wedhi", "d_dh", "minimal", ("wedi", "id"), ("wedhi", "jv"))])
                .contains("minimal")
        );
        assert!(
            !bad(&[pair("mp.dada-dhadha", "d_dh", "cross", ("dada", "jv"), ("dhadha", "id"))]).is_empty()
        );
        assert!(
            !bad(&[pair("mp.dada-dhadha", "d_dh", "cross", ("dada", "jv"), ("dhadha", "jv"))]).is_empty()
        );
        // Languages, levels, case, empty text.
        assert!(
            !bad(&[pair("mp.dada-dhadha", "d_dh", "cross", ("dada", "ms"), ("dhadha", "jv"))]).is_empty()
        );
        assert!(bad(&[wedi().replace("\"j1\"", "\"b1\"")]).contains("level"));
        assert!(
            !bad(&[pair("mp.Wedi-Wedhi", "d_dh", "minimal", ("Wedi", "jv"), ("Wedhi", "jv"))]).is_empty()
        );
        assert!(bad(&[wedi().replacen("en = \"x\"", "en = \" \"", 1)]).contains("empty"));
        // A contrast with pairs needs its tip.
        assert!(
            bad(&[pair("mp.pati-pathi", "t_th", "minimal", ("pati", "jv"), ("pathi", "jv"))])
                .contains("no tip")
        );
        let two_tips = format!("reviewed = false\nversion = 1\n{TIP}{TIP}");
        assert!(PairsData::parse(&two_tips).unwrap_err().contains("two tips"));
    }

    fn fixture() -> PairsData {
        let mut pairs = Vec::new();
        for (p, c) in [("wedi", "wedhi"), ("adi", "adhi")] {
            pairs.push(pair(&format!("mp.{p}-{c}"), "d_dh", "minimal", (p, "jv"), (c, "jv")));
        }
        for (p, c) in [("dada", "dhadha"), ("beda", "bedha"), ("gede", "gedhe"), ("sedih", "sedhih")] {
            pairs.push(pair(&format!("mp.{p}-{c}"), "d_dh", "cross", (p, "id"), (c, "jv")));
        }
        pairs.push(pair("mp.pati-pathi", "t_th", "minimal", ("pati", "jv"), ("pathi", "jv")));
        pairs.push(pair("mp.batik-bathik", "t_th", "cross", ("batik", "id"), ("bathik", "jv")));
        let tth = TIP.replace("d_dh", "t_th");
        PairsData::parse(&format!("{}{tth}", file(&pairs))).unwrap()
    }

    #[test]
    fn rounds_are_deterministic_for_a_seed() {
        let d = fixture();
        let a = rounds(&d, None, ROUND_LEN, &mut Rng::new(42));
        assert_eq!(a, rounds(&d, None, ROUND_LEN, &mut Rng::new(42)));
        let differ = (0..20u64).any(|s| rounds(&d, None, ROUND_LEN, &mut Rng::new(s)) != a);
        assert!(differ, "different seeds give different games");
        // Both sides and both option orders turn up.
        let all: Vec<Round> =
            (0..20u64).flat_map(|s| rounds(&d, None, ROUND_LEN, &mut Rng::new(s))).collect();
        assert!(all.iter().any(|r| r.target == Side::Plain));
        assert!(all.iter().any(|r| r.target == Side::Curled));
        assert!(all.iter().any(|r| r.options[0] == Side::Curled));
        assert!(all.iter().all(|r| r.options[0] != r.options[1]));
        assert!(all.iter().all(|r| r.check(r.target) && r.target_word() == r.pair.word(r.target)));
    }

    #[test]
    fn no_pair_repeats_in_a_game() {
        let d = fixture();
        for seed in 0..50 {
            let r = rounds(&d, None, ROUND_LEN, &mut Rng::new(seed));
            assert_eq!(r.len(), d.pairs.len(), "fewer pairs than rounds: one round each");
            let ids: HashSet<&str> = r.iter().map(|r| r.pair.id.as_str()).collect();
            assert_eq!(ids.len(), r.len());
        }
        assert!(rounds(&d, None, 0, &mut Rng::new(1)).is_empty());
        assert!(rounds(&PairsData::default(), None, ROUND_LEN, &mut Rng::new(1)).is_empty());
    }

    #[test]
    fn minimal_pairs_come_before_cross_pairs() {
        let d = fixture();
        for seed in 0..50 {
            // Three d/dh rounds: both minimal pairs, then one cross pair.
            let r = rounds(&d, Some(Contrast::DDh), 3, &mut Rng::new(seed));
            assert_eq!(r.len(), 3);
            assert_eq!(r.iter().filter(|r| r.pair.kind == PairKind::Minimal).count(), 2);
            // One round: always a minimal pair.
            let r = rounds(&d, Some(Contrast::DDh), 1, &mut Rng::new(seed));
            assert_eq!(r[0].pair.kind, PairKind::Minimal);
        }
    }

    #[test]
    fn a_contrast_filters_the_pairs() {
        let d = fixture();
        for seed in 0..20 {
            let r = rounds(&d, Some(Contrast::TTh), ROUND_LEN, &mut Rng::new(seed));
            assert_eq!(r.len(), 2);
            assert!(r.iter().all(|r| r.pair.contrast == Contrast::TTh));
            let r = rounds(&d, Some(Contrast::DDh), ROUND_LEN, &mut Rng::new(seed));
            assert_eq!(r.len(), 6);
            assert!(r.iter().all(|r| r.pair.contrast == Contrast::DDh));
        }
    }

    #[test]
    fn segments_mark_the_contrast_letters() {
        let d = fixture();
        let seg = |id: &str, side| d.pair(id).unwrap().segments(side);
        let s = |t: &str, m| (t.to_string(), m);
        assert_eq!(
            seg("mp.dada-dhadha", Side::Curled),
            [s("dh", true), s("a", false), s("dh", true), s("a", false)]
        );
        assert_eq!(
            seg("mp.dada-dhadha", Side::Plain),
            [s("d", true), s("a", false), s("d", true), s("a", false)]
        );
        assert_eq!(seg("mp.wedi-wedhi", Side::Plain), [s("we", false), s("d", true), s("i", false)]);
        assert_eq!(seg("mp.batik-bathik", Side::Curled), [s("ba", false), s("th", true), s("ik", false)]);
        for p in &d.pairs {
            for side in [Side::Plain, Side::Curled] {
                let joined: String = p.segments(side).into_iter().map(|(t, _)| t).collect();
                assert_eq!(joined, p.word(side).word);
            }
        }
    }

    fn is_arabic_letter(c: char) -> bool {
        ('\u{0600}'..='\u{06FF}').contains(&c)
    }

    /// Latin letters in Arabic text sit inside FSI … PDI.
    fn isolation_errors(v: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let mut depth = 0i32;
        for c in v.chars() {
            match c {
                '\u{2068}' => depth += 1,
                '\u{2069}' => {
                    depth -= 1;
                    if depth < 0 {
                        errors.push("PDI without FSI".into());
                    }
                }
                '\u{202A}'..='\u{202E}' | '\u{2066}' | '\u{2067}' => {
                    errors.push(format!("use FSI/PDI, not U+{:04X}", c as u32))
                }
                c if c.is_ascii_alphabetic() && depth == 0 => {
                    errors.push(format!("Latin {c:?} outside FSI…PDI"))
                }
                _ => {}
            }
        }
        if depth != 0 {
            errors.push("unbalanced FSI/PDI".into());
        }
        errors
    }

    fn check_text(what: &str, t: &Trilingual) {
        for (lang, s) in [("arz", &t.arz), ("ar", &t.ar)] {
            assert!(s.chars().any(is_arabic_letter), "{what} {lang}: no Arabic in {s:?}");
            assert_eq!(isolation_errors(s), Vec::<String>::new(), "{what} {lang}: {s:?}");
        }
        assert!(t.en.chars().any(|c| c.is_ascii_alphabetic()), "{what} en: {:?}", t.en);
    }

    #[test]
    fn the_shipped_content_parses() {
        let d = data();
        assert!(!d.reviewed, "stays unverified until the learner approves it");
        assert!(d.version >= 1);
        for c in Contrast::ALL {
            let n = d.pairs.iter().filter(|p| p.contrast == c).count();
            assert!((8..=20).contains(&n), "{}: {n} pairs", c.topic());
            assert!(d.pairs.iter().any(|p| p.contrast == c && p.kind == PairKind::Minimal), "{}", c.topic());
            assert!(d.pairs.iter().any(|p| p.contrast == c && p.level == "j1"), "{}", c.topic());
            let tip = d.tip(c).unwrap_or_else(|| panic!("tip for {}", c.topic()));
            check_text(&format!("tip {}", c.topic()), &tip.text);
            // The tips don't claim dh/th are ض/ط.
            for s in [&tip.text.arz, &tip.text.ar] {
                assert!(s.contains('د') || s.contains('ت'), "{s}");
            }
        }
        assert!(d.pair("mp.wedi-wedhi").is_some(), "the classic pair");
        for p in &d.pairs {
            check_text(&format!("{} plain", p.id), &p.plain.meaning);
            check_text(&format!("{} curled", p.id), &p.curled.meaning);
            if let Some(n) = &p.note {
                check_text(&format!("{} note", p.id), n);
            }
        }
        // Every game for every choice fills ten rounds.
        for c in [None, Some(Contrast::DDh), Some(Contrast::TTh)] {
            assert_eq!(rounds(&d, c, ROUND_LEN, &mut Rng::new(7)).len(), ROUND_LEN);
        }
    }
}
