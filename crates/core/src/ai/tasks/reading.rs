//! Reader tasks: explain a sentence, structure a screenshot.

use std::collections::BTreeSet;

use kabar_protocol::ai::Task;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{AiOutput, Register, filled, has_arabic, input_is_javanese, input_str, input_strs};

/// "Explain this sentence": the grammar entries it uses, plus glue text in
/// the learner's chosen voice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplainSentence {
    /// Ids from the input's `grammar_candidates`, most important first.
    pub grammar_ids: Vec<String>,
    pub register: Register,
    #[serde(default)]
    pub culture: Option<String>,
    /// The whole sentence, rendered naturally in the voice's language.
    pub paraphrase: String,
    pub explanation: String,
}

impl AiOutput for ExplainSentence {
    const TASK: Task = Task::ExplainSentence;

    fn validate(&self) -> Result<(), String> {
        use Register::*;
        if !matches!(self.register, Chat | Formal | Neutral | Slang | Krama | Ngoko | Mixed) {
            return Err("register must be one of chat, formal, neutral, slang, krama, ngoko, mixed".into());
        }
        filled(&self.paraphrase, "paraphrase")?;
        filled(&self.explanation, "explanation")?;
        let mut seen = BTreeSet::new();
        if let Some(dup) = self.grammar_ids.iter().find(|id| !seen.insert(id.as_str())) {
            return Err(format!("grammar id `{dup}` appears twice"));
        }
        Ok(())
    }

    fn check(&self, input: &Value) -> Result<(), String> {
        let candidates = input_strs(input, "grammar_candidates");
        if let Some(id) = self.grammar_ids.iter().find(|id| !candidates.contains(&id.as_str())) {
            return Err(format!("grammar id `{id}` is not one of grammar_candidates"));
        }
        let arabic_voice = matches!(input_str(input, "voice"), Some("arz" | "ar"));
        if arabic_voice && !(has_arabic(&self.paraphrase) && has_arabic(&self.explanation)) {
            return Err("paraphrase and explanation must be in Arabic script for this voice".into());
        }
        if !arabic_voice && has_arabic(&self.paraphrase) {
            return Err("paraphrase must be in English for this voice".into());
        }
        Ok(())
    }

    fn input_needs_review(input: &Value) -> bool {
        input_is_javanese(input)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenKind {
    Chat,
    Listing,
    Post,
    Ui,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Center,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenMessage {
    pub speaker: String,
    pub side: Side,
    pub text: String,
    /// As printed on the screen (`10.42`); absent when none is shown.
    #[serde(default)]
    pub time: Option<String>,
}

/// A screenshot's OCR blocks, put in order: who said what, top to bottom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureScreenshot {
    pub kind: ScreenKind,
    pub messages: Vec<ScreenMessage>,
}

impl AiOutput for StructureScreenshot {
    const TASK: Task = Task::StructureScreenshot;

    fn validate(&self) -> Result<(), String> {
        if self.messages.is_empty() {
            return Err("messages must list every bubble or block on the screen".into());
        }
        for (i, m) in self.messages.iter().enumerate() {
            filled(&m.text, &format!("messages[{i}].text"))?;
            filled(&m.speaker, &format!("messages[{i}].speaker"))?;
            match (self.kind, m.side) {
                (ScreenKind::Chat, Side::Center) => {
                    return Err(format!("messages[{i}]: chat bubbles are left or right"));
                }
                (ScreenKind::Chat, Side::Right) if m.speaker != "me" => {
                    return Err(format!("messages[{i}]: right-hand bubbles belong to speaker `me`"));
                }
                (ScreenKind::Listing | ScreenKind::Post | ScreenKind::Ui, Side::Left | Side::Right) => {
                    return Err(format!("messages[{i}]: only chat bubbles have a side; use center"));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
