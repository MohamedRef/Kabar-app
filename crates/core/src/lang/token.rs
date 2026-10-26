//! Tokenizer for chat-style Indonesian, Javanese and English mixed together.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    /// Letters, possibly with internal hyphens (anak-anak) or apostrophes.
    Word,
    /// Letters and digits mixed, e.g. "512gb", "s24".
    Alnum,
    /// Digits with Indonesian separators: "26.000.000", "3,5".
    Number,
    /// A rupiah amount: "Rp26.000.000", "rp 50rb".
    Price,
    Emoji,
    Punct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reduplication {
    /// anak-anak, mlaku-mlaku
    Full,
    /// berlari-lari (prefix on the first half)
    Prefixed,
    /// sayur-mayur, bolak-balik (sound change between halves)
    Imitative,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub text: String,
    /// Byte range in the original string, for highlighting and alignment.
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
    /// For reduplicated words: the repeated base ("anak" for "anak-anak").
    pub redup: Option<(Reduplication, String)>,
    /// A clitic written with a hyphen: "kuasa-Mu" → "mu".
    pub hyphen_clitic: Option<String>,
}

impl Token {
    pub fn lower(&self) -> String {
        self.text.to_lowercase()
    }

    pub fn is_wordlike(&self) -> bool {
        matches!(self.kind, TokenKind::Word | TokenKind::Alnum)
    }
}

const HYPHEN_CLITICS: &[&str] = &["nya", "ku", "mu", "lah", "kah", "pun", "tah"];

pub fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let end_of = |i: usize| chars.get(i).map_or(text.len(), |&(b, _)| b);
    let mut out = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let (start, c) = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Rupiah: "Rp", "Rp.", "rp " followed by a number.
        if let Some(j) = rupiah_prefix(&chars, i)
            && let Some(k) = scan_number(&chars, j)
        {
            let k = scan_unit_suffix(&chars, k);
            out.push(token(text, start, end_of(k), TokenKind::Price));
            i = k;
            continue;
        }

        if c.is_ascii_digit() {
            let k = scan_number(&chars, i).unwrap_or(i + 1);
            // Digits glued to letters ("512gb", "50rb") form one token.
            let mut m = k;
            while m < chars.len() && chars[m].1.is_alphanumeric() {
                m += 1;
            }
            let kind = if m > k { TokenKind::Alnum } else { TokenKind::Number };
            out.push(token(text, start, end_of(m), kind));
            i = m;
            continue;
        }

        if c.is_alphabetic() {
            let mut k = i;
            let mut has_digit = false;
            while k < chars.len() {
                let ch = chars[k].1;
                let joiner = matches!(ch, '-' | '\'' | '’')
                    && k + 1 < chars.len()
                    && chars[k + 1].1.is_alphabetic()
                    && k > i;
                if ch.is_alphabetic() || ch.is_ascii_digit() || joiner {
                    has_digit |= ch.is_ascii_digit();
                    k += 1;
                } else {
                    break;
                }
            }
            let mut t =
                token(text, start, end_of(k), if has_digit { TokenKind::Alnum } else { TokenKind::Word });
            annotate_hyphen(&mut t);
            out.push(t);
            i = k;
            continue;
        }

        if is_emoji(c) {
            // Keep emoji sequences (ZWJ, variation selectors, skin tones) together.
            let mut k = i + 1;
            while k < chars.len()
                && (is_emoji_joiner(chars[k].1) || (is_emoji(chars[k].1) && is_emoji_joiner(chars[k - 1].1)))
            {
                k += 1;
            }
            out.push(token(text, start, end_of(k), TokenKind::Emoji));
            i = k;
            continue;
        }

        // Punctuation: repeated marks ("??", "...") stay one token.
        let mut k = i + 1;
        while k < chars.len() && chars[k].1 == c {
            k += 1;
        }
        out.push(token(text, start, end_of(k), TokenKind::Punct));
        i = k;
    }
    out
}

fn token(text: &str, start: usize, end: usize, kind: TokenKind) -> Token {
    Token { text: text[start..end].to_string(), start, end, kind, redup: None, hyphen_clitic: None }
}

fn rupiah_prefix(chars: &[(usize, char)], i: usize) -> Option<usize> {
    let c0 = chars.get(i)?.1;
    let c1 = chars.get(i + 1)?.1;
    if !(c0 == 'R' || c0 == 'r') || !(c1 == 'p' || c1 == 'P') {
        return None;
    }
    let mut j = i + 2;
    if chars.get(j).is_some_and(|c| c.1 == '.') {
        j += 1;
    }
    while chars.get(j).is_some_and(|c| c.1 == ' ') {
        j += 1;
    }
    chars.get(j).filter(|c| c.1.is_ascii_digit()).map(|_| j)
}

/// Digits with '.' or ',' between digit groups. Returns the index after it.
fn scan_number(chars: &[(usize, char)], i: usize) -> Option<usize> {
    if !chars.get(i)?.1.is_ascii_digit() {
        return None;
    }
    let mut k = i;
    while k < chars.len() {
        let c = chars[k].1;
        let sep = matches!(c, '.' | ',') && chars.get(k + 1).is_some_and(|n| n.1.is_ascii_digit());
        if c.is_ascii_digit() || sep {
            k += 1;
        } else {
            break;
        }
    }
    Some(k)
}

