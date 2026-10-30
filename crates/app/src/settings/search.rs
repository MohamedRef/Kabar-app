//! Local settings search, tolerant of Arabic marks and common letter variants.

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(*c, '\u{0610}'..='\u{061a}' | '\u{0640}' | '\u{064b}'..='\u{065f}' | '\u{0670}' | '\u{06d6}'..='\u{06ed}'))
        .map(|c| match c {
            'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
            'ى' => 'ي',
            _ => c,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn matches(query: &str, text: &str) -> bool {
    let text = normalize(text);
    normalize(query).split_whitespace().all(|word| {
        text.contains(word)
            || word.strip_prefix("ال").is_some_and(|bare| !bare.is_empty() && text.contains(bare))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_marks_case_and_whitespace_do_not_hide_a_setting() {
        assert!(matches("  إِشْعَارات  ", "الإشعارات والتذكيرات"));
        assert!(matches("  FONT   size ", "Theme, font and text size"));
        assert!(matches("علي", "على الجهاز"));
        assert!(matches("حجم الخط", "خط حجم"));
        assert!(matches("", "anything"));
        assert!(matches(" \t ", "anything"));
        assert!(!matches("font sound", "Font size"));
    }
}
