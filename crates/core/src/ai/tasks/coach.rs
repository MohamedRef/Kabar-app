//! Kak Nur's tasks: placement, the monthly level check, daily plan, recap,
//! weekly report and coach messages. All personal: never routed to engines
//! that train on inputs.
//!
//! These are the AI-worded forms; the numbers they talk about come from
//! `crate::coach` and `kabar_protocol::coach`, whose `DailyPlan` and
//! `WeeklyReport` are the computed records, not these.

use kabar_protocol::ai::Task;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{AiOutput, arabic, filled, input_str};
use crate::coach::onboarding::LEVELS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

fn check_level(level: &str) -> Result<(), String> {
    if LEVELS.contains(&level) { Ok(()) } else { Err(format!("level `{level}` is not one of {LEVELS:?}")) }
}

/// The levels of one language, in order.
fn ladder(lang: &str) -> &'static [&'static str] {
    if lang == "jv" { &LEVELS[7..] } else { &LEVELS[..7] }
}

/// `bipa2-03` → `bipa2`, `j3.u01` → `j3`.
fn level_of(item: &str) -> Option<&'static str> {
    LEVELS.iter().copied().filter(|l| item.starts_with(l)).max_by_key(|l| l.len())
}

fn pass_rate(correct: usize, total: usize) -> f64 {
    correct as f64 / total as f64
}

/// Where the journey starts, from the placement answers (the app's
/// `correct` flags are trusted): the lowest level under 75% correct, else
/// the level after the highest one tested.
pub fn placement_for(input: &Value) -> Option<(&'static str, Confidence)> {
    let ladder = ladder(input_str(input, "lang")?);
    let answers: Vec<(usize, bool)> = input
        .get("answers")?
        .as_array()?
        .iter()
        .filter_map(|a| {
            let level = level_of(a.get("item")?.as_str()?)?;
            Some((ladder.iter().position(|l| *l == level)?, a.get("correct")?.as_bool()?))
        })
        .collect();
    let score = |i: usize| {
        let at: Vec<bool> = answers.iter().filter(|(l, _)| *l == i).map(|(_, c)| *c).collect();
        (at.iter().filter(|c| **c).count(), at.len())
    };
    let failed = (0..ladder.len()).find(|&i| {
        let (c, n) = score(i);
        n > 0 && pass_rate(c, n) < 0.75
    });
    let placed =
        failed.unwrap_or_else(|| answers.iter().map(|(l, _)| l + 1).max().unwrap_or(0).min(ladder.len() - 1));
    let (c, n) = score(placed);
    let confidence = if n == 0 {
        Confidence::Low
    } else if pass_rate(c, n) >= 0.5 || n <= 2 || answers.iter().any(|(l, ok)| *l > placed && *ok) {
        Confidence::Medium
    } else {
        Confidence::High
    };
    Some((ladder[placed], confidence))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub level: String,
    pub confidence: Confidence,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub gaps: Vec<String>,
}

impl AiOutput for Placement {
    const TASK: Task = Task::PlacementTest;

    fn validate(&self) -> Result<(), String> {
        check_level(&self.level)?;
        if self.strengths.len() > 4 || self.gaps.len() > 4 {
            return Err("at most 4 strengths and 4 gaps".into());
        }
        Ok(())
    }

