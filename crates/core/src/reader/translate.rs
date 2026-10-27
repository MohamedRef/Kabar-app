//! The Reader's translation under each line (spec §11): split a text's lines
//! into requests the `translate_text` task can answer in one go. Every request
//! is personal: Reader texts are often someone's chat, so they never reach an
//! engine that trains on inputs.

use serde_json::json;

use crate::ai::AiRequest;
use crate::lang::Lang;

/// Lines per request: three voices per line must fit the route's output cap.
pub const MAX_LINES: usize = 25;
/// Characters per request, for long article paragraphs.
pub const MAX_CHARS: usize = 1_800;

/// The `translate_text` input, exactly as the golden set shapes it.
pub fn request(lang: Lang, lines: &[&str]) -> AiRequest {
    AiRequest {
        input: json!({
            "lang": if lang == Lang::Javanese { "jv" } else { "id" },
            "lines": lines,
        }),
        personal: true,
        image_sha256: None,
    }
}

/// Consecutive index ranges of `lines`, each at most `max_lines` long and
/// `max_chars` characters (a single longer line gets a range of its own).
pub fn chunks(lines: &[&str], max_lines: usize, max_chars: usize) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = 0;
    for (i, l) in lines.iter().enumerate() {
        let n = l.chars().count();
        if i > start && (i - start >= max_lines || chars + n > max_chars) {
            out.push(start..i);
            start = i;
            chars = 0;
        }
        chars += n;
    }
    if start < lines.len() {
        out.push(start..lines.len());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_personal_and_shaped_like_the_golden_set() {
        let r = request(Lang::Javanese, &["Sek tak lapor pusat", "arp dicul piro"]);
        assert!(r.personal);
        assert_eq!(r.input["lang"], "jv");
        assert_eq!(r.input["lines"][1], "arp dicul piro");
        assert_eq!(request(Lang::Indonesian, &["Apa kabar?"]).input["lang"], "id");
    }

    #[test]
    fn long_texts_split_by_lines_and_length() {
        let lines = ["Saya sedang membaca buku."; 7];
        assert_eq!(chunks(&lines, 3, 1_000), vec![0..3, 3..6, 6..7]);
        assert_eq!(chunks(&lines, 25, 60), vec![0..2, 2..4, 4..6, 6..7]);
        let long = "a".repeat(100);
        assert_eq!(chunks(&[&long, "b", &long], 25, 50), vec![0..1, 1..2, 2..3]);
        assert!(chunks(&[], 25, 50).is_empty());
    }
}
