//! Typed outputs per task, shaped like `content/golden/README.md`. Each
//! validates beyond what serde checks; a validation error goes back to the
//! model on the one retry.

use kabar_protocol::ai::Task;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod coach;
mod content;
mod practice;
mod reading;

pub use coach::{
    CoachMessage, Confidence, DailyPlan, DailyRecap, LevelCheck, LevelMove, Placement, PlanKind, PlanStep,
    WeeklyReport,
};
pub use content::{RadioLine, RadioScript};
pub use practice::{
    CheckStrokes, Correction, GuessSketch, MapBranch, MindMap, ReadHandwriting, RoleplayTurn, UNCLEAR_SKETCH,
};
pub use reading::{ExplainSentence, LineTranslation, TranslateText};
// structure_screenshot's answer lives with the rest of the screenshot code.
pub use crate::reader::screen_ai::{ScreenKind, ScreenMessage, StructureScreenshot};
pub use crate::reader::screenshot::Side;

pub trait AiOutput: DeserializeOwned {
    const TASK: Task;

    /// False when a golden set's `expect` is an abridged illustration (a
    /// story's opening lines, two branches of a map) rather than a whole
    /// answer; `content/golden/<task>.yaml` then carries the full
    /// requirements in `criteria`. [`check`](Self::check) is skipped for
    /// those expects, never for a live answer.
    const EXPECT_IS_WHOLE_ANSWER: bool = true;

    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    /// Checks against the request input (plan minutes add up to the goal, ids
    /// come from the candidates). Runs after [`validate`](Self::validate).
    fn check(&self, _input: &Value) -> Result<(), String> {
        Ok(())
    }

    /// The answer must be shown as unverified until the learner approves it
    /// (Javanese content, or the model said it was unsure).
    fn needs_review(&self) -> bool {
        false
    }

    /// The request alone makes any answer unverified (a Javanese text,
    /// persona or voice).
    fn input_needs_review(_input: &Value) -> bool {
        false
    }
}

/// Parses and checks one reply for `task` exactly as the router would
/// (`validate`, then `check` against the input), returning the parsed output
/// as canonical JSON. For callers that hold a task id rather than a type:
/// the bake-off, tools, logs.
pub fn check_reply(task: Task, text: &str, input: &Value) -> Result<Value, String> {
    fn run<T: AiOutput + Serialize>(text: &str, input: &Value) -> Result<Value, String> {
        let v: T = super::json::parse(text)?;
        v.validate()?;
        v.check(input)?;
        serde_json::to_value(&v).map_err(|e| e.to_string())
    }
    match task {
        Task::AnalyzeText => run::<AnalyzeText>(text, input),
        Task::ExplainSentence => run::<ExplainSentence>(text, input),
        Task::TranslateText => run::<TranslateText>(text, input),
        Task::GradeAnswer => run::<GradeAnswer>(text, input),
        Task::RoleplayTurn => run::<RoleplayTurn>(text, input),
        Task::LearnEntry => run::<crate::learn::LearnEntry>(text, input),
        Task::StructureScreenshot => run::<StructureScreenshot>(text, input),
        Task::ReadHandwriting => run::<ReadHandwriting>(text, input),
        Task::GuessSketch => run::<GuessSketch>(text, input),
        Task::CheckStrokes => run::<CheckStrokes>(text, input),
        Task::MindMap => run::<MindMap>(text, input),
        Task::StoryEpisode => run::<crate::pagi::Story>(text, input),
        Task::RadioScript => run::<RadioScript>(text, input),
        Task::PlacementTest => run::<Placement>(text, input),
        Task::LevelCheck => run::<LevelCheck>(text, input),
        Task::DailyPlan => run::<DailyPlan>(text, input),
        Task::DailyRecap => run::<DailyRecap>(text, input),
        Task::WeeklyReport => run::<WeeklyReport>(text, input),
        Task::CoachMessage => run::<CoachMessage>(text, input),
    }
}

/// Speech register or style, shared by the reading and roleplay tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Register {
    Chat,
    Formal,
    Neutral,
    Slang,
    Mixed,
    Casual,
    Standard,
    Ngoko,
    Madya,
    Krama,
    KramaInggil,
}

