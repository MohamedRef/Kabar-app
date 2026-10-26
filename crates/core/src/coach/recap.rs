//! The 30-second end-of-day recap: what I learned, one thing to remember,
//! tomorrow's first step.

use kabar_protocol::coach::{ActivityEvent, DayRecap, LocalDate, LocalTime, PlanItemKind};

/// The recap for `date`. `tomorrow_first` is the first item of tomorrow's
/// plan, if one has been built.
pub fn day_recap(
    events: &[ActivityEvent],
    date: LocalDate,
    goal_minutes: u16,
    plan_items: usize,
    tomorrow_first: Option<PlanItemKind>,
) -> DayRecap {
    let _ = (events, date, goal_minutes, plan_items, tomorrow_first);
    todo!("coach agent A")
}

/// Whether to offer the recap now: after the goal is met, or in the evening
/// once the learner's last study time has passed.
pub fn recap_ready(goal_met: bool, studied: bool, now: LocalTime, study_times: &[LocalTime]) -> bool {
    let _ = (goal_met, studied, now, study_times);
    todo!("coach agent A")
}
