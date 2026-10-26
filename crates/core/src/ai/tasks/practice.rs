//! Practice tasks: roleplay, the pen (handwriting, sketches, strokes) and
//! mind maps.

use std::collections::BTreeSet;

use kabar_protocol::ai::Task;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{AiOutput, GlossedWord, Register, arabic, filled, input_is_javanese, input_str};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Correction {
    /// The learner's words as written.
    pub text: String,
    pub fix: String,
    pub why_en: String,
    pub why_ar: String,
}

/// One persona turn in a roleplay, with gentle corrections of the learner's
/// last line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleplayTurn {
    pub reply: String,
    /// Empty when the learner was right.
    #[serde(default)]
    pub corrections: Vec<Correction>,
    /// The register of the reply itself.
    pub register: Register,
}

impl AiOutput for RoleplayTurn {
    const TASK: Task = Task::RoleplayTurn;

    fn validate(&self) -> Result<(), String> {
        use Register::*;
        filled(&self.reply, "reply")?;
        if !matches!(self.register, Casual | Standard | Ngoko | Madya | Krama | KramaInggil) {
            return Err("register must be one of casual, standard, ngoko, madya, krama, krama_inggil".into());
        }
        for (i, c) in self.corrections.iter().enumerate() {
            filled(&c.text, &format!("corrections[{i}].text"))?;
            filled(&c.why_en, &format!("corrections[{i}].why_en"))?;
            arabic(&c.why_ar, &format!("corrections[{i}].why_ar"))?;
            if c.fix.trim() == c.text.trim() {
                return Err(format!("corrections[{i}]: fix is the same as text; drop it"));
            }
        }
        Ok(())
    }

    fn input_needs_review(input: &Value) -> bool {
        matches!(input_str(input, "persona"), Some("mas_dimas" | "bu_siti"))
            || input_str(input, "level").is_some_and(|l| l.starts_with('j'))
    }
}

/// What is written on the canvas, exactly; empty for a blank or scribble.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadHandwriting {
    pub text: String,
}

impl AiOutput for ReadHandwriting {
    const TASK: Task = Task::ReadHandwriting;
}

/// `guess_id` for a blank or unreadable canvas.
pub const UNCLEAR_SKETCH: &str = "tidak_jelas";

/// Gambar!: what the sketch shows, as Indonesian lemmas in snake_case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuessSketch {
    pub guess_id: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
}

fn is_lemma_id(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('_')
        && !s.ends_with('_')
        && s.chars().all(|c| c.is_ascii_lowercase() || c == '_')
}

impl AiOutput for GuessSketch {
    const TASK: Task = Task::GuessSketch;

    fn validate(&self) -> Result<(), String> {
        for id in std::iter::once(&self.guess_id).chain(&self.alternatives) {
            if !is_lemma_id(id) {
                return Err(format!("`{id}` must be one Indonesian lemma in snake_case, like nasi_goreng"));
            }
        }
        if self.guess_id == UNCLEAR_SKETCH && !self.alternatives.is_empty() {
            return Err(format!("{UNCLEAR_SKETCH} has no alternatives"));
        }
        if self.alternatives.len() > 3 {
            return Err("at most 3 alternatives".into());
        }
        let mut seen = BTreeSet::from([self.guess_id.as_str()]);
        if let Some(a) = self.alternatives.iter().find(|a| !seen.insert(a.as_str())) {
            return Err(format!("alternative `{a}` repeats the guess or another alternative"));
        }
        Ok(())
    }
}

/// Calligraphy and aksara feedback on one written glyph or word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckStrokes {
    pub correct: bool,
    #[serde(default)]
    pub issues: Vec<String>,
}

impl AiOutput for CheckStrokes {
    const TASK: Task = Task::CheckStrokes;

    fn validate(&self) -> Result<(), String> {
        if !self.correct && self.issues.is_empty() {
            return Err("name at least one issue when the attempt is not correct".into());
        }
        if self.issues.iter().any(|i| i.trim().is_empty()) {
            return Err("issues must not hold empty strings".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapBranch {
    pub label: String,
    pub words: Vec<GlossedWord>,
}

/// A topic, its sub-themes and the words under each.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MindMap {
    pub center: String,
    pub branches: Vec<MapBranch>,
}

impl AiOutput for MindMap {
    const TASK: Task = Task::MindMap;
    const EXPECT_IS_WHOLE_ANSWER: bool = false;

    fn validate(&self) -> Result<(), String> {
        filled(&self.center, "center")?;
        if self.branches.is_empty() {
            return Err("the map needs branches".into());
        }
        let mut seen = BTreeSet::new();
        for b in &self.branches {
            filled(&b.label, "branch label")?;
            if !(2..=6).contains(&b.words.len()) {
                return Err(format!("branch `{}`: 2-6 words, got {}", b.label, b.words.len()));
            }
            for w in &b.words {
                w.validate()?;
                if !seen.insert(w.word.to_lowercase()) {
                    return Err(format!("`{}` appears twice in the map", w.word));
                }
            }
        }
        Ok(())
    }

    fn check(&self, _input: &Value) -> Result<(), String> {
        if !(3..=6).contains(&self.branches.len()) {
            return Err(format!("3-6 branches, got {}", self.branches.len()));
        }
        Ok(())
    }

    fn input_needs_review(input: &Value) -> bool {
        input_is_javanese(input)
    }
}
