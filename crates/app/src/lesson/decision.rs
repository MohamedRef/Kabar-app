//! Pure presentation decisions for lesson answers and voice notes.

use super::*;

/// How a graded choice is marked, so right and wrong never rely on colour
/// alone (WCAG 1.4.1): a glyph in the key-badge slot plus an aria label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChoiceMark {
    /// The right answer, which the learner didn't pick.
    Right,
    /// The learner's pick, and it's right.
    YoursRight,
    /// The learner's pick, and it's wrong.
    YoursWrong,
}

impl ChoiceMark {
    pub(super) fn glyph(self) -> &'static str {
        match self {
            ChoiceMark::Right | ChoiceMark::YoursRight => "✓",
            ChoiceMark::YoursWrong => "✗",
        }
    }

    pub(super) fn says(self) -> L {
        match self {
            ChoiceMark::Right => L::ChoiceCorrect,
            ChoiceMark::YoursRight => L::ChoiceYoursCorrect,
            ChoiceMark::YoursWrong => L::ChoiceYours,
        }
    }
}

/// Where the "no voice, listening skipped" note goes after a skip from `from`
/// to `to`: the first step the learner actually sees after the first skip.
/// A run of back-to-back skips carries it forward; later skips leave it put,
/// so the note shows once per lesson instead of on every step.
pub(super) fn voice_note_step(note: Option<Step>, from: Step, to: Step) -> Option<Step> {
    match note {
        None => Some(to),
        Some(at) if at == from => Some(to),
        kept => kept,
    }
}

/// Whether the text after a blank starts with punctuation that should sit
/// right against the filled word.
pub(super) fn blank_hugs_next(after: &str) -> bool {
    after.starts_with([',', '.', '!', '?', ';', ':', ')', '…'])
}

/// The note shows on its one step, and again in the summary.
pub(super) fn show_voice_note(note: Option<Step>, skips: u32, step: Step) -> bool {
    skips > 0 && (note == Some(step) || step == Step::Summary)
}

/// The mark for one option. Nothing is marked until the answer is checked;
/// other options stay unmarked.
pub(super) fn choice_mark(locked: bool, is_answer: bool, is_chosen: bool) -> Option<ChoiceMark> {
    match (locked, is_answer, is_chosen) {
        (false, _, _) => None,
        (true, true, true) => Some(ChoiceMark::YoursRight),
        (true, true, false) => Some(ChoiceMark::Right),
        (true, false, true) => Some(ChoiceMark::YoursWrong),
        (true, false, false) => None,
    }
}
