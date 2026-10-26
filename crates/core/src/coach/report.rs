//! The weekly report (Thursday evening by default): time, new words,
//! retention, Pelita, strongest and weakest skills, Liga Pribadi and next
//! week's focus. Kak Nur's note is picked by [`super::voice`].

use kabar_protocol::coach::{ActivityEvent, CoachProfile, LeagueResult, LocalDate, WeeklyReport};

/// The report for the 7 days starting `week_start` (a Monday).
pub fn weekly_report(
    events: &[ActivityEvent],
    week_start: LocalDate,
    streak: u32,
    league: LeagueResult,
) -> WeeklyReport {
    let _ = (events, week_start, streak, league);
    todo!("coach agent A")
}

/// If a report is due at `now_ms` and hasn't been made yet, the Monday of the
/// week it covers. `last_report` is the `week_start` of the newest report.
pub fn report_due(profile: &CoachProfile, now_ms: u64, last_report: Option<LocalDate>) -> Option<LocalDate> {
    let _ = (profile, now_ms, last_report);
    todo!("coach agent A")
}
