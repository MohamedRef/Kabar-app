//! Lesson pacing (docs/PEDAGOGY.md): the caps every lesson file must fit and
//! the time model behind a lesson's "~5 min". The content checks in
//! `crates/content` fail any lesson over these caps; the Lesson Player and the
//! Journey Map show [`estimate_minutes`]; the coach sizes daily plans with it.
//!
//! The per-step seconds are starting estimates. The coach recalibrates plans
//! from real attempt times; revisit these numbers when that data exists.

use std::ops::RangeInclusive;

use kabar_protocol::lesson::{ExerciseKind, LessonContent};

/// Hard cap on a lesson's estimated time.
pub const MAX_LESSON_SECONDS: u32 = 7 * 60;
/// What a typical lesson should take.
pub const TARGET_LESSON_SECONDS: RangeInclusive<u32> = 4 * 60..=6 * 60;
/// Examples one teaching slide may show.
pub const MAX_SLIDE_EXAMPLES: usize = 3;
/// Exercises any lesson may have, whatever its kind.
pub const MIN_EXERCISES: usize = 6;
pub const MAX_EXERCISES: usize = 12;
/// Exercise types a lesson must mix.
pub const MIN_EXERCISE_TYPES: usize = 4;

/// Reading a slide, plus each of its examples.
pub const SLIDE_SECONDS: u32 = 30;
/// A slide table replaces body bullets, so it adds no time but stays small.
pub const MAX_TABLE_ROWS: usize = 6;
pub const MAX_TABLE_COLUMNS: usize = 4;
pub const SLIDE_EXAMPLE_SECONDS: u32 = 10;
/// Hearing and reading one dialogue line.
pub const DIALOGUE_LINE_SECONDS: u32 = 8;
/// The new-words summary, plus each word on it.
pub const SUMMARY_SECONDS: u32 = 10;
pub const SUMMARY_WORD_SECONDS: u32 = 3;

/// What a lesson of one curriculum kind may hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caps {
    pub slides: RangeInclusive<usize>,
    pub exercises: RangeInclusive<usize>,
    pub dialogue_lines: RangeInclusive<usize>,
    pub new_words: RangeInclusive<usize>,
    /// Grammar points the curriculum may give one lesson.
    pub max_grammar: usize,
}

/// Caps for a curriculum lesson kind (`learn`, `practice`, `review`).
pub fn caps(kind: &str) -> Option<Caps> {
    let (slides, exercises, new_words, max_grammar) = match kind {
        "learn" => (1..=4, 6..=10, 3..=8, 2),
        "practice" => (0..=1, 6..=10, 0..=6, 1),
        "review" => (0..=1, 8..=12, 0..=0, 0),
        _ => return None,
    };
    Some(Caps { slides, exercises, dialogue_lines: 4..=6, new_words, max_grammar })
}

/// Seconds one exercise of this kind takes, including feedback.
pub fn exercise_seconds(kind: &ExerciseKind) -> u32 {
    match kind {
        ExerciseKind::MultipleChoice { .. } => 15,
        ExerciseKind::Listen { .. } | ExerciseKind::FillBlank { .. } | ExerciseKind::Speaking { .. } => 20,
        ExerciseKind::BuildSentence { .. } => 30,
        ExerciseKind::MatchPairs { .. } | ExerciseKind::Dictation { .. } => 35,
        ExerciseKind::Translate { .. } => 40,
    }
}

/// Estimated seconds for the whole lesson: slides → exercises → dialogue →
/// new-words summary.
pub fn estimate_seconds(lesson: &LessonContent) -> u32 {
    let slides: u32 =
        lesson.slides.iter().map(|s| SLIDE_SECONDS + SLIDE_EXAMPLE_SECONDS * s.examples.len() as u32).sum();
    let exercises: u32 = lesson.exercises.iter().map(|e| exercise_seconds(&e.kind)).sum();
    let dialogue = lesson.dialogue.as_ref().map_or(0, |d| DIALOGUE_LINE_SECONDS * d.lines.len() as u32);
    let summary = SUMMARY_SECONDS + SUMMARY_WORD_SECONDS * lesson.new_words.len() as u32;
    slides + exercises + dialogue + summary
}

/// [`estimate_seconds`] rounded up to whole minutes, as the curriculum's
/// `minutes` and the "~N min" labels show it.
pub fn estimate_minutes(lesson: &LessonContent) -> u32 {
    estimate_seconds(lesson).div_ceil(60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lesson::parse_lesson;

    const LESSON: &str = include_str!("../../../../fixtures/lessons/bipa1.u01.l01.yaml");

    #[test]
    fn every_kind_has_caps_and_unknown_kinds_none() {
        for kind in ["learn", "practice", "review"] {
            let c = caps(kind).unwrap();
            assert!(*c.exercises.start() >= MIN_EXERCISE_TYPES);
            assert!(*c.exercises.start() >= MIN_EXERCISES && *c.exercises.end() <= MAX_EXERCISES);
            assert_eq!(c.dialogue_lines, 4..=6);
        }
        assert_eq!(caps("learn").unwrap().new_words, 3..=8);
        assert_eq!(caps("review").unwrap().new_words, 0..=0);
        assert_eq!(caps("train"), None);
    }

    #[test]
    fn the_time_cap_binds_before_the_count_caps() {
        // The biggest learn lesson the counts allow: 4 slides × 3 examples,
        // 10 exercises, 6 dialogue lines, 8 words. Even all multiple choice it
        // is over 7 minutes, so authors have to trade slides for exercises.
        let biggest = 4 * (SLIDE_SECONDS + MAX_SLIDE_EXAMPLES as u32 * SLIDE_EXAMPLE_SECONDS)
            + 10 * 15
            + 6 * DIALOGUE_LINE_SECONDS
            + SUMMARY_SECONDS
            + 8 * SUMMARY_WORD_SECONDS;
        assert!(biggest > MAX_LESSON_SECONDS, "{biggest}");
        // A lean one fits the target: 2 slides × 2 examples, 7 mixed
        // exercises, 4 lines, 5 words.
        let lean = 2 * (SLIDE_SECONDS + 2 * SLIDE_EXAMPLE_SECONDS)
            + (15 + 35 + 20 + 20 + 30 + 40 + 20)
            + 4 * DIALOGUE_LINE_SECONDS
            + SUMMARY_SECONDS
            + 5 * SUMMARY_WORD_SECONDS;
        assert!(TARGET_LESSON_SECONDS.contains(&lean), "{lean}");
    }

    #[test]
    fn estimate_adds_up_every_step() {
        let lesson = parse_lesson(LESSON).unwrap();
        let slides: u32 = lesson.slides.iter().map(|s| 30 + 10 * s.examples.len() as u32).sum();
        let exercises: u32 = lesson.exercises.iter().map(|e| exercise_seconds(&e.kind)).sum();
        let lines = lesson.dialogue.as_ref().map_or(0, |d| d.lines.len() as u32);
        let expected = slides + exercises + 8 * lines + 10 + 3 * lesson.new_words.len() as u32;
        assert_eq!(estimate_seconds(&lesson), expected);
        assert_eq!(estimate_minutes(&lesson), expected.div_ceil(60));
    }
}
