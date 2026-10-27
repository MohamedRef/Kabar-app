//! Level and placement tests (`kabar.test/v1`, SPEC §6, §10): written at
//! build time under `content/tests/<language>/<id>.yaml`. Level tests
//! (`bipa1.test`) double as "Take a flight"; `placement` picks where the
//! journey starts. Items are lesson exercises plus open writing/speaking
//! tasks graded against a rubric.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::lesson::{ContentReview, Exercise, ExerciseKind, Localized};

/// The `schema:` tag every test file carries.
pub const TEST_SCHEMA: &str = "kabar.test/v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestContent {
    /// Always [`TEST_SCHEMA`].
    pub schema: String,
    /// `<level>.test` for level tests, `placement` for the placement test.
    pub id: String,
    /// `id` (Indonesian) or `jv` (Javanese).
    pub language: String,
    pub kind: TestKind,
    pub title: Localized,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<Localized>,
    /// Rough length, shown before starting (never a countdown).
    pub minutes: u32,
    /// Level tests: the share of points needed to pass (0–1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass_mark: Option<f32>,
    /// How the test is scored, for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring: Option<String>,
    pub sections: Vec<TestSection>,
    #[serde(default)]
    pub review: ContentReview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    Level,
    Placement,
}

/// What a section or item measures (the skill radar, plus grammar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestSkill {
    Listening,
    Reading,
    Writing,
    Speaking,
    Vocabulary,
    Grammar,
}

impl TestSkill {
    pub const ALL: [Self; 6] =
        [Self::Listening, Self::Reading, Self::Writing, Self::Speaking, Self::Vocabulary, Self::Grammar];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestSection {
    pub id: String,
    pub skill: TestSkill,
    /// Placement: the level this section probes (`bipa1`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    pub title: Localized,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Localized>,
    pub items: Vec<TestItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestItem {
    /// Unique in the test, e.g. `t01`.
    pub id: String,
    /// Points the item is worth (default 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<f32>,
    /// Overrides the section's skill for this item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<TestSkill>,
    /// Overrides the section's level (placement).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// The curriculum unit this item tests, e.g. `bipa1.u04`, so results
    /// can send the learner back to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Grammar-library ids the item checks, e.g. `id.question.apa`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grammar: Vec<String>,
    #[serde(flatten)]
    pub kind: TestItemKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain: Option<Localized>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<String>,
}

impl TestItem {
    pub fn points(&self) -> f32 {
        self.points.unwrap_or(1.0)
    }

    /// The item as a lesson exercise (closed items), so the Lesson Player's
    /// grading and views work on it unchanged.
    pub fn exercise(&self) -> Option<Exercise> {
        match &self.kind {
            TestItemKind::Exercise(kind) => Some(Exercise {
                id: self.id.clone(),
                kind: kind.clone(),
                explain: self.explain.clone(),
                words: self.words.clone(),
            }),
            _ => None,
        }
    }

    pub fn open_task(&self) -> Option<&OpenTask> {
        match &self.kind {
            TestItemKind::WritingTask(t) | TestItemKind::SpeakingTask(t) => Some(t),
            TestItemKind::Exercise(_) => None,
        }
    }
}

/// A lesson exercise (same `type` tag and fields) or an open task.
#[derive(Clone, Debug, PartialEq)]
pub enum TestItemKind {
    Exercise(ExerciseKind),
    /// `type: writing_task`: write a short text.
    WritingTask(OpenTask),
    /// `type: speaking_task`: talk about something.
    SpeakingTask(OpenTask),
}

/// An open task: AI-graded against the rubric later (`grade_answer`); until
/// then the learner compares with the rubric and sample and self-assesses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenTask {
    pub prompt: Localized,
    /// Criteria, each an equal share of the item's points.
    pub rubric: Vec<String>,
    /// A model answer in the target language.
    pub sample: String,
}

const WRITING_TASK: &str = "writing_task";
const SPEAKING_TASK: &str = "speaking_task";

impl Serialize for TestItemKind {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        let (tag, task) = match self {
            Self::Exercise(kind) => return kind.serialize(s),
            Self::WritingTask(t) => (WRITING_TASK, t),
            Self::SpeakingTask(t) => (SPEAKING_TASK, t),
        };
        let mut v = serde_json::to_value(task).map_err(S::Error::custom)?;
        if let Some(map) = v.as_object_mut() {
            map.insert("type".into(), tag.into());
        }
        v.serialize(s)
    }
}

