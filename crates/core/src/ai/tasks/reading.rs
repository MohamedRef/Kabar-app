//! Reader tasks: explain a sentence, translate a text line by line. (structure_screenshot's answer is in
//! `reader::screen_ai`.)

use std::collections::BTreeSet;

use kabar_protocol::ai::Task;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{AiOutput, Register, arabic, filled, has_arabic, input_is_javanese, input_str, input_strs};

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

/// One input line in all three voices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineTranslation {
    /// Index into the input's `lines`.
    pub i: usize,
    /// Egyptian colloquial, Arabic script.
    pub arz: String,
    /// Modern Standard Arabic.
    pub ar: String,
    pub en: String,
}

/// The Reader's translation shown under each line of a text: natural, in
/// context (chat spellings, slang, speech levels), one entry per input line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranslateText {
    pub lines: Vec<LineTranslation>,
    pub register: Register,
}

impl AiOutput for TranslateText {
    const TASK: Task = Task::TranslateText;

    fn validate(&self) -> Result<(), String> {
        if self.lines.is_empty() {
            return Err("lines must have one entry per input line".into());
        }
        let mut seen = BTreeSet::new();
        for l in &self.lines {
            if !seen.insert(l.i) {
                return Err(format!("line {} appears twice", l.i));
            }
            arabic(&l.arz, &format!("arz of line {}", l.i))?;
            arabic(&l.ar, &format!("ar of line {}", l.i))?;
            filled(&l.en, &format!("en of line {}", l.i))?;
            if has_arabic(&l.en) {
                return Err(format!("en of line {} must be in English", l.i));
            }
        }
        Ok(())
    }

    fn check(&self, input: &Value) -> Result<(), String> {
        let n = input_strs(input, "lines").len();
        if self.lines.len() != n {
            return Err(format!("{n} input lines need {n} translations, got {}", self.lines.len()));
        }
        if let Some(l) = self.lines.iter().find(|l| l.i >= n) {
            return Err(format!("line index {} is out of range (0..{n})", l.i));
        }
        Ok(())
    }

    fn input_needs_review(input: &Value) -> bool {
        input_is_javanese(input)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn answer(lines: Value) -> TranslateText {
        serde_json::from_value(json!({ "lines": lines, "register": "chat" })).unwrap()
    }

    #[test]
    fn every_line_is_translated_once_in_each_voice() {
        let input = json!({ "lang": "jv", "lines": ["arp dicul piro", "Sek tak lapor pusat"] });
        let fixture =
            include_str!("../../../../../fixtures/ai/translate_text.arp_dicul_piro.workers_ai.json");
        let ok: TranslateText = serde_json::from_str(fixture).unwrap();
        assert_eq!(ok.validate(), Ok(()));
        assert_eq!(ok.check(&input), Ok(()));
        assert!(TranslateText::input_needs_review(&input));

        let one = answer(json!([{ "i": 0, "arz": "بكام؟", "ar": "بكم؟", "en": "How much?" }]));
        assert!(one.check(&input).unwrap_err().contains("2 input lines"));
        let twice = answer(json!([
            { "i": 0, "arz": "بكام؟", "ar": "بكم؟", "en": "How much?" },
            { "i": 0, "arz": "بكام؟", "ar": "بكم؟", "en": "How much?" },
        ]));
        assert!(twice.validate().unwrap_err().contains("twice"));
        let latin = answer(json!([{ "i": 0, "arz": "bikam", "ar": "بكم؟", "en": "How much?" }]));
        assert!(latin.validate().unwrap_err().contains("Arabic script"));
        let arabic_en = answer(json!([{ "i": 0, "arz": "بكام؟", "ar": "بكم؟", "en": "بكام" }]));
        assert!(arabic_en.validate().is_err());
    }
}
