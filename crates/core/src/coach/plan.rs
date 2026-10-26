//! *Rencana Hari Ini*: today's plan from due reviews, the current unit and
//! weak spots, sized by "5, 15 or 30 minutes today?".

use kabar_protocol::coach::{ActivityEvent, DailyPlan, Focus, LocalDate, PlanSize};

use super::history::DayStats;

/// Days without study after which the plan becomes a short, light comeback.
pub const COMEBACK_AFTER_DAYS: i32 = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanInputs<'a> {
    pub date: LocalDate,
    pub size: PlanSize,
    /// Cards due today (from the review engine).
    pub due_reviews: u32,
    /// The unit the learner is on, if any (from the Journey).
    pub current_unit: Option<&'a str>,
    /// Weak spots, most urgent first (see [`weak_spots`]).
    pub weak_spots: &'a [Focus],
    /// The last day with any study before `date`.
    pub last_study_day: Option<LocalDate>,
}

/// Builds the plan. Deterministic: the same inputs give the same plan with
/// the same item ids on every device.
pub fn build_plan(inputs: &PlanInputs) -> DailyPlan {
    let _ = inputs;
    todo!("coach agent A")
}

/// Topics answered worst over the `window_days` before `today`, worst first.
pub fn weak_spots(events: &[ActivityEvent], today: LocalDate, window_days: i32) -> Vec<Focus> {
    let _ = (events, today, window_days);
    todo!("coach agent A")
}

/// How far today's plan has come.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanProgress {
    /// One flag per plan item, in order.
    pub done: Vec<bool>,
    pub minutes_done: u32,
    pub goal_met: bool,
}

pub fn progress(plan: &DailyPlan, today: &DayStats) -> PlanProgress {
    let _ = (plan, today);
    todo!("coach agent A")
}
