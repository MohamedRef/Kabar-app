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
    /// A Play game this lesson opens at its end: an Unggah-Ungguh scene
    /// (`uu.grandmother`) or a Minimal Pairs pair (`mp.wedi-wedhi`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<String>,
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
    /// A small comparison table (e.g. ngoko / krama pairs), shown under the body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<SlideTable>,
}

/// Target-language columns plus a meaning column in the reader's language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideTable {
    /// Column labels, target-language or linguistic terms ("Indonesia", "ngoko").
    pub head: Vec<String>,
    pub rows: Vec<TableRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    /// One target-language cell per `head` column.
    pub cells: Vec<String>,
    #[serde(flatten)]
    pub meaning: Localized,
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
    /// Hear `text` (pre-generated audio, else system TTS), then pick or type
    /// it, or (with `question`) answer a question about it.
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
        /// Listening comprehension: a question in the learner's language
        /// about what was heard. Then `options[answer]` answers it (and need
        /// not be `text`); without it the options are transcripts.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        question: Option<Localized>,
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
    /// The Bridge card it shares when its own spelling has none
    /// (bersalaman → salam, Senen → senin).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<String>,
    /// Javanese only: the Indonesian word spelled the same with another
    /// meaning (mau "a moment ago" ≠ Indonesian mau "want"). Shown as a badge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub false_friend: Option<String>,
    /// Javanese only: spelled like an Indonesian word and means the same,
    /// although the glosses are worded differently (kapan, salam).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub same_as_id: bool,
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
    /// The answer on screen at `step`, so a reopened lesson shows it again
    /// and, once checked, its feedback instead of a fresh Check. Absent in
    /// positions written before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<SavedResponse>,
}

/// The learner's answer to the exercise on screen, kept by content (option
/// and tile texts, pair words), never by on-screen positions, which a
/// reshuffle or a content edit would change.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedResponse {
    /// The picked option's text (`options`, or the English of
    /// `options_meaning` for meaning choices).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    /// Typed text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Build sentence: the picked tiles, in answer order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiles: Vec<String>,
    /// Match pairs: target words of the pairs already matched, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched: Vec<String>,
    /// Match pairs: target words of wrong taps so far.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missed: Vec<String>,
    /// Set once the answer was checked: the grade its feedback shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<SavedGrade>,
}

impl SavedResponse {
    /// Nothing picked, typed or checked.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A checked answer's grade, as its feedback shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "grade", rename_all = "snake_case")]
pub enum SavedGrade {
    Correct,
    Almost { expected: String },
    Wrong,
    NeedsReview,
    Practiced,
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
    /// Whether passing this run also counts as the learner's check of the
    /// lesson's content: the rule until 2026-10-29, when finishing and
    /// checking the content became two things (a `verified` event is the
    /// check now). Events written before then lack the field and keep
    /// counting; newer builds write false.
    #[serde(default = "legacy_pass_verifies")]
    pub pass_verifies: bool,
}

fn legacy_pass_verifies() -> bool {
    true
}

/// Payload of a `verified` row in `lesson_events` (the learner checked the
/// lesson's content and found it right) and of an `unverified` row (they
/// took that back). The newest of the two by HLC stands.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LessonVerified {}

/// Why a lesson counts as checked, shown next to it so it's never a guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    /// The learner tapped "the content is right".
    Checked,
    /// Passed with a good score under the old rule, before the two were
    /// separated (kept, never reset).
    PassedEarlier { score: u32 },
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
        assert_eq!(p.response, None);
        // Unused new fields stay out of the JSON, so old readers see the same shape.
        assert_eq!(serde_json::to_string(&p).unwrap(), json);
    }

    #[test]
    fn a_saved_response_round_trips() {
        let p = LessonPosition {
            lesson_id: "bipa1.u01.l01".into(),
            step: LessonStep::Exercise { id: "e07".into(), retry: false },
            progress: None,
            response: Some(SavedResponse {
                tiles: vec!["Selamat".into(), "jalan,".into()],
                checked: Some(SavedGrade::Almost { expected: "Selamat jalan".into() }),
                ..SavedResponse::default()
            }),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""checked":{"grade":"almost","expected":"Selamat jalan"}"#), "{json}");
        assert_eq!(serde_json::from_str::<LessonPosition>(&json).unwrap(), p);
        assert!(SavedResponse::default().is_empty());
    }

    #[test]
    fn completions_before_the_split_still_verify() {
        let old = r#"{"score":90,"xp":12,"duration_ms":1000}"#;
        assert!(serde_json::from_str::<LessonCompleted>(old).unwrap().pass_verifies);
        let new = LessonCompleted {
            score: 90,
            xp: 12,
            duration_ms: 1000,
            mistakes: vec![],
            results: vec![],
            pass_verifies: false,
        };
        let json = serde_json::to_string(&new).unwrap();
        assert!(!serde_json::from_str::<LessonCompleted>(&json).unwrap().pass_verifies);
    }
}
