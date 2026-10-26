//! Smart, account-aware reminders. Called by Kabar Cloud's cron dispatcher
//! (push through FCM) and by each device (local notifications), with the same
//! synced data, so once today's goal is met on any device every device stays
//! silent.

use kabar_protocol::coach::{ActivityEvent, CoachProfile, DailyPlan, LocalTime, ReminderState};

use super::voice::Line;

/// Everything a reminder decision needs.
#[derive(Clone, Copy, Debug)]
pub struct ReminderContext<'a> {
    pub profile: &'a CoachProfile,
    pub state: &'a ReminderState,
    pub events: &'a [ActivityEvent],
    /// Today's plan, if one exists yet.
    pub plan: Option<&'a DailyPlan>,
    pub now_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Send(Reminder),
    Silent(Silence),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reminder {
    pub kind: ReminderKind,
    /// What Kak Nur says; rendered on the device in the UI language.
    pub line: Line,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReminderKind {
    Study,
    /// After a break: light, no streak talk.
    WelcomeBack,
    WeeklyReport,
}

/// Why nothing was sent; logged by the cron and shown in a debug view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Silence {
    StyleOff,
    GoalMet,
    QuietHours,
    Snoozed,
    NotYet,
    DailyLimit,
}

/// How often Kabar Cloud's cron calls [`decide`]; a reminder fires at the
/// first call at or after its time.
pub const CRON_INTERVAL_MIN: u16 = 15;

pub fn decide(ctx: &ReminderContext) -> Decision {
    let _ = ctx;
    todo!("coach agent B")
}

/// When the next reminder would fire if nothing changes (for scheduling local
/// notifications on the device).
pub fn next_at(ctx: &ReminderContext) -> Option<u64> {
    let _ = ctx;
    todo!("coach agent B")
}

/// The learner's chosen times, nudged toward when they actually study.
pub fn effective_times(profile: &CoachProfile, events: &[ActivityEvent], now_ms: u64) -> Vec<LocalTime> {
    let _ = (profile, events, now_ms);
    todo!("coach agent B")
}

/// One tap to snooze.
pub fn snooze(state: &ReminderState, now_ms: u64, minutes: u32) -> ReminderState {
    let _ = (state, now_ms, minutes);
    todo!("coach agent B")
}

/// Records a sent reminder (resets the daily count on a new local day).
pub fn mark_sent(state: &ReminderState, now_ms: u64, utc_offset_min: i16) -> ReminderState {
    let _ = (state, now_ms, utc_offset_min);
    todo!("coach agent B")
}
