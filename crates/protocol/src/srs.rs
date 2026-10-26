//! Review events: append-only, synced, replayed in HLC order on every device.

use serde::{Deserialize, Serialize};

use crate::Hlc;
use crate::lesson::{Example, Localized};

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

/// Bumped when [`Card`] changes shape; older payloads still read through
/// serde defaults, and unknown fields from newer clients are ignored.
pub const CARD_VERSION: u32 = 1;

fn card_version() -> u32 {
    CARD_VERSION
}

/// The language a card teaches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StudyLanguage {
    /// Indonesian (`id`).
    #[default]
    Id,
    /// Javanese (`jv`).
    Jv,
}

impl StudyLanguage {
    /// BCP 47 tag, for `lang` attributes.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Jv => "jv",
        }
    }

    /// Locale for speech voices.
    pub fn speech_tag(self) -> &'static str {
        match self {
            Self::Id => "id-ID",
            Self::Jv => "jv-ID",
        }
    }
}

/// Where a card came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CardSource {
    /// The built-in starter deck.
    #[default]
    Seed,
    Lesson,
    Reader,
    Capture,
    Manual,
    /// A source added by a newer client.
    #[serde(other)]
    Other,
}

/// How far an Arabic-origin word's meaning moved (SPEC §12).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeaningShift {
    /// Means what the Arabic means.
    #[default]
    Same,
    /// Related, but the meaning or use moved.
    Shifted,
    /// Looks familiar, means something else: the False Friends Alarm.
    FalseFriend,
}

/// The Arabic word a target-language word came from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArabicOrigin {
    /// The Arabic word, e.g. «خبر».
    pub word: String,
    /// Root letters, e.g. «خ ب ر».
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub shift: MeaningShift,
    /// The sound change or meaning shift, explained.
    #[serde(default)]
    pub note: Option<Localized>,
}

/// One review card (the `cards` table payload). Synced last-writer-wins by
/// HLC. Its FSRS state is never stored: it is derived by replaying the log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    #[serde(default = "card_version")]
    pub v: u32,
    /// Stable across devices, so seeding on two devices never duplicates.
    pub id: String,
    /// The target-language word or phrase, e.g. "kabar".
    pub word: String,
    #[serde(default)]
    pub language: StudyLanguage,
    pub kind: CardKind,
    pub meaning: Localized,
    /// Usage note for the back of the card (register, common phrases).
    #[serde(default)]
    pub note: Option<Localized>,
    /// The sentence the word was saved from; cloze cards blank the word in it.
    #[serde(default)]
    pub example: Option<Example>,
    /// Arabic Bridge data, for words of Arabic origin.
    #[serde(default)]
    pub arabic: Option<ArabicOrigin>,
    #[serde(default)]
    pub source: CardSource,
    /// Lesson or text id when `source` points at one.
    #[serde(default)]
    pub source_ref: Option<String>,
    /// Approved by the learner. Javanese content stays flagged until then.
    #[serde(default)]
    pub verified: bool,
}

impl Card {
    /// Cards for the same word (recognition, bridge, …) are siblings; a
    /// session introduces only one sibling per day.
    pub fn sibling_key(&self) -> (StudyLanguage, &str) {
        (self.language, self.word.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_card_payload_reads_with_defaults() {
        let json = r#"{"id":"c1","word":"kabar","kind":"recognition",
            "meaning":{"ar_eg":"أخبار","ar":"خبر","en":"news"},"future_field":1}"#;
        let card: Card = serde_json::from_str(json).unwrap();
        assert_eq!(card.v, CARD_VERSION);
        assert_eq!(card.language, StudyLanguage::Id);
        assert_eq!(card.source, CardSource::Seed);
        assert!(card.arabic.is_none() && !card.verified);
    }

    #[test]
    fn unknown_source_reads_as_other() {
        let json = r#"{"id":"c1","word":"x","kind":"bridge","source":"radio",
            "meaning":{"ar_eg":"","ar":"","en":""}}"#;
        assert_eq!(serde_json::from_str::<Card>(json).unwrap().source, CardSource::Other);
    }

    #[test]
    fn review_event_round_trips() {
        let ev = ReviewEvent {
            id: "e1".into(),
            card_id: "c1".into(),
            hlc: Hlc { wall_ms: 5, counter: 1, device: crate::DeviceId("d".into()) },
            at_ms: 5,
            rating: Rating::Good,
            duration_ms: 1200,
            mode: ReviewMode::CardWall,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains(r#""mode":"card-wall""#));
        assert_eq!(serde_json::from_str::<ReviewEvent>(&json).unwrap(), ev);
    }
}
