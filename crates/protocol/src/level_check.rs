//! The monthly level check (SPEC §6): a 10-minute mini check that updates the
//! skill radar and the "at this pace you reach BIPA 3 around <date>" forecast.
//!
//! Stored as an append-only `lesson_events` row (synced like every lesson
//! event): lesson id [`LEVEL_CHECK_LESSON_ID`], kind [`LEVEL_CHECK_KIND`],
//! payload [`LevelCheckResult`]. Older clients ignore rows they don't know;
//! newer fields are added with serde defaults, never by changing old ones.

use serde::{Deserialize, Serialize};

use crate::coach::{LocalDate, Skill};

/// Bumped when [`LevelCheckResult`] changes incompatibly.
pub const LEVEL_CHECK_VERSION: u32 = 1;
/// `lesson_events.lesson_id` of every level check row.
pub const LEVEL_CHECK_LESSON_ID: &str = "level_check";
/// `lesson_events.kind` of a finished level check.
pub const LEVEL_CHECK_KIND: &str = "level_check";

fn version() -> u32 {
    LEVEL_CHECK_VERSION
}

/// What the check decided about the level (Kak Nur's rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LevelMove {
    Up,
    Down,
    Stay,
}

/// One radar axis, 0.0–1.0.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadarPoint {
    pub skill: Skill,
    pub value: f32,
}

/// "At this pace you reach {level} around {date}."
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelEta {
    pub level: String,
    pub date: LocalDate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelCheckResult {
    #[serde(default = "version")]
    pub v: u32,
    /// The learner's local day of the check.
    pub date: LocalDate,
    /// `id` (Indonesian) or `jv` (Javanese).
    pub language: String,
    /// The level the check started from (`bipa2`).
    pub from_level: String,
    /// The level after the decision.
    pub level: String,
    pub moved: LevelMove,
    /// Share of the mini check's points earned; `None` when the level has no
    /// test items yet (the check then looks at history only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    /// Measured axes only; an axis with no evidence is left out.
    #[serde(default)]
    pub radar: Vec<RadarPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta: Option<LevelEta>,
    /// Ids of mini-check items missed (fully or partly).
    #[serde(default)]
    pub missed: Vec<String>,
    #[serde(default)]
    pub duration_ms: u64,
}

impl LevelCheckResult {
    pub fn radar_value(&self, skill: Skill) -> Option<f32> {
        self.radar.iter().find(|p| p.skill == skill).map(|p| p.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_reads_old_rows() {
        let r = LevelCheckResult {
            v: LEVEL_CHECK_VERSION,
            date: LocalDate::from_ymd(2026, 10, 26).unwrap(),
            language: "id".into(),
            from_level: "bipa2".into(),
            level: "bipa2".into(),
            moved: LevelMove::Stay,
            score: Some(0.7),
            radar: vec![RadarPoint { skill: Skill::Reading, value: 0.8 }],
            eta: Some(LevelEta { level: "bipa3".into(), date: LocalDate::from_ymd(2027, 5, 1).unwrap() }),
            missed: vec!["v03".into()],
            duration_ms: 540_000,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains(r#""moved":"stay""#));
        assert_eq!(serde_json::from_str::<LevelCheckResult>(&json).unwrap(), r);
        // A minimal row (an older or newer client) still reads.
        let min = r#"{"date":"2026-10-26","language":"id","from_level":"bipa1","level":"bipa2","moved":"up","extra":1}"#;
        let m: LevelCheckResult = serde_json::from_str(min).unwrap();
        assert_eq!((m.v, m.moved, m.radar_value(Skill::Reading)), (1, LevelMove::Up, None));
    }
}
