//! Cards for the words a finished lesson taught (SPEC §14 → §16): the
//! lesson's `new_words` join the review deck so they come back tomorrow.

use kabar_protocol::lesson::{Example, LessonContent};
use kabar_protocol::srs::{ArabicOrigin, Card, CardKind, CardSource, StudyLanguage};

use super::cloze;

/// The stable id of a lesson word's recognition card: the same on every
/// device, so finishing a lesson twice (or on two devices) never duplicates.
pub fn lesson_card_id(lesson_id: &str, word: &str) -> String {
    format!("lesson.{lesson_id}.{}.recognition", word.to_lowercase())
}

/// Recognition cards for the lesson's new words that the deck doesn't hold
/// yet. A word that already has a recognition card (the starter deck's
/// "selamat", or the same word from an earlier lesson) is left alone, so
/// replaying a lesson adds nothing. The card's example is the first slide
/// example or dialogue line that contains the word.
pub fn lesson_cards(lesson: &LessonContent, existing: &[Card]) -> Vec<Card> {
    let language = if lesson.language == "jv" { StudyLanguage::Jv } else { StudyLanguage::Id };
    let has_card = |word: &str, out: &[Card]| {
        existing.iter().chain(out).any(|c| {
            c.kind == CardKind::Recognition && c.language == language && c.word.eq_ignore_ascii_case(word)
        })
    };
    let examples: Vec<Example> = lesson
        .slides
        .iter()
        .flat_map(|s| s.examples.iter().cloned())
        .chain(lesson.dialogue.iter().flat_map(|d| {
            d.lines.iter().map(|l| Example { text: l.text.clone(), meaning: l.meaning.clone() })
        }))
        .collect();
    let mut out: Vec<Card> = Vec::new();
    for w in &lesson.new_words {
        let word = w.word.trim();
        if word.is_empty() || has_card(word, &out) {
            continue;
        }
        out.push(Card {
            v: kabar_protocol::srs::CARD_VERSION,
            id: lesson_card_id(&lesson.id, word),
            word: word.to_string(),
            language,
            kind: CardKind::Recognition,
            meaning: w.meaning.clone(),
            note: None,
            example: examples.iter().find(|e| cloze(&e.text, word).is_some()).cloned(),
            arabic: w.arabic.as_ref().map(|a| ArabicOrigin { word: a.clone(), ..Default::default() }),
            source: CardSource::Lesson,
            source_ref: Some(lesson.id.clone()),
            verified: lesson.review.verified,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lesson::parse_lesson;
    use crate::review::seed_bridge_deck;

    const FIXTURE: &str = include_str!("../../../../fixtures/lessons/bipa1.u01.l01.yaml");

    #[test]
    fn new_words_become_cards_once() {
        let lesson = parse_lesson(FIXTURE).unwrap();
        let seed = seed_bridge_deck();
        let cards = lesson_cards(&lesson, &seed);
        let words: Vec<&str> = cards.iter().map(|c| c.word.as_str()).collect();
        assert!(!words.contains(&"selamat"), "the starter deck already has it");
        assert!(words.contains(&"pagi") && words.contains(&"sampai jumpa"), "{words:?}");
        assert_eq!(cards.len(), lesson.new_words.len() - 1);
        let pagi = cards.iter().find(|c| c.word == "pagi").unwrap();
        assert_eq!(pagi.id, "lesson.bipa1.u01.l01.pagi.recognition");
        assert_eq!(pagi.source, CardSource::Lesson);
        assert_eq!(pagi.source_ref.as_deref(), Some("bipa1.u01.l01"));
        assert_eq!(pagi.meaning.en, "morning");
        assert!(pagi.example.as_ref().is_some_and(|e| cloze(&e.text, "pagi").is_some()));

        // Finishing the lesson again adds nothing.
        let mut deck = seed;
        deck.extend(cards);
        assert!(lesson_cards(&lesson, &deck).is_empty());
    }
}
