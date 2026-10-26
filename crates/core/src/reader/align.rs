//! Split-mode alignment (spec §11): link each source word to a span of the
//! translation, so tapping either side highlights the other.
//!
//! The AI's `analyze_text` answer glosses every word (`AlignedToken.i` is the
//! index into the input `words` list, i.e. the word-like tokens of the text in
//! order). Here we find each gloss inside the full translation:
//!
//! 1. Candidates: the whole gloss without parenthetical notes, then each
//!    alternative split on `/ , ، ; ؛ |` (English also drops a leading
//!    "the / a / an / to").
//! 2. Matching on a normalized copy of the translation: case-folded, Arabic
//!    tashkeel and tatweel stripped, alef forms unified, ta marbuta → ha,
//!    alef maqsura → ya. Matches must sit on word boundaries.
//! 3. Arabic only, as a last resort: single words compared after stripping
//!    one common prefix (article, conjunction, Egyptian future/present
//!    markers), so "أبلّغ" finds "هبلّغ".
//!
//! Each translation span is used at most once, and a match at or after the
//! previous one is preferred (left-to-right monotonic) over an earlier one.

use serde::{Deserialize, Serialize};

use crate::ai::tasks::AnalyzeText;
use crate::lang::token::tokenize;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// Index into the `words` list (= `AlignedToken.i`).
    pub word: usize,
    /// Byte span of the word in the source text.
    pub source: (usize, usize),
    /// Byte span inside the translation, when the gloss was found there.
    pub target: Option<(usize, usize)>,
}

/// Byte spans of the word-like tokens of `source`, in order: the `words`
/// list sent to `analyze_text`.
pub fn word_spans(source: &str) -> Vec<(usize, usize)> {
    tokenize(source).into_iter().filter(|t| t.is_wordlike()).map(|t| (t.start, t.end)).collect()
}

/// One link per entry of `words`, in order. `target` spans index into
/// `analysis.translation_ar` when `arabic`, else `analysis.translation_en`.
/// `source` is accepted for symmetry with the UI; spans come from `words`.
pub fn align(source: &str, words: &[(usize, usize)], analysis: &AnalyzeText, arabic: bool) -> Vec<Link> {
    debug_assert!(words.iter().all(|&(s, e)| source.is_char_boundary(s) && source.is_char_boundary(e)));
    let translation = if arabic { &analysis.translation_ar } else { &analysis.translation_en };
    let hay = Folded::new(translation);
    let hay_words = word_ranges(&hay.chars);

    let mut targets: Vec<Option<(usize, usize)>> = vec![None; words.len()];
    let mut used: Vec<(usize, usize)> = Vec::new();
    let mut cursor = 0;

    let mut tokens: Vec<_> = analysis.tokens.iter().filter(|t| t.i < words.len()).collect();
    tokens.sort_by_key(|t| t.i);
    for tok in tokens {
        let gloss = if arabic { &tok.gloss_ar } else { &tok.gloss_en };
        let mut lists: Vec<Vec<(usize, usize)>> = Vec::new();
        let cands: Vec<Vec<char>> = candidates(gloss, !arabic).iter().map(|c| Folded::new(c).chars).collect();
        for c in &cands {
            lists.push(find_all(&hay.chars, c));
        }
        if arabic {
            for c in cands.iter().filter(|c| !c.iter().any(|ch| ch.is_whitespace())) {
                let s = stem(c);
                if s.len() >= 3 {
                    lists.push(
                        hay_words.iter().copied().filter(|&(a, b)| stem(&hay.chars[a..b]) == s).collect(),
                    );
                }
            }
        }

        let free =
            |o: &(usize, usize), used: &[(usize, usize)]| used.iter().all(|u| o.1 <= u.0 || o.0 >= u.1);
        let pick = [true, false].into_iter().find_map(|monotonic| {
            lists.iter().flatten().copied().find(|o| free(o, &used) && (!monotonic || o.0 >= cursor))
        });
        if let Some(o) = pick {
            used.push(o);
            cursor = o.1;
            targets[tok.i] = Some(hay.byte_span(o));
        }
    }

    words
        .iter()
        .zip(targets)
        .enumerate()
        .map(|(word, (&source, target))| Link { word, source, target })
        .collect()
}

