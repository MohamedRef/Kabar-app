//! Grammar library entries (`content/grammar/<lang>/*.yaml`, schema
//! `schemas/grammar-point.schema.json`): one grammar point explained three
//! ways. "Explain this sentence" assembles its answers from these.

use serde::{Deserialize, Serialize};

/// Egyptian Arabic, MSA and English, as the grammar files spell them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Three {
    pub ar_eg: String,
    pub ar: String,
    pub en: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Title {
    /// The forms in the target language ("ada, tidak ada").
    pub target: String,
    pub ar_eg: String,
    pub ar: String,
    pub en: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarExample {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakdown: Option<String>,
    pub en: String,
    pub ar_eg: String,
    pub ar: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub register: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarMistake {
    pub wrong: String,
    pub right: String,
    pub why: Three,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarReview {
    pub status: String,
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarPoint {
    pub id: String,
    /// `id` or `jv`.
    pub language: String,
    /// `bipa1`…`bipa7`, `j1`…`j6`.
    pub level: String,
    pub title: Title,
    pub summary: Three,
    #[serde(default)]
    pub pattern: Vec<String>,
    pub explanation: Three,
    pub examples: Vec<GrammarExample>,
    #[serde(default)]
    pub mistakes: Vec<GrammarMistake>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arabic_bridge: Option<Three>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub see_also: Vec<String>,
    pub review: GrammarReview,
}
