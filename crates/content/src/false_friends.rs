//! Javanese words spelled like an Indonesian new word (#290). Mohamed learns
//! Indonesian first, so a Javanese `mau` ("a moment ago") reads as Indonesian
//! `mau` ("want") unless the lesson says otherwise. When the English glosses
//! of the pair share no content word, the Javanese entry must either name the
//! false friend (`false_friend: mau`) or confirm the match (`same_as_id: true`).
//! A false friend may also be an Indonesian street form (`aja` for `saja`).

use std::collections::{BTreeMap, BTreeSet};

use kabar_core::lang::{Lang, street};
use serde_json::Value;

/// Gloss words that carry no meaning of their own.
const STOP: &[&str] = &[
    "the", "and", "for", "with", "not", "but", "are", "was", "you", "your", "our", "its", "his", "her",
    "their", "this", "that", "from", "into", "onto", "than", "why", "also", "very", "just", "only", "one",
    "two", "some", "any", "all",
];

/// Content words of an English gloss, leaving out parenthesised asides.
fn content_words(gloss: &str) -> BTreeSet<String> {
    let mut plain = String::new();
    let mut depth = 0usize;
    for c in gloss.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(c.to_ascii_lowercase()),
            _ => {}
        }
    }
    plain
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| w.len() >= 3 && !STOP.contains(w))
        .map(str::to_owned)
        .collect()
}

fn words(v: &Value) -> impl Iterator<Item = &Value> {
    v.get("new_words").and_then(Value::as_array).into_iter().flatten()
}

fn text<'a>(w: &'a Value, key: &str) -> &'a str {
    w.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Lowercase spellings of a headword: `aja / ojo` is both `aja` and `ojo`.
fn variants(word: &str) -> Vec<String> {
    word.split(" / ").map(|f| f.trim().to_lowercase()).filter(|f| !f.is_empty()).collect()
}

pub fn check(lessons: &BTreeMap<String, Value>, out: &mut Vec<String>) {
    let lang = |v: &Value| v.get("language").and_then(Value::as_str).unwrap_or("").to_owned();
    // Indonesian new words: lowercase word -> (lesson, English gloss), first teaching wins.
    let mut id: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    for (lesson, v) in lessons.iter().filter(|(_, v)| lang(v) == "id") {
        for w in words(v) {
            for form in variants(text(w, "word")) {
                id.entry(form).or_insert((lesson, text(w, "en")));
            }
        }
    }
    for (lesson, v) in lessons {
        let jv = lang(v) == "jv";
        for w in words(v) {
            let word = text(w, "word");
            let ff = w.get("false_friend").and_then(Value::as_str);
            let same = w.get("same_as_id").and_then(Value::as_bool).unwrap_or(false);
            if !jv && (ff.is_some() || w.get("same_as_id").is_some()) {
                out.push(format!("{lesson}: {word}: false_friend / same_as_id are for Javanese words only"));
                continue;
            }
            if let Some(f) = ff.map(str::to_lowercase)
                && !id.contains_key(&f)
                && !street::formal(Lang::Indonesian, &f).is_some_and(|s| id.contains_key(s))
            {
                out.push(format!(
                    "{lesson}: {word}: false_friend '{f}' is neither an Indonesian new word nor a street form of one"
                ));
            }
            if ff.is_some() && same {
                out.push(format!("{lesson}: {word}: false_friend and same_as_id contradict each other"));
            }
            if !jv || ff.is_some() || same {
                continue;
            }
            for form in variants(word) {
                let Some((id_lesson, id_gloss)) = id.get(&form) else { continue };
                if content_words(text(w, "en")).is_disjoint(&content_words(id_gloss)) {
                    out.push(format!(
                        "{lesson}: {word} looks like Indonesian {form} \"{id_gloss}\" ({id_lesson}); \
                         add false_friend: {form} with a ≠ note in the gloss, or same_as_id: true"
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lessons(jv_word: Value) -> BTreeMap<String, Value> {
        BTreeMap::from([
            (
                "bipa1.u02.l04".to_owned(),
                json!({"language": "id", "new_words": [{"word": "mau", "en": "to want"}, {"word": "kapan", "en": "when"}]}),
            ),
            ("j1.u05.l02".to_owned(), json!({"language": "jv", "new_words": [jv_word]})),
        ])
    }

    fn problems(jv_word: Value) -> Vec<String> {
        let mut out = Vec::new();
        check(&lessons(jv_word), &mut out);
        out
    }

    #[test]
    fn unflagged_false_friend_is_reported() {
        let p = problems(json!({"word": "mau", "en": "a moment ago (= tadi)"}));
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("false_friend: mau"));
    }

    #[test]
    fn slash_headwords_are_checked_per_form() {
        let p = problems(json!({"word": "arep / mau", "en": "a moment ago"}));
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("false_friend: mau"));
    }

    #[test]
    fn flagged_or_matching_words_pass() {
        assert!(problems(json!({"word": "mau", "en": "a moment ago", "false_friend": "mau"})).is_empty());
        assert!(problems(json!({"word": "kapan", "en": "at what time", "same_as_id": true})).is_empty());
        assert!(problems(json!({"word": "kapan", "en": "when (= kapan)"})).is_empty());
        assert!(problems(json!({"word": "sepur", "en": "train"})).is_empty());
    }

    #[test]
    fn bad_flags_are_reported() {
        assert_eq!(problems(json!({"word": "mau", "en": "x", "false_friend": "zzz"})).len(), 1);
        assert_eq!(
            problems(json!({"word": "mau", "en": "x", "false_friend": "mau", "same_as_id": true})).len(),
            1
        );
    }
}