/// A case- and diacritic-folded copy of a string, remembering which byte
/// span of the original each folded char came from.
struct Folded {
    chars: Vec<char>,
    spans: Vec<(usize, usize)>,
}

impl Folded {
    fn new(s: &str) -> Self {
        let mut chars = Vec::new();
        let mut spans: Vec<(usize, usize)> = Vec::new();
        for (b, c) in s.char_indices() {
            let end = b + c.len_utf8();
            if is_dropped(c) {
                // Keep a trailing shadda/harakah inside the previous letter's span.
                if let Some(last) = spans.last_mut() {
                    last.1 = end;
                }
                continue;
            }
            for f in fold(c) {
                chars.push(f);
                spans.push((b, end));
            }
        }
        Self { chars, spans }
    }

    /// Byte span in the original string of folded chars `a..b` (non-empty).
    fn byte_span(&self, (a, b): (usize, usize)) -> (usize, usize) {
        (self.spans[a].0, self.spans[b - 1].1)
    }
}

/// Tashkeel (harakat, shadda, sukun, Quranic marks, superscript alef) and tatweel.
fn is_dropped(c: char) -> bool {
    matches!(c, '\u{0610}'..='\u{061A}' | '\u{064B}'..='\u{065F}' | '\u{0670}' | '\u{06D6}'..='\u{06ED}' | '\u{0640}')
}

