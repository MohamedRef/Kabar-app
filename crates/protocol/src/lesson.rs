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
    /// `question_meaning` carries the prompt).
    MultipleChoice {
        #[serde(default)]
        question: String,
        #[serde(default)]
        question_meaning: Option<Localized>,
        options: Vec<String>,
        /// Index into `options`.
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
        #[serde(default)]
        answer: Option<usize>,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "snake_case")]
pub enum LessonStep {
    Slide { index: usize },
    Exercise { id: String },
    Dialogue,
    Summary,
}
