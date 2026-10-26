//! Small text helpers shared by the pipeline: word tokenizing for frequency
//! counts and script checks for validation.

use unicode_normalization::UnicodeNormalization;

/// Lowercased word tokens: letters, with inner hyphens (anak-anak) and
/// apostrophes (Javanese ng'omah) kept. Digits and punctuation split words.
pub fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_alphabetic() || c == '-' || c == '\''))
        .map(|w| w.trim_matches(|c| c == '-' || c == '\''))
        .filter(|w| !w.is_empty())
        .map(|w| w.nfc().flat_map(char::to_lowercase).collect())
}

/// True for a plausible Latin-script dictionary word: lowercase a–z (plus é
/// for Javanese), optionally hyphenated (anak-anak, mlaku-mlaku).
pub fn is_latin_word(word: &str) -> bool {
    !word.is_empty()
        && word.split('-').all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || matches!(c, 'é' | 'è'))
        })
}

pub fn is_arabic_letter(c: char) -> bool {
    matches!(c, '\u{0620}'..='\u{064A}' | '\u{0671}'..='\u{06D3}')
}

/// Contains at least one Arabic letter.
pub fn has_arabic(text: &str) -> bool {
    text.chars().any(is_arabic_letter)
}

/// Arabic letters only, without diacritics or tatweel, with alef/hamza/yaa
/// variants folded, so خَبَر and خبر compare equal.
pub fn arabic_skeleton(text: &str) -> String {
    text.chars()
        .filter_map(|c| match c {
            'أ' | 'إ' | 'آ' | 'ٱ' => Some('ا'),
            'ى' => Some('ي'),
            'ة' => Some('ه'),
            'ؤ' | 'ئ' => Some('ء'),
            c if is_arabic_letter(c) => Some(c),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_reduplication_and_drops_digits() {
        let got: Vec<_> = words("Anak-anak bermain di taman, 512GB! 'Kowe'").collect();
        assert_eq!(got, ["anak-anak", "bermain", "di", "taman", "gb", "kowe"]);
    }

    #[test]
    fn latin_word_shape() {
        assert!(is_latin_word("mlaku-mlaku"));
        assert!(is_latin_word("kabar"));
        assert!(!is_latin_word("iPhone"));
        assert!(!is_latin_word("17"));
        assert!(!is_latin_word("-nya"));
    }

    #[test]
    fn arabic_skeleton_ignores_vowels_and_folds_variants() {
        assert_eq!(arabic_skeleton("خَبَر"), arabic_skeleton("خبر"));
        assert_eq!(arabic_skeleton("سَلَامَة"), arabic_skeleton("سلامه"));
        assert_eq!(arabic_skeleton("إِمْكان"), "امكان");
        assert!(has_arabic("كلمة"));
        assert!(!has_arabic("kalimat"));
    }
}
