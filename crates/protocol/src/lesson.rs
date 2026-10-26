//! Lesson content (SPEC §14): teaching slides → exercises → mini dialogue →
//! new-words summary → souvenir. Written at build time under
//! `content/lessons/<language>/<lesson-id>.yaml`, shipped in content packs,
//! read by the Lesson Player. Shape: `schemas/lesson.schema.json`.

use serde::{Deserialize, Serialize};

/// The `schema:` tag every lesson file carries.
pub const LESSON_SCHEMA: &str = "kabar.lesson/v1";

/// One text in Egyptian Arabic, MSA and English (same field names as the
/// grammar library, so content files read alike).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Localized {
    pub ar_eg: String,
    pub ar: String,
    pub en: String,
}

/// Review state of build-time content; Javanese stays unverified until approved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentReview {
    /// `draft`, `approved` or `needs-changes`.
    pub status: String,
    pub verified: bool,
}

impl Default for ContentReview {
    fn default() -> Self {
        Self { status: "draft".into(), verified: false }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LessonContent {
    /// Always [`LESSON_SCHEMA`].
    pub schema: String,
    /// Matches the curriculum lesson id, e.g. `bipa1.u01.l01`.
    pub id: String,
    /// `id` (Indonesian) or `jv` (Javanese).
    pub language: String,
    /// Target-language title, as in the curriculum.
    pub title: String,
    pub can_do: Localized,
    /// 3–6 teaching slides (practice and review lessons may have none).
    #[serde(default)]
    pub slides: Vec<Slide>,
    /// 8–15 exercises.
    pub exercises: Vec<Exercise>,
    #[serde(default)]
    pub dialogue: Option<Dialogue>,
    #[serde(default)]
    pub new_words: Vec<NewWord>,
    /// Earned when the lesson is finished; kept in the Suitcase.
    #[serde(default)]
    pub souvenir: Option<Souvenir>,
    #[serde(default)]
    pub review: ContentReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slide {
    /// Target-language heading, e.g. "Selamat pagi!".
    #[serde(default)]
    pub title: Option<String>,
    pub body: Localized,
    #[serde(default)]
    pub examples: Vec<Example>,
    /// Grammar-library id this slide teaches, for "explain more".
    #[serde(default)]
    pub grammar: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Example {
    /// Target-language sentence.
    pub text: String,
    #[serde(flatten)]
    pub meaning: Localized,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exercise {
    /// Unique inside the lesson, e.g. `e01`; Lanjut resumes by it.
    pub id: String,
    #[serde(flatten)]
    pub kind: ExerciseKind,
    /// Shown after answering: why the answer is what it is.
    #[serde(default)]
    pub explain: Option<Localized>,
    /// Lemmas this exercise trains (feeds review cards and weak-word tracking).
    #[serde(default)]
    pub words: Vec<String>,
}

/// Which way a translation goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Arabic/English prompt → answer in Indonesian/Javanese.
    ToTarget,
    /// Indonesian/Javanese prompt → answer in the learner's language.
    FromTarget,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExerciseKind {
    /// Pick one option. `question` is target-language text (or empty when
    /// `question_meaning` carries the prompt). Options are either
    /// target-language strings (`options`) or meanings in the learner's
    /// language (`options_meaning`): exactly one of the two is non-empty.
    MultipleChoice {
        #[serde(default)]
        question: String,
        #[serde(default)]
        question_meaning: Option<Localized>,
        #[serde(default)]
        options: Vec<String>,
        /// "What does X mean?" options, shown in the UI language.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        options_meaning: Vec<Localized>,
        /// Index into `options` (or `options_meaning`).
        answer: usize,
    },
    /// Tap a word, then its meaning.
    MatchPairs { pairs: Vec<Pair> },
    /// Free text or handwriting, graded against reference answers first
    /// (AI `grade_answer` only for open answers that miss every reference).
    Translate {
        direction: Direction,
        /// `to_target`: the meaning to express.
        #[serde(default)]
        source_meaning: Option<Localized>,
        /// `from_target`: the sentence to translate.
        #[serde(default)]
        source_text: Option<String>,
        /// `to_target`: accepted target-language answers.
        #[serde(default)]
        answers: Vec<String>,
        /// `from_target`: accepted answers per explanation language.
        #[serde(default)]
        answers_meaning: Option<AcceptedMeanings>,
    },
    /// Arrange tiles into the sentence. Tiles = the answer's words + `distractors`.
    BuildSentence {
        source: Localized,
        answer: String,
        #[serde(default)]
        alternatives: Vec<String>,
        #[serde(default)]
        distractors: Vec<String>,
    },
    /// Hear `text` (pre-generated audio, else system TTS), then pick or type it.
    Listen {
        text: String,
        #[serde(default)]
        audio: Option<String>,
        /// Empty → type what you heard.
        #[serde(default)]
        options: Vec<String>,
        /// Index into `options`; `options[answer]` is `text`. None when typed.
        #[serde(default)]
        answer: Option<usize>,
        /// Shown with the feedback, so hearing it also teaches what it means.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        meaning: Option<Localized>,
    },
    /// `sentence` holds one `___`. Empty `options` → typed answer.
    FillBlank {
        sentence: String,
        meaning: Localized,
        #[serde(default)]
        options: Vec<String>,
        answers: Vec<String>,
    },
    /// Hear it, write it exactly (typed or handwritten).
    Dictation {
        text: String,
        #[serde(default)]
        audio: Option<String>,
        meaning: Localized,
    },
    /// Say it aloud. Phased: shown as listen-and-repeat until speech
    /// recognition lands (SPEC §20).
    Speaking { text: String, meaning: Localized },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pair {
    pub text: String,
    #[serde(flatten)]
    pub meaning: Localized,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedMeanings {
    #[serde(default)]
    pub ar_eg: Vec<String>,
    #[serde(default)]
    pub ar: Vec<String>,
    #[serde(default)]
    pub en: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dialogue {
    #[serde(default)]
    pub title: Option<String>,
    pub lines: Vec<DialogueLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueLine {
    pub speaker: String,
    pub text: String,
    #[serde(flatten)]
    pub meaning: Localized,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewWord {
    pub word: String,
    #[serde(default)]
    pub pos: Option<String>,
    #[serde(flatten)]
    pub meaning: Localized,
    /// Arabic origin when it's an Arabic loan (Bridge gold), e.g. "خبر".
    #[serde(default)]
    pub arabic: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Souvenir {
    pub id: String,
    /// Target-language name, e.g. "Batik Betawi".
    pub title: String,
    pub note: Localized,
}

/// Where the learner is inside a lesson, for Lanjut (handoff) and resume.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LessonPosition {
    pub lesson_id: String,
    pub step: LessonStep,
    /// Results so far, so a resumed lesson (on this or another device) keeps
    /// its score and retries. Absent in positions written before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<LessonProgress>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "snake_case")]
pub enum LessonStep {
    Slide {
        index: usize,
    },
    Exercise {
        id: String,
        /// The end-of-lesson second try of an exercise missed the first time.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        retry: bool,
    },
    Dialogue,
    Summary,
}

/// Results of the exercises answered so far in one run through a lesson.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LessonProgress {
    #[serde(default)]
    pub results: Vec<ExerciseResult>,
    /// Active time spent in the lesson so far.
    #[serde(default)]
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExerciseResult {
    /// The exercise id.
    pub id: String,
    pub first: Outcome,
    /// The end-of-lesson retry, when the first try was missed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<Outcome>,
    /// Words to practise again (feeds weak-word tracking).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mistakes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Correct,
    /// Right apart from a small typo.
    Almost,
    Missed,
    /// Not graded: speaking practice (until speech checking lands).
    Practiced,
    /// "Can't listen now": no score, no retry.
    Skipped,
}

/// Payload of a `completed` row in `lesson_events`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LessonCompleted {
    /// First-try score, 0–100.
    pub score: u32,
    pub xp: u32,
    pub duration_ms: u64,
    /// Words the learner missed, for the review queue.
    #[serde(default)]
    pub mistakes: Vec<String>,
    #[serde(default)]
    pub results: Vec<ExerciseResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_positions_still_parse() {
        let json = r#"{"lesson_id":"bipa1.u01.l01","step":{"at":"exercise","id":"e03"}}"#;
        let p: LessonPosition = serde_json::from_str(json).unwrap();
        assert_eq!(p.step, LessonStep::Exercise { id: "e03".into(), retry: false });
        assert_eq!(p.progress, None);
        // Unused new fields stay out of the JSON, so old readers see the same shape.
        assert_eq!(serde_json::to_string(&p).unwrap(), json);
    }
}