/// A word with its English and Arabic gloss (stories, mind maps).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlossedWord {
    pub word: String,
    pub gloss_en: String,
    pub gloss_ar: String,
}

impl GlossedWord {
    fn validate(&self) -> Result<(), String> {
        filled(&self.word, "word")?;
        filled(&self.gloss_en, &format!("gloss_en of `{}`", self.word))?;
        arabic(&self.gloss_ar, &format!("gloss_ar of `{}`", self.word))
    }
}

fn filled(s: &str, field: &str) -> Result<(), String> {
    if s.trim().is_empty() { Err(format!("{field} must not be empty")) } else { Ok(()) }
}

fn has_arabic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

/// Non-empty and written in Arabic script.
fn arabic(s: &str, field: &str) -> Result<(), String> {
    filled(s, field)?;
    if has_arabic(s) { Ok(()) } else { Err(format!("{field} must be written in Arabic script")) }
}

fn input_str<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

fn input_strs<'a>(input: &'a Value, key: &str) -> Vec<&'a str> {
    input
        .get(key)
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| a.iter().filter_map(Value::as_str).collect())
}

fn input_is_javanese(input: &Value) -> bool {
    input_str(input, "lang") == Some("jv")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenLang {
    Id,
    Jv,
    En,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignedToken {
    pub i: usize,
    pub surface: String,
    pub lang: TokenLang,
    pub lemma: String,
    pub gloss_en: String,
    pub gloss_ar: String,
    #[serde(default)]
    pub note: Option<String>,
    pub confident: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeText {
    pub translation_en: String,
    pub translation_ar: String,
    pub register: String,
    pub tokens: Vec<AlignedToken>,
}

impl AiOutput for AnalyzeText {
    const TASK: Task = Task::AnalyzeText;

    fn validate(&self) -> Result<(), String> {
        if self.translation_en.trim().is_empty() || self.translation_ar.trim().is_empty() {
            return Err("translation_en and translation_ar must not be empty".into());
        }
        if !has_arabic(&self.translation_ar) {
            return Err("translation_ar must be written in Arabic script".into());
        }
        if self.tokens.is_empty() {
            return Err("tokens must list every word".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for t in &self.tokens {
            if !seen.insert(t.i) {
                return Err(format!("token index {} appears twice", t.i));
            }
            if t.gloss_en.trim().is_empty() {
                return Err(format!("token {} (`{}`) has an empty gloss_en", t.i, t.surface));
            }
        }
        Ok(())
    }

    fn needs_review(&self) -> bool {
        self.tokens.iter().any(|t| t.lang == TokenLang::Jv || !t.confident)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Correct,
    Almost,
    Wrong,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MistakeKind {
    Spelling,
    Grammar,
    WordChoice,
    Register,
    Missing,
    Extra,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mistake {
    pub kind: MistakeKind,
    pub text: String,
    pub fix: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradeAnswer {
    pub verdict: Verdict,
    pub score: u8,
    pub corrected: Option<String>,
    pub feedback_en: String,
    pub feedback_ar: String,
    #[serde(default)]
    pub mistakes: Vec<Mistake>,
}

impl AiOutput for GradeAnswer {
    const TASK: Task = Task::GradeAnswer;

    fn validate(&self) -> Result<(), String> {
        if self.score > 100 {
            return Err(format!("score must be 0-100, got {}", self.score));
        }
        let consistent = match self.verdict {
            Verdict::Correct => self.score >= 80,
            Verdict::Almost => (40..=95).contains(&self.score),
            Verdict::Wrong => self.score <= 60,
        };
        if !consistent {
            return Err(format!("score {} does not match verdict {:?}", self.score, self.verdict));
        }
        if self.verdict != Verdict::Correct && self.corrected.as_deref().is_none_or(|c| c.trim().is_empty()) {
            return Err("corrected is required unless the verdict is correct".into());
        }
        if self.feedback_en.trim().is_empty() {
            return Err("feedback_en must not be empty".into());
        }
        Ok(())
    }
}