    fn check(&self, input: &Value) -> Result<(), String> {
        let Some((level, confidence)) = placement_for(input) else { return Ok(()) };
        if self.level != level || self.confidence != confidence {
            return Err(format!(
                "by the placement rule this is level {level} with {confidence:?} confidence, not {} with {:?}",
                self.level, self.confidence
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LevelMove {
    Up,
    Down,
    Stay,
}

/// The monthly level check's rule (`content/golden/README.md`): up on a
/// passed level test, down only on clear and sustained evidence, at most
/// one level, never off either end.
pub fn level_decision(input: &Value) -> Option<(&'static str, LevelMove)> {
    let current = input_str(input, "current_level")?;
    let ladder = ladder(if current.starts_with('j') { "jv" } else { "id" });
    let i = ladder.iter().position(|l| *l == current)?;
    let stats = input.get("stats")?;
    let num = |v: Option<&Value>| v.and_then(Value::as_f64);
    let test = stats.get("level_test").filter(|t| !t.is_null());
    let score = num(test.and_then(|t| t.get("score")));
    let passed = score.zip(num(test.and_then(|t| t.get("pass_mark")))).is_some_and(|(s, p)| s >= p);
    let slipping =
        match (num(stats.get("review_accuracy")), num(stats.get("retention")), num(stats.get("active_days")))
        {
            (Some(acc), Some(ret), Some(days)) => acc < 0.55 && ret < 0.60 && days >= 14.0,
            _ => false,
        };
    let (to, moved) = if passed {
        (i + 1, LevelMove::Up)
    } else if score.is_some_and(|s| s < 0.45) || slipping {
        (i.wrapping_sub(1), LevelMove::Down)
    } else {
        (i, LevelMove::Stay)
    };
    Some(match ladder.get(to) {
        Some(l) => (*l, moved),
        None => (ladder[i], LevelMove::Stay),
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelCheck {
    pub level: String,
    pub moved: LevelMove,
    /// 2–4 short lines quoting the input.
    pub evidence: Vec<String>,
    pub next_focus: Vec<String>,
}

impl AiOutput for LevelCheck {
    const TASK: Task = Task::LevelCheck;

    fn validate(&self) -> Result<(), String> {
        check_level(&self.level)?;
        if !(1..=4).contains(&self.evidence.len()) {
            return Err("evidence: 2-4 short lines quoting the input".into());
        }
        if !(1..=3).contains(&self.next_focus.len()) {
            return Err("next_focus: 2-3 concrete units, lessons or skills".into());
        }
        Ok(())
    }

    fn check(&self, input: &Value) -> Result<(), String> {
        let Some((level, moved)) = level_decision(input) else { return Ok(()) };
        if self.level != level || self.moved != moved {
            return Err(format!(
                "by the level rule this is level {level}, moved {moved:?}, not {} / {:?}",
                self.level, self.moved
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    Review,
    Lesson,
    WeakSpot,
    Roleplay,
    Reading,
    Listening,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanStep {
    pub kind: PlanKind,
    pub minutes: u16,
    /// Quotes the input: the due count, the weak point, the lesson title.
    pub why: String,
}

/// *Rencana Hari Ini* as Kak Nur suggests it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyPlan {
    pub items: Vec<PlanStep>,
    pub message_ar: String,
    pub message_en: String,
}

impl AiOutput for DailyPlan {
    const TASK: Task = Task::DailyPlan;

    fn validate(&self) -> Result<(), String> {
        if self.items.is_empty() {
            return Err("items must not be empty".into());
        }
        for (i, s) in self.items.iter().enumerate() {
            if s.minutes == 0 {
                return Err(format!("items[{i}] has 0 minutes"));
            }
            filled(&s.why, &format!("items[{i}].why"))?;
        }
        arabic(&self.message_ar, "message_ar")?;
        filled(&self.message_en, "message_en")
    }

    fn check(&self, input: &Value) -> Result<(), String> {
        if let Some(goal) = input.get("goal_minutes").and_then(Value::as_u64) {
            let total: u64 = self.items.iter().map(|s| u64::from(s.minutes)).sum();
            if total != goal {
                return Err(format!(
                    "item minutes add up to {total}; they must add up to goal_minutes ({goal})"
                ));
            }
        }
        let has = |k: PlanKind| self.items.iter().any(|s| s.kind == k);
        if has(PlanKind::Lesson) && input.get("next_lesson").is_none_or(Value::is_null) {
            return Err("next_lesson is null: plan no lesson".into());
        }
        if has(PlanKind::Review) && input.get("due_reviews").and_then(Value::as_u64) == Some(0) {
            return Err("due_reviews is 0: plan no review".into());
        }
        Ok(())
    }
}

/// The 30-second end-of-day recap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyRecap {
    pub message_ar: String,
    pub message_en: String,
    /// The one thing to remember.
    pub highlight: String,
    /// Tomorrow's first step.
    pub tomorrow: String,
}

impl AiOutput for DailyRecap {
    const TASK: Task = Task::DailyRecap;

    fn validate(&self) -> Result<(), String> {
        arabic(&self.message_ar, "message_ar")?;
        filled(&self.message_en, "message_en")?;
        filled(&self.highlight, "highlight")?;
        filled(&self.tomorrow, "tomorrow")
    }
}

/// Kak Nur's note on the week.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeeklyReport {
    pub summary_ar: String,
    pub summary_en: String,
    /// Empty for an empty week: never invented.
    #[serde(default)]
    pub wins: Vec<String>,
    pub focus_next_week: Vec<String>,
}

impl AiOutput for WeeklyReport {
    const TASK: Task = Task::WeeklyReport;

    fn validate(&self) -> Result<(), String> {
        arabic(&self.summary_ar, "summary_ar")?;
        filled(&self.summary_en, "summary_en")?;
        if self.wins.len() > 4 {
            return Err("at most 4 wins".into());
        }
        if !(1..=3).contains(&self.focus_next_week.len()) {
            return Err("focus_next_week: 1-3 items".into());
        }
        Ok(())
    }
}

/// One short message from Kak Nur for a trigger (missed day, comeback, …).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoachMessage {
    pub message_ar: String,
    pub message_en: String,
}

impl AiOutput for CoachMessage {
    const TASK: Task = Task::CoachMessage;

    fn validate(&self) -> Result<(), String> {
        arabic(&self.message_ar, "message_ar")?;
        filled(&self.message_en, "message_en")
    }
}