impl<'de> Deserialize<'de> for TestItemKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut v = serde_json::Value::deserialize(d)?;
        let tag = v.get("type").and_then(|t| t.as_str()).unwrap_or_default().to_owned();
        let open = |mut v: serde_json::Value| {
            if let Some(map) = v.as_object_mut() {
                map.remove("type");
            }
            serde_json::from_value::<OpenTask>(v).map_err(|e| D::Error::custom(format!("{tag}: {e}")))
        };
        match tag.as_str() {
            WRITING_TASK => open(v.take()).map(Self::WritingTask),
            SPEAKING_TASK => open(v.take()).map(Self::SpeakingTask),
            _ => serde_json::from_value(v).map(Self::Exercise).map_err(D::Error::custom),
        }
    }
}

/// Payload of a `placed` row in `lesson_events` (lesson id `placement`):
/// where the placement test starts the journey.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacementResult {
    /// The level to start at, e.g. `bipa2`.
    pub start_level: String,
    /// The unit inside `start_level` to start at, when the tagged items
    /// show where the gaps begin (`bipa2.u03`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_unit: Option<String>,
    /// Share of points per section taken, in order (0–1).
    #[serde(default)]
    pub sections: Vec<SectionScore>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionScore {
    pub id: String,
    pub score: f32,
}

/// Payload of a `test_finished` row in `lesson_events` (every finished level
/// test, passed or not); a passed one also gets a `completed` row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestFinished {
    /// Share of points earned, 0–1.
    pub score: f32,
    pub passed: bool,
    pub duration_ms: u64,
    /// Share of points per skill (0–1).
    #[serde(default)]
    pub skills: Vec<(TestSkill, f32)>,
    /// Ids of items missed (fully or partly).
    #[serde(default)]
    pub missed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_parse_as_exercises_or_open_tasks_and_round_trip() {
        let json = r#"[
            {"id":"t01","type":"listen","text":"Selamat pagi!","options":["Selamat pagi!","Selamat malam!"],"answer":0},
            {"id":"t02","points":4,"skill":"writing","type":"writing_task",
             "prompt":{"ar_eg":"اكتب","ar":"اكتب","en":"Write"},"rubric":["a","b"],"sample":"Hai!"}
        ]"#;
        let items: Vec<TestItem> = serde_json::from_str(json).unwrap();
        assert!(matches!(items[0].kind, TestItemKind::Exercise(ExerciseKind::Listen { .. })));
        assert_eq!(items[0].points(), 1.0);
        assert!(items[0].exercise().is_some());
        assert!(matches!(&items[1].kind, TestItemKind::WritingTask(t) if t.sample == "Hai!"));
        assert_eq!(items[1].points(), 4.0);
        assert_eq!(items[1].skill, Some(TestSkill::Writing));
        assert!(items[1].exercise().is_none());
        let back: Vec<TestItem> = serde_json::from_str(&serde_json::to_string(&items).unwrap()).unwrap();
        assert_eq!(back, items);
    }

    #[test]
    fn a_broken_open_task_names_its_type() {
        let json =
            r#"{"id":"t1","type":"speaking_task","prompt":{"ar_eg":"x","ar":"x","en":"x"},"sample":"s"}"#;
        let err = serde_json::from_str::<TestItem>(json).unwrap_err().to_string();
        assert!(err.contains("speaking_task") && err.contains("rubric"), "{err}");
    }

    #[test]
    fn items_carry_their_unit_and_grammar() {
        let json = r#"{"id":"t01","unit":"bipa1.u04","grammar":["id.question.apa"],"type":"speaking","text":"Halo","meaning":{"ar_eg":"a","ar":"b","en":"c"}}"#;
        let item: TestItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.unit.as_deref(), Some("bipa1.u04"));
        assert_eq!(item.grammar, ["id.question.apa"]);
        // Untagged items still parse, and write back without the new keys.
        let json =
            r#"{"id":"t02","type":"speaking","text":"Halo","meaning":{"ar_eg":"a","ar":"b","en":"c"}}"#;
        let item: TestItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.unit, None);
        assert!(!serde_json::to_string(&item).unwrap().contains("unit"));
    }
}
