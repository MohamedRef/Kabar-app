//! Kabar Pagi (spec §14): a 2-minute morning micro-story built from the
//! learner's due words plus a few new ones, with audio.
//!
//! Primary: the `story_episode` AI task through the router (learning text
//! only, never personal data). Offline or when no free engine answers: the
//! best-matching story from the build-time bank in `content/pagi/`.

use kabar_protocol::ai::Task;
use kabar_protocol::cloud::{TtsSegment, Voice};
use serde::{Deserialize, Serialize};

use crate::ai::AiOutput;

/// Most due words a story is built around, and most new words added.
pub const MAX_DUE: usize = 6;
pub const MAX_NEW: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagiWords {
    pub due: Vec<String>,
    pub new: Vec<String>,
}

/// Picks today's words: the due words the learner is shakiest on first
/// (callers pass them in that order), then new words from `new_pool` (ranked,
/// most useful first) that aren't in `known`.
pub fn pick_words(due: &[String], known: &[String], new_pool: &[String], seed: u64) -> PagiWords {
    let _ = (due, known, new_pool, seed);
    todo!("m10 agent C")
}

/// A story, as the AI returns it and as the bank stores it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Story {
    /// Indonesian title.
    pub title: String,
    /// e.g. "bipa1".
    pub level: String,
    pub sentences: Vec<StorySentence>,
    /// The target words the story uses, as dictionary forms.
    pub words: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorySentence {
    /// Indonesian.
    pub id: String,
    /// Egyptian Arabic.
    pub arz: String,
    /// Modern Standard Arabic.
    pub ar: String,
    pub en: String,
}

impl AiOutput for Story {
    const TASK: Task = Task::StoryEpisode;

    fn validate(&self) -> Result<(), String> {
        todo!("m10 agent C")
    }
}

/// The router input for `story_episode`.
pub fn story_request(words: &PagiWords, level: &str) -> serde_json::Value {
    let _ = (words, level);
    todo!("m10 agent C")
}

/// Whether a story uses every word asked for (affixed forms count:
/// `dibaca` uses `baca`).
pub fn uses_words(story: &Story, words: &[String]) -> Vec<String> {
    let _ = (story, words);
    todo!("m10 agent C")
}

/// The build-time story bank (`content/pagi/stories.jsonl`).
#[derive(Clone, Debug, Default)]
pub struct StoryBank {
    pub stories: Vec<Story>,
}

impl StoryBank {
    pub fn parse(jsonl: &str) -> Result<Self, String> {
        let _ = jsonl;
        todo!("m10 agent C")
    }

    /// The story that covers the most of today's words at or below `level`;
    /// ties broken by `seed` so mornings vary.
    pub fn best(&self, words: &PagiWords, level: &str, seed: u64) -> Option<&Story> {
        let _ = (words, level, seed);
        todo!("m10 agent C")
    }
}

/// One TTS segment per sentence (Azure via Kabar Cloud, or system voices).
pub fn tts_segments(story: &Story, voice: Voice) -> Vec<TtsSegment> {
    let _ = (story, voice);
    todo!("m10 agent C")
}

/// Rough listening time at a learner-friendly pace.
pub fn listening_seconds(story: &Story) -> u32 {
    let _ = story;
    todo!("m10 agent C")
}
