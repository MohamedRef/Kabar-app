//! Typed view of the content files. The JSON Schemas in `schemas/` are the
//! source of truth for shape; these structs carry what the app and the
//! semantic checks read. Unknown fields are ignored here on purpose.

use serde::{Deserialize, Serialize};

/// A short label: target-language form (optional), English and MSA.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Text {
    #[serde(default)]
    pub target: Option<String>,
    pub en: String,
    pub ar: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curriculum {
    pub language: String,
    pub title: Text,
    pub levels: Vec<Level>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub id: String,
    pub code: String,
    pub title: Text,
    #[serde(default)]
    pub requires: Vec<String>,
    pub units: Vec<Unit>,
    pub level_test: LevelTest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelTest {
    pub id: String,
    pub minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    pub id: String,
    pub title: Text,
    pub stop: String,
    pub can_do: Vec<String>,
    #[serde(default)]
    pub grammar: Vec<String>,
    pub vocabulary: Vec<String>,
    pub lessons: Vec<Lesson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub can_do: String,
    #[serde(default)]
    pub grammar: Vec<String>,
    pub minutes: u32,
}

/// Egyptian Arabic, MSA and English versions of one text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Three {
    pub ar_eg: String,
    pub ar: String,
    pub en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarPoint {
    pub id: String,
    pub language: String,
    pub level: String,
    pub summary: Three,
    pub explanation: Three,
    pub examples: Vec<Example>,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub see_also: Vec<String>,
    pub review: Review,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub text: String,
    pub en: String,
    pub ar_eg: String,
    pub ar: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub status: String,
    pub verified: bool,
}
