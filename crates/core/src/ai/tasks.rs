//! Typed outputs per task. Each validates beyond what serde checks; a
//! validation error goes back to the model on the one retry.

use kabar_protocol::ai::Task;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait AiOutput: DeserializeOwned {
    const TASK: Task;

    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    /// Checks that need the request input too (e.g. every chat message must
    /// be traceable to the OCR text it came from). Runs after `validate`, on
    /// fresh replies and on cache hits alike.
    fn validate_against(&self, input: &Value) -> Result<(), String> {
        let _ = input;
        Ok(())
    }

    /// The answer must be shown as unverified until the learner approves it
    /// (Javanese content, or the model said it was unsure).
    fn needs_review(&self) -> bool {
        false
    }
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
        if !self.translation_ar.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c)) {
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
