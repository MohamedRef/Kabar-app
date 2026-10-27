//! The Journey Map model (SPEC §10): curriculum levels are places, units are
//! stops around them, lessons are the steps at each stop.
//!
//! Pure logic: parse the curriculum YAML (only the fields the map needs; the
//! rest is ignored), compute progress from the set of completed lesson ids,
//! collect souvenirs, and lay the route out on the map ([`geo`], [`layout`]).
//! Nothing is ever locked: skipping ahead is always allowed, so "current" is
//! only a suggestion.

pub mod geo;
pub mod layout;

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A short label: target-language form (optional), English and MSA.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    #[serde(default)]
    pub target: Option<String>,
    pub en: String,
    pub ar: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curriculum {
    /// `id` (Indonesian) or `jv` (Javanese).
    pub language: String,
    pub title: Text,
    pub levels: Vec<Level>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub id: String,
    /// "BIPA 1", "J1".
    pub code: String,
    pub title: Text,
    /// Level ids that are best done first; may name another curriculum
    /// (J1 requires `bipa1`). Advice only, never a lock.
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub journey: Option<JourneyPlace>,
    #[serde(default)]
    pub units: Vec<Unit>,
    #[serde(default)]
    pub level_test: Option<LevelTest>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JourneyPlace {
    pub place: String,
    #[serde(default)]
    pub kind: PlaceKind,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceKind {
    Island,
    TrainLine,
    BranchLine,
    City,
    /// Anything newer than this build knows; drawn in the route's own style.
    #[default]
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelTest {
    pub id: String,
    #[serde(default)]
    pub minutes: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    pub id: String,
    pub title: Text,
    /// The town or station this unit stops at.
    pub stop: String,
    #[serde(default)]
    pub culture: Option<Culture>,
    #[serde(default)]
    pub lessons: Vec<Lesson>,
}

/// The souvenir card a finished unit earns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Culture {
    pub souvenir: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lesson {
    pub id: String,
    #[serde(default)]
    pub kind: LessonKind,
    pub title: String,
    #[serde(default)]
    pub can_do: String,
    #[serde(default)]
    pub minutes: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LessonKind {
    #[default]
    Learn,
    Practice,
    Review,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "curriculum: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// Parses `curriculum/*.yaml`, keeping only what the map needs.
pub fn parse_curriculum(yaml: &str) -> Result<Curriculum, ParseError> {
    serde_yaml_ng::from_str(yaml).map_err(|e| ParseError(e.to_string()))
}

impl Curriculum {
    /// Javanese content stays flagged unverified until the user approves it
    /// (see `Progress::unit_approved` and `Progress::level_approved`).
    pub fn unverified(&self) -> bool {
        self.language == "jv"
    }

    pub fn level(&self, id: &str) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }

    /// Every lesson in route order.
    pub fn lessons(&self) -> impl Iterator<Item = &Lesson> {
        self.levels.iter().flat_map(Level::lessons)
    }

    /// The level's `requires` that point outside this curriculum (J1 → `bipa1`).
    pub fn external_requires<'a>(&self, level: &'a Level) -> impl Iterator<Item = &'a str> {
        let own: BTreeSet<&str> = self.levels.iter().map(|l| l.id.as_str()).collect();
        level.requires.iter().map(String::as_str).filter(move |r| !own.contains(r))
    }
}

impl Level {
    pub fn lessons(&self) -> impl Iterator<Item = &Lesson> {
        self.units.iter().flat_map(|u| u.lessons.iter())
    }

    pub fn kind(&self) -> PlaceKind {
        self.journey.as_ref().map_or(PlaceKind::Other, |j| j.kind)
    }

    pub fn place(&self) -> &str {
        self.journey.as_ref().map_or(self.code.as_str(), |j| j.place.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LessonState {
    Done,
    /// The first lesson not yet done, in route order: what "Lanjut" opens.
    Current,
    /// Not done yet. Still open: nothing on the map is locked.
    Open,
}

/// Progress on one curriculum, from the completed lesson (and level test) ids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    done: BTreeSet<String>,
    done_levels: BTreeSet<String>,
    current: Option<String>,
    /// Lessons the learner approved by passing them (`lesson::approves`).
    approved: BTreeSet<String>,
}

impl Progress {
    /// A passed level test ("Take a flight") counts every lesson of its level
    /// as done.
    pub fn new(curriculum: &Curriculum, completed: &BTreeSet<String>) -> Self {
        let mut done = BTreeSet::new();
        let mut done_levels = BTreeSet::new();
        for level in &curriculum.levels {
            let flown = level.level_test.as_ref().is_some_and(|t| completed.contains(&t.id));
            let mut all = true;
            for lesson in level.lessons() {
                if flown || completed.contains(&lesson.id) {
                    done.insert(lesson.id.clone());
                } else {
                    all = false;
                }
            }
            if flown || (all && level.lessons().next().is_some()) {
                done_levels.insert(level.id.clone());
            }
        }
        let current = curriculum.lessons().find(|l| !done.contains(&l.id)).map(|l| l.id.clone());
        Self { done, done_levels, current, approved: BTreeSet::new() }
    }

    /// Adds the lessons the learner approved by passing them.
    pub fn with_approved(mut self, approved: &BTreeSet<String>) -> Self {
        self.approved = approved.clone();
        self
    }

    /// Every lesson of the unit was passed with a reasonable score.
    pub fn unit_approved(&self, unit: &Unit) -> bool {
        !unit.lessons.is_empty() && unit.lessons.iter().all(|l| self.approved.contains(&l.id))
    }

    /// Every lesson of the level was passed with a reasonable score.
    pub fn level_approved(&self, level: &Level) -> bool {
        level.lessons().next().is_some() && level.lessons().all(|l| self.approved.contains(&l.id))
    }

    /// Applies a placement result: when the current lesson sits before
    /// `start` on the route, Lanjut moves to the first unfinished lesson from
    /// there on. `start` is a level (`bipa2`) or, when the test could tell, a
    /// unit inside it (`bipa2.u03`). Nothing is marked done, and earlier
    /// lessons stay open. Unknown ids (e.g. the other route's) change nothing.
    pub fn start_at(mut self, curriculum: &Curriculum, start: &str) -> Self {
        let lessons: Vec<&Lesson> = curriculum.lessons().collect();
        let first =
            curriculum.levels.iter().find(|l| l.id == start).and_then(|l| l.lessons().next()).or_else(|| {
                curriculum
                    .levels
                    .iter()
                    .flat_map(|l| &l.units)
                    .find(|u| u.id == start)
                    .and_then(|u| u.lessons.first())
            });
        let Some(from) = first.and_then(|f| lessons.iter().position(|l| l.id == f.id)) else { return self };
        let at = |id: &str| lessons.iter().position(|l| l.id == id);
        if self.current.as_deref().and_then(at).is_some_and(|i| i < from) {
            self.current = lessons[from..].iter().find(|l| !self.done.contains(&l.id)).map(|l| l.id.clone());
        }
        self
    }

    pub fn lesson_state(&self, lesson_id: &str) -> LessonState {
        if self.done.contains(lesson_id) {
            LessonState::Done
        } else if self.current.as_deref() == Some(lesson_id) {
            LessonState::Current
        } else {
            LessonState::Open
        }
    }

    /// The lesson "Lanjut" continues with; `None` once the route is finished.
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    pub fn level_done(&self, level_id: &str) -> bool {
        self.done_levels.contains(level_id)
    }

    pub fn done_levels(&self) -> &BTreeSet<String> {
        &self.done_levels
    }

    pub fn unit_done(&self, unit: &Unit) -> bool {
        !unit.lessons.is_empty() && unit.lessons.iter().all(|l| self.done.contains(&l.id))
    }

    /// (done, total) lessons in a level.
    pub fn level_counts(&self, level: &Level) -> (usize, usize) {
        level.lessons().fold((0, 0), |(d, t), l| (d + usize::from(self.done.contains(&l.id)), t + 1))
    }

    /// Whether the current lesson sits in this level/unit.
    pub fn holds_current(&self, unit: &Unit) -> bool {
        self.current.as_deref().is_some_and(|c| unit.lessons.iter().any(|l| l.id == c))
    }
}

/// A souvenir card in the Suitcase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Souvenir {
    pub unit_id: String,
    pub level_code: String,
    pub stop: String,
    pub name: String,
    pub note: String,
    pub unverified: bool,
}

/// Souvenirs of finished units, in route order.
pub fn souvenirs(curriculum: &Curriculum, progress: &Progress) -> Vec<Souvenir> {
    curriculum
        .levels
        .iter()
        .flat_map(|level| level.units.iter().map(move |u| (level, u)))
        .filter(|(_, u)| progress.unit_done(u))
        .filter_map(|(level, u)| {
            let c = u.culture.as_ref()?;
            Some(Souvenir {
                unit_id: u.id.clone(),
                level_code: level.code.clone(),
                stop: u.stop.clone(),
                name: c.souvenir.clone(),
                note: c.note.clone(),
                unverified: curriculum.unverified() && !progress.unit_approved(u),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
