//! Review events: append-only, synced, replayed in HLC order on every device.

use serde::{Deserialize, Serialize};

use crate::Hlc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

impl Rating {
    pub const ALL: [Self; 4] = [Self::Again, Self::Hard, Self::Good, Self::Easy];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CardKind {
    /// See the word, recall the meaning.
    Recognition,
    /// See the meaning, produce the word (typed or handwritten).
    Production,
    /// Hear it, recall the meaning.
    AudioMeaning,
    /// Fill the gap in the sentence where the word was saved.
    Cloze,
    /// Arabic Bridge: Arabic origin ↔ Indonesian word.
    Bridge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewMode {
    Classic,
    /// Tablet Card Wall: the grade comes from the pile and the sorting speed.
    CardWall,
    /// One-minute bursts on the phone.
    Burst,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReviewEvent {
    /// Unique per event; replays ignore duplicates.
    pub id: String,
    pub card_id: String,
    pub hlc: Hlc,
    /// Wall-clock time of the answer (ms since epoch).
    pub at_ms: u64,
    pub rating: Rating,
    /// Time spent answering.
    pub duration_ms: u32,
    pub mode: ReviewMode,
}
