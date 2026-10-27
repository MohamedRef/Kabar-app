//! Spoken build-time content (SPEC §15, §14): the four conversation personas,
//! Sinetron episodes and Kabar Radio scripts. Written under
//! `content/personas/`, `content/stories/sinetron/` and `content/radio/<lang>/`;
//! `kabar-content` checks them (formats in `content/README.md`), the app plays
//! them.

use serde::{Deserialize, Serialize};

use crate::cloud::Voice;
use crate::lesson::{ContentReview, Exercise, Localized, NewWord};

pub const PERSONA_SCHEMA: &str = "kabar.persona/v1";
pub const STORY_SCHEMA: &str = "kabar.story/v1";
pub const RADIO_SCHEMA: &str = "kabar.radio/v1";

/// The speaker name a Sinetron line uses for narration.
pub const NARRATOR: &str = "Narrator";
/// The speaker name a persona dialogue uses for the learner.
pub const LEARNER: &str = "learner";

/// A title in the learning language plus its three meanings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Title {
    pub target: String,
    #[serde(flatten)]
    pub meaning: Localized,
}

/// One spoken line, the same shape everywhere (persona dialogues, Sinetron,
/// Radio).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub speaker: String,
    pub text: String,
    #[serde(flatten)]
    pub meaning: Localized,
    /// A culture or grammar aside, shown as a tappable chip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Localized>,
    /// Persona dialogues only: the learner's mistake this line recasts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction: Option<Correction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Correction {
    pub wrong: String,
    pub right: String,
    pub why: Localized,
}

// --- Sinetron ----------------------------------------------------------------

/// A Sinetron episode (`kabar.story/v1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub schema: String,
    /// `sinetron.eNN`.
    pub id: String,
    pub series: String,
    pub episode: u32,
    /// `id` or `jv`.
    pub language: String,
    pub level: String,
    /// The curriculum unit whose completion unlocks this episode.
    pub unlock_after: String,
    pub title: Title,
    pub summary: Localized,
    /// Everyone who speaks, besides [`NARRATOR`].
    pub cast: Vec<String>,
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub new_words: Vec<NewWord>,
    /// Lesson exercises, asked after the episode.
    pub questions: Vec<Exercise>,
    /// "Next time on…": a teaser for the next episode.
    #[serde(default)]
    pub next: Option<Localized>,
    pub review: ContentReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scene {
    pub setting: Localized,
    pub lines: Vec<Line>,
}

// --- Kabar Radio ---------------------------------------------------------------

/// A prebuilt two-host Kabar Radio episode for one unit (`kabar.radio/v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadioEpisode {
    pub schema: String,
    /// `radio.<unit id>`.
    pub id: String,
    pub language: String,
    pub level: String,
    pub unit: String,
    pub title: Title,
    pub hosts: Vec<Host>,
    pub minutes: u32,
    pub segments: Vec<Segment>,
    /// The unit words the episode spotlights.
    pub words: Vec<String>,
    pub review: ContentReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub name: String,
    /// Azure voice name, e.g. `id-ID-GadisNeural`.
    pub voice: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub kind: SegmentKind,
    #[serde(default)]
    pub title: Option<String>,
    pub lines: Vec<Line>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    Intro,
    Dialogue,
    WordSpotlight,
    Recap,
    Outro,
}

// --- personas ------------------------------------------------------------------

/// A conversation persona (`kabar.persona/v1`): who they are, and the style
/// guide `roleplay_turn` is prompted with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Persona {
    pub schema: String,
    /// Canonical, hyphenated: `mas-dimas`.
    pub id: String,
    pub name: String,
    pub language: String,
    /// Azure voice name.
    pub voice: String,
    pub home: String,
    /// The curriculum units whose scenes this persona practises.
    #[serde(default)]
    pub units: Vec<String>,
    pub role: Localized,
    pub personality: Localized,
    pub style: PersonaStyle,
    #[serde(default)]
    pub topics: Vec<String>,
    pub openers: Vec<Opener>,
    pub example_dialogues: Vec<ExampleDialogue>,
    pub review: ContentReview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaStyle {
    pub register: Localized,
    #[serde(rename = "do")]
    pub dos: Vec<String>,
    pub dont: Vec<String>,
    pub corrections: Localized,
    pub levels: Vec<LevelBand>,
}

/// How the persona talks to a learner at one curriculum level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelBand {
    pub level: String,
    pub guide: String,
    #[serde(default)]
    pub vocabulary: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opener {
    pub text: String,
    #[serde(flatten)]
    pub meaning: Localized,
    #[serde(default)]
    pub level: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExampleDialogue {
    pub id: String,
    pub level: String,
    pub situation: Localized,
    pub lines: Vec<Line>,
}

/// Persona ids are hyphenated (`mas-dimas`); golden sets and model output
/// sometimes write `mas_dimas`. Both name the same persona.
pub fn canonical_persona_id(id: &str) -> String {
    id.trim().to_lowercase().replace('_', "-")
}

/// The [`Voice`] for an Azure voice name from a content file.
pub fn voice_named(azure_name: &str) -> Option<Voice> {
    [Voice::IdGadis, Voice::IdArdi, Voice::JvSiti, Voice::JvDimas]
        .into_iter()
        .find(|v| v.azure_name() == azure_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_flatten_their_meanings() {
        let json = r#"{"speaker":"Sari","text":"Selamat pagi!","ar_eg":"صباح الخير!","ar":"صباح الخير!","en":"Good morning!",
            "note":{"ar_eg":"x","ar":"y","en":"z"}}"#;
        let line: Line = serde_json::from_str(json).unwrap();
        assert_eq!(line.meaning.en, "Good morning!");
        assert_eq!(line.note.unwrap().en, "z");
        assert!(line.correction.is_none());
    }

    #[test]
    fn voices_and_persona_ids() {
        assert_eq!(voice_named("jv-ID-DimasNeural"), Some(Voice::JvDimas));
        assert_eq!(voice_named("en-US-JennyNeural"), None);
        assert_eq!(canonical_persona_id("Mas_Dimas"), "mas-dimas");
        assert_eq!(canonical_persona_id("bu-siti"), "bu-siti");
    }
}
