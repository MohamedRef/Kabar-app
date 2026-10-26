//! AI router wire types: shared by the app (which routes), Kabar Cloud
//! (which proxies `workers_ai` / `gemini_free` / `claude`) and Kabar Home
//! (which serves `local`).

use serde::{Deserialize, Serialize};

/// Bumped when `EngineCall` / `EngineReply` change incompatibly.
pub const AI_WIRE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineId {
    /// Tokenizer, normalizer, dictionary, grammar library. Always tried first.
    Deterministic,
    /// Cloudflare Workers AI (SEA-LION v4 27B), free daily Neurons.
    WorkersAi,
    /// Ollama on Kabar Home.
    Local,
    /// Gemini API free tier. Inputs may train Google's models: learning text only.
    GeminiFree,
    /// Anthropic API. Off unless the learner enables it.
    Claude,
}

impl EngineId {
    pub const ALL: [EngineId; 5] =
        [Self::Deterministic, Self::WorkersAi, Self::Local, Self::GeminiFree, Self::Claude];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::WorkersAi => "workers_ai",
            Self::Local => "local",
            Self::GeminiFree => "gemini_free",
            Self::Claude => "claude",
        }
    }

    /// The provider may keep or train on what it is sent.
    pub fn trains_on_inputs(self) -> bool {
        matches!(self, Self::GeminiFree)
    }
}

/// Every runtime AI task (spec §19).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    AnalyzeText,
    ExplainSentence,
    GradeAnswer,
    RoleplayTurn,
    PlacementTest,
    LevelCheck,
    DailyPlan,
    DailyRecap,
    WeeklyReport,
    CoachMessage,
    StoryEpisode,
    RadioScript,
    StructureScreenshot,
    ReadHandwriting,
    GuessSketch,
    CheckStrokes,
    MindMap,
}

impl Task {
    pub const ALL: [Task; 17] = [
        Self::AnalyzeText,
        Self::ExplainSentence,
        Self::GradeAnswer,
        Self::RoleplayTurn,
        Self::PlacementTest,
        Self::LevelCheck,
        Self::DailyPlan,
        Self::DailyRecap,
        Self::WeeklyReport,
        Self::CoachMessage,
        Self::StoryEpisode,
        Self::RadioScript,
        Self::StructureScreenshot,
        Self::ReadHandwriting,
        Self::GuessSketch,
        Self::CheckStrokes,
        Self::MindMap,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnalyzeText => "analyze_text",
            Self::ExplainSentence => "explain_sentence",
            Self::GradeAnswer => "grade_answer",
            Self::RoleplayTurn => "roleplay_turn",
            Self::PlacementTest => "placement_test",
            Self::LevelCheck => "level_check",
            Self::DailyPlan => "daily_plan",
            Self::DailyRecap => "daily_recap",
            Self::WeeklyReport => "weekly_report",
            Self::CoachMessage => "coach_message",
            Self::StoryEpisode => "story_episode",
            Self::RadioScript => "radio_script",
            Self::StructureScreenshot => "structure_screenshot",
            Self::ReadHandwriting => "read_handwriting",
            Self::GuessSketch => "guess_sketch",
            Self::CheckStrokes => "check_strokes",
            Self::MindMap => "mind_map",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// Built from the learner's own data (coach memory, history, journal):
    /// never sent to an engine that trains on inputs.
    pub fn is_personal(self) -> bool {
        matches!(
            self,
            Self::PlacementTest
                | Self::LevelCheck
                | Self::DailyPlan
                | Self::DailyRecap
                | Self::WeeklyReport
                | Self::CoachMessage
        )
    }

    /// Takes an image (screenshot, handwriting, sketch, strokes).
    pub fn needs_vision(self) -> bool {
        matches!(
            self,
            Self::StructureScreenshot | Self::ReadHandwriting | Self::GuessSketch | Self::CheckStrokes
        )
    }
}

/// What the router hands an engine: an already-rendered prompt. The server
/// proxies it as is, so prompt logic lives in one place (kabar-core).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineCall {
    /// Also tells the proxy where to read/write the shared D1 cache.
    pub cache_key: CacheKey,
    pub system: String,
    pub user: String,
    /// Content-addressed blob (SHA-256 hex) for vision tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_sha256: Option<String>,
    /// True on the one retry after the first reply failed validation.
    #[serde(default)]
    pub is_repair: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineReply {
    pub engine: EngineId,
    /// The model's raw text; the router parses and validates it.
    pub text: String,
    /// Provider model id, e.g. "@cf/aisingapore/gemma-sea-lion-v4-27b-it".
    #[serde(default)]
    pub model: String,
}

/// Why an engine call failed, as the proxy reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EngineError {
    /// A free quota is used up; the hard stop refused the call.
    QuotaExhausted {
        retry_after_s: Option<u64>,
    },
    RateLimited {
        retry_after_s: Option<u64>,
    },
    /// Engine is switched off or unreachable (Kabar Home asleep, offline).
    Unavailable {
        reason: String,
    },
    /// The router refused before calling: personal data to a training engine.
    PrivacyBlocked,
    Other {
        message: String,
    },
}

/// Shared cache key (spec §19): (task, input hash, engine, prompt version).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    pub task: Task,
    /// SHA-256 hex of the canonical JSON input.
    pub input_hash: String,
    pub engine: EngineId,
    pub prompt_version: u32,
}

impl CacheKey {
    /// Stable string form, used as the D1 primary key.
    pub fn id(&self) -> String {
        format!(
            "{}:{}:v{}:{}",
            self.task.as_str(),
            self.engine.as_str(),
            self.prompt_version,
            self.input_hash
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_through_serde() {
        for t in Task::ALL {
            let json = serde_json::to_string(&t).unwrap();
            assert_eq!(json, format!("\"{}\"", t.as_str()));
            assert_eq!(Task::parse(t.as_str()), Some(t));
        }
        for e in EngineId::ALL {
            assert_eq!(serde_json::to_string(&e).unwrap(), format!("\"{}\"", e.as_str()));
        }
    }

    #[test]
    fn engine_error_shape() {
        let e = EngineError::QuotaExhausted { retry_after_s: Some(60) };
        assert_eq!(serde_json::to_string(&e).unwrap(), r#"{"kind":"quota_exhausted","retry_after_s":60}"#);
    }
}