/// "50rb", "2jt": short unit words glued to a price.
fn scan_unit_suffix(chars: &[(usize, char)], k: usize) -> usize {
    let mut m = k;
    while m < chars.len() && chars[m].1.is_alphabetic() && m - k < 4 {
        m += 1;
    }
    let unit: String = chars[k..m].iter().map(|c| c.1.to_ascii_lowercase()).collect();
    if ["rb", "ribu", "k", "jt", "juta", "m"].contains(&unit.as_str()) { m } else { k }
}

fn annotate_hyphen(t: &mut Token) {
    let lower = t.text.to_lowercase();
    let Some((a, b)) = lower.split_once('-') else { return };
    if b.contains('-') || a.is_empty() || b.is_empty() {
        return;
    }
    if HYPHEN_CLITICS.contains(&b) {
        t.hyphen_clitic = Some(b.to_string());
    } else if a == b {
        t.redup = Some((Reduplication::Full, a.to_string()));
    } else if a.ends_with(b) && a.len() > b.len() {
        t.redup = Some((Reduplication::Prefixed, b.to_string()));
    } else if let Some(base) = suffixed_redup(a, b) {
        t.redup = Some((Reduplication::Full, base.to_string()));
    } else if looks_imitative(a, b) {
        t.redup = Some((Reduplication::Imitative, a.to_string()));
    }
}

/// "anak-anaknya", "rumah-rumahan": the second half is the first plus a suffix.
fn suffixed_redup<'a>(a: &'a str, b: &str) -> Option<&'a str> {
    (b.len() > a.len() && b.starts_with(a)).then_some(a)
}

/// sayur-mayur, bolak-balik, lauk-pauk: same length ±1, same ending or same consonant frame.
fn looks_imitative(a: &str, b: &str) -> bool {
    let (la, lb) = (a.chars().count(), b.chars().count());
    if la < 3 || lb < 3 || la.abs_diff(lb) > 1 {
        return false;
    }
    let tail = |s: &str| s.chars().rev().take(3).collect::<String>();
    let consonants = |s: &str| s.chars().filter(|c| !"aeiou".contains(*c)).collect::<String>();
    tail(a) == tail(b) || consonants(a) == consonants(b)
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0x2190..=0x21FF | 0x3030 | 0x303D)
}

fn is_emoji_joiner(c: char) -> bool {
    matches!(c as u32, 0x200D | 0xFE0F | 0x1F3FB..=0x1F3FF | 0x20E3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(s: &str) -> Vec<String> {
        tokenize(s).into_iter().map(|t| t.text).collect()
    }

    #[test]
    fn splits_words_and_punctuation() {
        assert_eq!(texts("Saya sedang membaca buku."), ["Saya", "sedang", "membaca", "buku", "."]);
        assert_eq!(texts("ng prabowo?"), ["ng", "prabowo", "?"]);
        assert_eq!(texts("Apa kabar??"), ["Apa", "kabar", "??"]);
    }

    #[test]
    fn keeps_offsets() {
        let s = "piye dhe";
        for t in tokenize(s) {
            assert_eq!(&s[t.start..t.end], t.text);
        }
    }

    #[test]
    fn reduplication() {
        let t = &tokenize("Anak-anak bermain")[0];
        assert_eq!(t.redup, Some((Reduplication::Full, "anak".into())));
        assert_eq!(tokenize("mlaku-mlaku")[0].redup, Some((Reduplication::Full, "mlaku".into())));
        assert_eq!(tokenize("berlari-lari")[0].redup, Some((Reduplication::Prefixed, "lari".into())));
        assert_eq!(tokenize("anak-anaknya")[0].redup, Some((Reduplication::Full, "anak".into())));
        assert_eq!(tokenize("sayur-mayur")[0].redup.as_ref().map(|r| r.0), Some(Reduplication::Imitative));
        assert_eq!(tokenize("bolak-balik")[0].redup.as_ref().map(|r| r.0), Some(Reduplication::Imitative));
    }

    #[test]
    fn hyphen_clitic() {
        assert_eq!(tokenize("kuasa-Mu")[0].hyphen_clitic.as_deref(), Some("mu"));
    }

    #[test]
    fn numbers_prices_and_alnum() {
        let toks = tokenize("iphone 17 pm 512gb Rp26.000.000 harga 3,5 juta rp 50rb");
        let kinds: Vec<_> = toks.iter().map(|t| (t.text.as_str(), t.kind)).collect();
        assert_eq!(
            kinds,
            [
                ("iphone", TokenKind::Word),
                ("17", TokenKind::Number),
                ("pm", TokenKind::Word),
                ("512gb", TokenKind::Alnum),
                ("Rp26.000.000", TokenKind::Price),
                ("harga", TokenKind::Word),
                ("3,5", TokenKind::Number),
                ("juta", TokenKind::Word),
                ("rp 50rb", TokenKind::Price),
            ]
        );
    }

    #[test]
    fn emoji_sequences_stay_whole() {
        assert_eq!(texts("mantap 👍🏽 wkwk 😂😂"), ["mantap", "👍🏽", "wkwk", "😂", "😂"]);
        assert_eq!(texts("👨‍👩‍👧 ok"), ["👨‍👩‍👧", "ok"]);
    }

    #[test]
    fn english_mixed() {
        assert_eq!(
            texts("Which is, gue literally capek banget."),
            ["Which", "is", ",", "gue", "literally", "capek", "banget", "."]
        );
        assert_eq!(texts("don't"), ["don't"]);
    }
}
