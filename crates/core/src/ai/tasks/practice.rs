//! Practice tasks: roleplay, the pen (handwriting, sketches, strokes) and
//! mind maps.

use std::collections::BTreeSet;

use kabar_protocol::ai::Task;
use kabar_protocol::story::canonical_persona_id;
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

    /// Each correction quotes the learner's newest line, and the reply's
    /// register fits the persona: the Indonesian personas never answer in a
    /// Javanese speech level, and the Javanese ones answer in one once the
    /// learner is on the Javanese journey (`j*`).
    fn check(&self, input: &Value) -> Result<(), String> {
        use Register::*;
        let learner = input_str(input, "learner").map(fold).unwrap_or_default();
        for (i, c) in self.corrections.iter().enumerate() {
            if !learner.contains(&fold(&c.text)) {
                return Err(format!("corrections[{i}].text must be copied from the learner's line"));
            }
        }
        let javanese_reply = matches!(self.register, Ngoko | Madya | Krama | KramaInggil);
        let persona = input_str(input, "persona").unwrap_or_default();
        let level = input_str(input, "level").unwrap_or_default();
        if persona_language(persona) == Some("id") && javanese_reply {
            return Err(format!("{persona} speaks Indonesian; register must be casual or standard"));
        }
        if persona_language(persona) == Some("jv") && level.starts_with('j') && !javanese_reply {
            return Err(format!(
                "{persona} speaks Javanese at {level}; register must be ngoko, madya, krama or krama_inggil"
            ));
        }
        Ok(())
    }

    fn input_needs_review(input: &Value) -> bool {
        input_str(input, "persona").and_then(persona_language) == Some("jv")
            || input_str(input, "level").is_some_and(|l| l.starts_with('j'))
    }
}

/// The language a chat persona speaks (`content/personas/*.yaml`), for ids in
/// either spelling (`mas-dimas`, `mas_dimas`). A test keeps it in step with
/// the files.
pub(crate) fn persona_language(id: &str) -> Option<&'static str> {
    match canonical_persona_id(id).as_str() {
        "mbak-gadis" | "mas-ardi" | "bu-lestari" | "bang-togar" | "kaka-yohana" => Some("id"),
        "mas-dimas" | "bu-siti" | "mbah-putri" | "mbak-ayu" | "pak-slamet" => Some("jv"),
        _ => None,
    }
}

/// Lowercase with runs of whitespace and punctuation as one space, so a
/// quote matches whatever spacing or capitals the model copied it with.
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for word in s.split(|c: char| c.is_whitespace() || (c.is_ascii_punctuation() && c != '-' && c != '\'')) {
        if !word.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.extend(word.chars().flat_map(char::to_lowercase));
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::persona_language;

    #[test]
    fn every_persona_file_has_its_language_listed() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/personas");
        let mut seen = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "yaml") {
                continue;
            }
            let p = crate::story::parse_persona(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(persona_language(&p.id), Some(p.language.as_str()), "{}", path.display());
            assert_eq!(persona_language(&p.id.replace('-', "_")), Some(p.language.as_str()));
            seen += 1;
        }
        assert!(seen >= 10, "found {seen} persona files");
    }
}
