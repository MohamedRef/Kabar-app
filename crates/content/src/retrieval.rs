//! In-lesson retrieval: every word a lesson teaches comes back in enough of
//! that lesson's own exercises for a first memory trace (#135).

use serde_json::Value;

/// Exercises that must retrieve each new word.
pub const MIN_RETRIEVALS: usize = 3;
/// Distinct exercise types among those retrievals.
pub const MIN_TYPES: usize = 2;

/// A word counts as retrieved by an exercise when the exercise tags it in
/// `words:` or writes it in a field the learner reads, hears, builds or
/// types. A suffixed form (batikipun, rumahnya) only counts when tagged.
pub fn check(id: &str, lesson: &Value, problems: &mut Vec<String>) {
    let exercises: Vec<&Value> =
        lesson.get("exercises").and_then(Value::as_array).into_iter().flatten().collect();
    let fields: Vec<(String, Vec<String>)> = exercises.iter().map(|e| (target_text(e), tags(e))).collect();
    let new_words = lesson.get("new_words").and_then(Value::as_array).into_iter().flatten();
    for word in new_words.filter_map(|w| w.get("word").and_then(Value::as_str)) {
        let forms = forms(word);
        let mut count = 0;
        let mut types = Vec::new();
        for (e, (text, tags)) in exercises.iter().zip(&fields) {
            if forms.iter().any(|f| tags.contains(f) || contains_word(text, f)) {
                count += 1;
                let kind = e.get("type").and_then(Value::as_str).unwrap_or("");
                if !types.contains(&kind) {
                    types.push(kind);
                }
            }
        }
        if count < MIN_RETRIEVALS || types.len() < MIN_TYPES {
            problems.push(format!(
                "{id}: new word '{word}' is retrieved in {count} exercise(s) of {} type(s); \
                 it needs at least {MIN_RETRIEVALS} across {MIN_TYPES} types (tag it in words: or use it in an exercise)",
                types.len()
            ));
        }
    }
}

/// The forms a `new_words` entry can be met in: lower case, without a
/// parenthetical note or trailing "?", and split on "/" or "," when it lists
/// alternatives ("-ipun / -nipun").
fn forms(word: &str) -> Vec<String> {
    let mut plain = String::new();
    let mut depth = 0;
    for c in word.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let plain = plain.trim_end_matches('?').to_owned();
    let mut forms = vec![plain.clone()];
    if plain.contains(['/', ',']) {
        forms.extend(plain.split(['/', ',']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned));
    }
    forms
}

fn tags(e: &Value) -> Vec<String> {
    let words = e.get("words").and_then(Value::as_array).into_iter().flatten();
    words.filter_map(Value::as_str).map(str::to_lowercase).collect()
}

/// The target-language text an exercise puts in front of the learner.
fn target_text(e: &Value) -> String {
    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let list = |k: &str| -> Vec<String> {
        let items = e.get(k).and_then(Value::as_array).into_iter().flatten();
        items.filter_map(Value::as_str).map(str::to_owned).collect()
    };
    let mut out = Vec::new();
    match e.get("type").and_then(Value::as_str).unwrap_or("") {
        "multiple_choice" => {
            out.push(s("question"));
            out.extend(list("options"));
        }
        "listen" => {
            out.push(s("text"));
            out.extend(list("options"));
        }
        "match_pairs" => {
            let pairs = e.get("pairs").and_then(Value::as_array).into_iter().flatten();
            out.extend(pairs.filter_map(|p| p.get("text").and_then(Value::as_str)).map(str::to_owned));
        }
        "translate" => {
            out.push(s("source_text"));
            out.extend(list("answers"));
        }
        "build_sentence" => out.push(s("answer")),
        "fill_blank" => {
            let answer = list("answers").into_iter().next().unwrap_or_default();
            out.push(s("sentence").replacen("___", &answer, 1));
        }
        "dictation" | "speaking" => out.push(s("text")),
        _ => {}
    }
    out.join(" | ").to_lowercase()
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whole-word match; a form starting with "-" is a suffix and must follow a
/// word character instead ("-nya" in "rumahnya").
fn contains_word(text: &str, form: &str) -> bool {
    let (needle, suffix) = match form.strip_prefix('-') {
        Some(rest) => (rest, true),
        None => (form, false),
    };
    if needle.is_empty() {
        return false;
    }
    text.match_indices(needle).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + needle.len()..].chars().next();
        let left = if suffix {
            before.is_some_and(is_word_char)
        } else {
            !before.is_some_and(|c| is_word_char(c) || c == '-')
        };
        left && !after.is_some_and(|c| is_word_char(c) || c == '-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn whole_words_and_suffixes() {
        assert!(contains_word("aku arep nang pasar.", "pasar"));
        assert!(!contains_word("pasaran", "pasar"));
        assert!(!contains_word("kupu-kupu", "kupu"));
        assert!(contains_word("rumahnya besar", "-nya"));
        assert!(!contains_word("nya", "-nya"));
    }

    #[test]
    fn forms_drop_notes_and_split_alternatives() {
        assert_eq!(forms("Sugeng rawuh"), ["sugeng rawuh"]);
        assert_eq!(forms("resep (obat)"), ["resep"]);
        assert_eq!(forms("oleh kurang?"), ["oleh kurang"]);
        assert_eq!(forms("-ipun / -nipun"), ["-ipun / -nipun", "-ipun", "-nipun"]);
    }

    #[test]
    fn flags_under_practised_words() {
        let lesson = json!({
            "new_words": [{"word": "mangga"}, {"word": "dereng"}],
            "exercises": [
                {"id": "e01", "type": "multiple_choice", "question": "Mangga, Mas.", "options": ["Silakan.", "Belum."]},
                {"id": "e02", "type": "listen", "text": "Dereng, Bu.", "options": ["Dereng, Bu.", "Sampun, Bu."]},
                {"id": "e03", "type": "fill_blank", "sentence": "Mangga, ___ nedha.", "answers": ["dereng"]},
                {"id": "e04", "type": "speaking", "text": "Inggih.", "words": ["mangga"]},
            ]
        });
        let mut problems = Vec::new();
        check("j3.u01.l03", &lesson, &mut problems);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("'dereng' is retrieved in 2 exercise(s) of 2 type(s)"));
    }
}
