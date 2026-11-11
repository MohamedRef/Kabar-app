//! A lesson's content fingerprint: what the learner's "the content is
//! right" check is bound to (engineering audit F25), so a lesson updated
//! under the same id asks to be checked again instead of keeping the old
//! version's trust.
//!
//! Policy [`POLICY`] (tested below): the fingerprint covers every word the
//! learner sees or answers (slides, exercises and their answers, dialogue,
//! new words, translations, souvenir, title). It ignores
//! - YAML layout, comments and key order (the parsed lesson is hashed);
//! - runs of whitespace inside text, and leading/trailing whitespace;
//! - the build-time `review` block (who approved the file is provenance,
//!   not content).
//!
//! Anything else, a letter, a punctuation mark, an answer, makes a new
//! fingerprint. Bumping [`POLICY`] makes every stored fingerprint differ,
//! so only do it with a plan for existing checks.

use kabar_protocol::lesson::LessonContent;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The hashing policy version, the fingerprint's prefix.
pub const POLICY: u32 = 1;

/// Fields that are not content.
const NOT_CONTENT: &[&str] = &["review"];

/// `v1:` and 32 hex digits.
pub fn lesson(l: &LessonContent) -> String {
    let mut value = serde_json::to_value(l).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut value {
        for key in NOT_CONTENT {
            map.remove(*key);
        }
    }
    let mut hash = Sha256::new();
    feed(&mut hash, &value);
    let digest = hash.finalize();
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("v{POLICY}:{hex}")
}

/// Hashes a value with type tags and lengths, so `["a b"]` and `["a", "b"]`
/// can't collide; object keys in sorted order.
fn feed(hash: &mut Sha256, v: &Value) {
    match v {
        Value::Null => hash.update(b"n"),
        Value::Bool(b) => hash.update(if *b { b"t" } else { b"f" }),
        Value::Number(n) => {
            let s = n.to_string();
            hash.update(b"#");
            hash.update((s.len() as u64).to_le_bytes());
            hash.update(s.as_bytes());
        }
        Value::String(s) => {
            let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
            hash.update(b"s");
            hash.update((s.len() as u64).to_le_bytes());
            hash.update(s.as_bytes());
        }
        Value::Array(items) => {
            hash.update(b"[");
            hash.update((items.len() as u64).to_le_bytes());
            for item in items {
                feed(hash, item);
            }
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            hash.update(b"{");
            hash.update((keys.len() as u64).to_le_bytes());
            for k in keys {
                hash.update((k.len() as u64).to_le_bytes());
                hash.update(k.as_bytes());
                feed(hash, &map[k]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lesson::parse_lesson;

    fn sample() -> String {
        std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content/lessons/id/bipa1.u01.l01.yaml"
        ))
        .unwrap()
    }

    fn print(yaml: &str) -> String {
        lesson(&parse_lesson(yaml).unwrap())
    }

    #[test]
    fn layout_comments_and_spacing_keep_the_fingerprint() {
        let yaml = sample();
        let base = print(&yaml);
        assert!(base.starts_with("v1:") && base.len() == 3 + 32, "{base}");
        // A comment, a blank line, and a double space inside a quoted word.
        let reformatted = format!("# reviewed again\n\n{yaml}").replacen("Selamat pagi", "Selamat  pagi", 1);
        assert_eq!(print(&reformatted), base);
        // Who approved the file isn't content.
        let mut l = parse_lesson(&yaml).unwrap();
        l.review.status = "approved".into();
        l.review.verified = true;
        assert_eq!(lesson(&l), base);
    }

    #[test]
    fn a_changed_word_or_answer_makes_a_new_fingerprint() {
        let yaml = sample();
        let base = print(&yaml);
        let mut l = parse_lesson(&yaml).unwrap();
        l.exercises[0].id.push('x');
        assert_ne!(lesson(&l), base);
        let mut l = parse_lesson(&yaml).unwrap();
        l.can_do.en.push('!');
        assert_ne!(lesson(&l), base, "a translation is content");
        let mut l = parse_lesson(&yaml).unwrap();
        l.title = l.title.to_lowercase();
        assert_ne!(lesson(&l), base, "case is content");
    }
}