fn fold(c: char) -> impl Iterator<Item = char> {
    let one = match c {
        'أ' | 'إ' | 'آ' | 'ٱ' => Some('ا'),
        'ة' => Some('ه'),
        'ى' => Some('ي'),
        '’' | '‘' | 'ʼ' => Some('\''),
        _ => None,
    };
    let lower = if one.is_none() { Some(c.to_lowercase()) } else { None };
    one.into_iter().chain(lower.into_iter().flatten())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// Gloss alternatives, best first, de-duplicated.
fn candidates(gloss: &str, english: bool) -> Vec<String> {
    let bare = strip_parens(gloss);
    let mut raw: Vec<String> = std::iter::once(bare.as_str())
        .chain(bare.split(['/', ',', '،', ';', '؛', '|']))
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    if english {
        for i in 0..raw.len() {
            for article in ["the ", "a ", "an ", "to "] {
                let c = &raw[i];
                if c.get(..article.len()).is_some_and(|p| p.eq_ignore_ascii_case(article)) {
                    raw.push(c[article.len()..].to_string());
                }
            }
        }
    }
    let mut out: Vec<String> = Vec::new();
    for s in raw {
        if s.chars().any(is_word_char) && !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

fn strip_parens(s: &str) -> String {
    let mut depth = 0u32;
    s.chars()
        .filter(|&c| match c {
            '(' => {
                depth += 1;
                false
            }
            ')' => {
                depth = depth.saturating_sub(1);
                false
            }
            _ => depth == 0,
        })
        .collect()
}

/// Every occurrence of `needle` in `hay` on word boundaries, as char ranges.
fn find_all(hay: &[char], needle: &[char]) -> Vec<(usize, usize)> {
    if needle.is_empty() || needle.len() > hay.len() {
        return Vec::new();
    }
    (0..=hay.len() - needle.len())
        .filter(|&i| {
            let j = i + needle.len();
            hay[i..j] == *needle
                && (i == 0 || !is_word_char(hay[i - 1]) || !is_word_char(needle[0]))
                && (j == hay.len() || !is_word_char(hay[j]) || !is_word_char(needle[needle.len() - 1]))
        })
        .map(|i| (i, i + needle.len()))
        .collect()
}

/// Char ranges of the maximal runs of word chars.
fn word_ranges(chars: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, &c) in chars.iter().enumerate() {
        match (is_word_char(c), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, chars.len()));
    }
    out
}

/// Strip one common Arabic prefix (folded forms), keeping at least three
/// letters: article and conjunctions (وال بال فال لل ال و ف ب ل) and the
/// Egyptian verb markers (ه ح future, ب present, ا ي ت ن person prefixes).
fn stem(word: &[char]) -> Vec<char> {
    const PREFIXES: [&str; 15] =
        ["وال", "بال", "فال", "لل", "ال", "و", "ف", "ب", "ل", "ه", "ح", "ا", "ي", "ت", "ن"];
    for p in PREFIXES {
        let p: Vec<char> = p.chars().collect();
        if word.starts_with(&p) && word.len() >= p.len() + 3 {
            return word[p.len()..].to_vec();
        }
    }
    word.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(json: &str) -> AnalyzeText {
        let envelope: serde_json::Value = serde_json::from_str(json).unwrap();
        serde_json::from_str(envelope["text"].as_str().unwrap()).unwrap()
    }

    fn gemini() -> AnalyzeText {
        fixture(include_str!("../../../../fixtures/ai/analyze_text.sek_tak_lapor_pusat.gemini_free.json"))
    }

    fn slice(s: &str, span: Option<(usize, usize)>) -> Option<&str> {
        span.map(|(a, b)| &s[a..b])
    }

    fn token(i: usize, surface: &str, en: &str, ar: &str) -> serde_json::Value {
        serde_json::json!({"i": i, "surface": surface, "lang": "jv", "lemma": surface,
            "gloss_en": en, "gloss_ar": ar, "note": null, "confident": true})
    }

    fn analysis(en: &str, ar: &str, tokens: Vec<serde_json::Value>) -> AnalyzeText {
        serde_json::from_value(serde_json::json!({
            "translation_en": en, "translation_ar": ar, "register": "chat", "tokens": tokens
        }))
        .unwrap()
    }

    #[test]
    fn word_spans_skip_punctuation() {
        let s = "ng prabowo?";
        let spans = word_spans(s);
        assert_eq!(spans.iter().map(|&(a, b)| &s[a..b]).collect::<Vec<_>>(), ["ng", "prabowo"]);
        let s = "iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan minat wa";
        let words: Vec<_> = word_spans(s).iter().map(|&(a, b)| &s[a..b]).collect();
        assert!(words.contains(&"512gb") && words.contains(&"minat") && !words.contains(&"17"));
    }

    #[test]
    fn arabic_alignment_from_recorded_fixture() {
        let src = "Sek tak lapor pusat";
        let words = word_spans(src);
        let a = gemini();
        let links = align(src, &words, &a, true);
        let ar = &a.translation_ar;
        assert_eq!(links.len(), 4);
        assert_eq!(
            links.iter().map(|l| &src[l.source.0..l.source.1]).collect::<Vec<_>>(),
            ["Sek", "tak", "lapor", "pusat"]
        );
        assert_eq!(slice(ar, links[0].target), Some("استنى"));
        // "هـ (أنا هعمل)" is a prefix gloss: nothing standalone to point at.
        assert_eq!(links[1].target, None);
        // Gloss "أبلّغ" vs translation "هبلّغ": found through the prefix-stripped stem.
        assert_eq!(slice(ar, links[2].target), Some("هبلّغ"));
        // Gloss "الإدارة / المركز": the first alternative is in the translation.
        assert_eq!(slice(ar, links[3].target), Some("الإدارة"));
    }

    #[test]
    fn english_alignment_from_recorded_fixture() {
        let src = "Sek tak lapor pusat";
        let a = gemini();
        let links = align(src, &word_spans(src), &a, false);
        let en = &a.translation_en;
        // "wait" is not in "Hold on, I'll report to HQ.": honest None.
        assert_eq!(links[0].target, None);
        assert_eq!(slice(en, links[1].target), Some("I'll"));
        assert_eq!(slice(en, links[2].target), Some("report"));
        assert_eq!(links[3].target, None);
    }

    #[test]
    fn spans_are_used_once_and_prefer_monotonic() {
        let src = "piye piye dhe";
        let a = analysis(
            "how, how, uncle?",
            "إزاي، إزاي يا عمو؟",
            vec![
                token(0, "piye", "how", "إزاي"),
                token(1, "piye", "How", "ازاى"),
                token(2, "dhe", "uncle", "عمّو"),
            ],
        );
        let words = word_spans(src);
        let en = align(src, &words, &a, false);
        assert_eq!(
            en.iter().map(|l| l.target).collect::<Vec<_>>(),
            [Some((0, 3)), Some((5, 8)), Some((10, 15))]
        );
        let ar = align(src, &words, &a, true);
        let t = &a.translation_ar;
        let (s0, s1) = (ar[0].target.unwrap(), ar[1].target.unwrap());
        assert!(s0.1 <= s1.0, "second piye maps after the first: {s0:?} {s1:?}");
        assert_eq!(&t[s1.0..s1.1], "إزاي");
        assert_eq!(slice(t, ar[2].target), Some("عمو"));
    }

    #[test]
    fn falls_back_to_earlier_span_when_order_differs() {
        // Arabic puts the adjective after the noun; English glosses still align.
        let src = "rumah besar";
        let a = analysis(
            "a big house",
            "بيت كبير",
            vec![token(0, "rumah", "house", "بيت"), token(1, "besar", "big", "كبير")],
        );
        let links = align(src, &word_spans(src), &a, false);
        assert_eq!(slice(&a.translation_en, links[0].target), Some("house"));
        assert_eq!(slice(&a.translation_en, links[1].target), Some("big"));
    }

    #[test]
    fn english_matches_respect_word_boundaries_and_articles() {
        let src = "arp dicul piro";
        let a = analysis(
            "How much will you sell it for?",
            "هتبيعه بكام؟",
            vec![
                token(0, "arp", "will", "ه"),
                token(1, "dicul", "to sell", "تبيع"),
                token(2, "piro", "how much", "بكام"),
            ],
        );
        let links = align(src, &word_spans(src), &a, false);
        let en = &a.translation_en;
        assert_eq!(slice(en, links[0].target), Some("will"));
        assert_eq!(slice(en, links[1].target), Some("sell"));
        assert_eq!(slice(en, links[2].target), Some("How much"));
        let ar = align(src, &word_spans(src), &a, true);
        assert_eq!(slice(&a.translation_ar, ar[2].target), Some("بكام"));
    }

    #[test]
    fn spans_are_char_boundaries_and_tokens_out_of_range_are_ignored() {
        let src = "Sek tak";
        let a = analysis(
            "Wait, I'll",
            "اِسْتَنَّى، هـ",
            vec![token(0, "Sek", "wait", "استنى"), token(7, "x", "x", "x")],
        );
        let links = align(src, &word_spans(src), &a, true);
        assert_eq!(links.len(), 2);
        let (s, e) = links[0].target.unwrap();
        assert!(a.translation_ar.is_char_boundary(s) && a.translation_ar.is_char_boundary(e));
        // Tashkeel inside and after the word stays in the span.
        assert_eq!(&a.translation_ar[s..e], "اِسْتَنَّى");
        assert_eq!(links[1].target, None);
    }

    #[test]
    fn folding_unifies_arabic_letter_forms() {
        assert_eq!(Folded::new("إدارة").chars, Folded::new("اداره").chars);
        assert_eq!(Folded::new("مستشفى").chars, Folded::new("مستشفي").chars);
        assert_eq!(Folded::new("كـبـير").chars, Folded::new("كبير").chars);
    }
}
