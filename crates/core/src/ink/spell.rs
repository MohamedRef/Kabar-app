//! Listening Companion: the learner scribbles a word the way they heard it;
//! Kabar suggests the real words it could be. Deterministic: edit distance
//! on a "sound-folded" spelling that forgives what ears and old spellings
//! confuse in Indonesian and Javanese.

use kabar_protocol::content::Lang;

/// A word Kabar knows, with whatever the caller wants to show for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub lang: Lang,
    pub word: String,
    /// Lower is more frequent (frequency rank); 0 for the learner's own words.
    pub rank: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub lang: Lang,
    pub word: String,
    /// Edits between the folded forms (0 = sounds the same).
    pub distance: usize,
    /// Edits between the spellings as written.
    pub spelling_distance: usize,
}

/// Folds a spelling to roughly how it sounds: old Dutch-era spellings
/// (oe → u, dj → j, tj → c, sj → sy), ch/kh → k, dh/th → d/t (Javanese
/// retroflex and dental sound alike to a beginner), f/v → p, z → s, q/x → k,
/// accents off, doubled letters once, a final glottal k after a vowel
/// dropped, and final u/i merged into o/e (Javanese -o/-u, -e/-i).
pub fn fold(word: &str) -> String {
    let w: String = word
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'á' | 'à' | 'â' => 'a',
            'ó' | 'ò' | 'ô' => 'o',
            'í' | 'ì' => 'i',
            'ú' | 'ù' => 'u',
            c => c,
        })
        .filter(|c| c.is_alphabetic())
        .collect();
    let mut s = w;
    for (from, to) in [
        ("oe", "u"),
        ("dj", "j"),
        ("tj", "c"),
        ("sj", "sy"),
        ("ch", "k"),
        ("kh", "k"),
        ("dh", "d"),
        ("th", "t"),
        ("ng", "N"),
        ("ny", "Y"),
    ] {
        s = s.replace(from, to);
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let c = match c {
            'f' | 'v' => 'p',
            'z' => 's',
            'q' => 'k',
            'x' => 'k',
            c => c,
        };
        if out.ends_with(c) {
            continue;
        }
        out.push(c);
    }
    // Final glottal k: "bapak" ≈ "bapa'", "enak" ≈ "ena".
    if out.len() > 2 && out.ends_with('k') {
        let before = out[..out.len() - 1].chars().last();
        if before.is_some_and(|c| "aiueo".contains(c)) {
            out.pop();
        }
    }
    // Final o/u and e/i are hard to hear apart (Javanese -o, -u; -e, -i).
    if let Some(last) = out.pop() {
        out.push(match last {
            'u' => 'o',
            'i' => 'e',
            c => c,
        });
    }
    out
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j - 1] + usize::from(a[i - 1] != b[j - 1])).min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Up to `limit` words the scribble could be, closest first: by folded
/// distance, then spelling distance, then frequency. Folded distance is
/// allowed up to a third of the word's length (at least 1).
pub fn suggest(scribble: &str, lang: Lang, lexicon: &[Known], limit: usize) -> Vec<Suggestion> {
    let raw = scribble.trim().to_lowercase();
    let f = fold(&raw);
    if f.is_empty() {
        return Vec::new();
    }
    let max = (f.chars().count() / 3).max(1);
    let mut found: Vec<(Suggestion, u32)> = lexicon
        .iter()
        .filter(|k| k.lang == lang)
        .filter_map(|k| {
            let kf = fold(&k.word);
            if kf.chars().count().abs_diff(f.chars().count()) > max {
                return None;
            }
            let d = distance(&f, &kf);
            (d <= max).then(|| {
                let s = Suggestion {
                    lang: k.lang,
                    word: k.word.clone(),
                    distance: d,
                    spelling_distance: distance(&raw, &k.word.to_lowercase()),
                };
                (s, k.rank)
            })
        })
        .collect();
    found.sort_by(|(a, ra), (b, rb)| {
        a.distance.cmp(&b.distance).then(a.spelling_distance.cmp(&b.spelling_distance)).then(ra.cmp(rb))
    });
    found.dedup_by(|a, b| a.0.word == b.0.word);
    found.into_iter().take(limit).map(|(s, _)| s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(words: &[(&str, Lang)]) -> Vec<Known> {
        words
            .iter()
            .enumerate()
            .map(|(i, (w, l))| Known { lang: *l, word: w.to_string(), rank: i as u32 + 1 })
            .collect()
    }

    #[test]
    fn folding_forgives_what_ears_confuse() {
        assert_eq!(fold("Djakarta"), fold("jakarta"));
        assert_eq!(fold("tjinta"), fold("cinta"));
        assert_eq!(fold("Soekarno"), fold("sukarno"));
        assert_eq!(fold("bapa"), fold("bapak"));
        assert_eq!(fold("khabar"), fold("kabar"));
        assert_eq!(fold("pikir"), fold("fikir"));
        assert_eq!(fold("piro"), fold("piru"));
        assert_eq!(fold("dhe"), fold("de"));
        assert_ne!(fold("pasar"), fold("pasir"));
    }

    #[test]
    fn suggestions_for_what_was_heard() {
        let lexicon = lex(&[
            ("sekali", Lang::Id),
            ("bagaimana", Lang::Id),
            ("enak", Lang::Id),
            ("pasar", Lang::Id),
            ("pasir", Lang::Id),
            ("kabar", Lang::Id),
            ("piye", Lang::Jv),
            ("pira", Lang::Jv),
        ]);
        // Heard "bagai mana", wrote one word with a slip.
        let s = suggest("bagaymana", Lang::Id, &lexicon, 3);
        assert_eq!(s[0].word, "bagaimana");
        // "ena" for enak: same sound.
        let s = suggest("ena", Lang::Id, &lexicon, 3);
        assert_eq!((s[0].word.as_str(), s[0].distance), ("enak", 0));
        // pasar beats pasir for "pasar", but both are offered.
        let s = suggest("pasar", Lang::Id, &lexicon, 3);
        assert_eq!(s.iter().map(|s| s.word.as_str()).collect::<Vec<_>>(), ["pasar", "pasir"]);
        // Language filter.
        assert!(suggest("piye", Lang::Id, &lexicon, 3).iter().all(|s| s.lang == Lang::Id));
        assert_eq!(suggest("piyé", Lang::Jv, &lexicon, 3)[0].word, "piye");
        assert!(suggest("  ", Lang::Id, &lexicon, 3).is_empty());
        assert!(suggest("xylofon", Lang::Id, &lexicon, 3).is_empty());
    }
}
