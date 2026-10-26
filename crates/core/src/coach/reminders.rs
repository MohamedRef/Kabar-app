//! Smart, account-aware reminders. Called by Kabar Cloud's cron dispatcher
//! (push through FCM) and by each device (local notifications), with the same
//! synced data, so once today's goal is met on any device every device stays
//! silent.
//!
//! The rules, in the order [`decide`] checks them:
//! 1. Style off → silent.
//! 2. Today's goal met (on any device; it's derived from synced events) → silent.
//! 3. Snoozed → silent.
//! 4. Inside quiet hours → silent.
//! 5. Daily limit reached: Gentle sends at most one a day, Steady one per study
//!    time (at most [`STEADY_MAX_PER_DAY`]); a lapsed learner gets at most one
//!    welcome-back a day whatever the style.
//! 6. Not yet: a reminder is due only within [`WINDOW_MIN`] after an effective
//!    study time (never past midnight), and only if nothing was sent since that
//!    time.
//!
//! Reminders never mention the streak or what might be lost: after a break
//! they're a light welcome back.

use std::collections::{BTreeMap, BTreeSet};

use kabar_protocol::coach::{
    ActivityEvent, ActivityKind, CoachProfile, DailyPlan, LocalDate, LocalTime, ReminderState, ReminderStyle,
    to_unix_ms,
};

use super::history;
use super::plan::COMEBACK_AFTER_DAYS;
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
/// How long after a study time its reminder may still go out.
pub const WINDOW_MIN: u16 = 120;
/// Steady style: one reminder per study time, but never more than this a day.
pub const STEADY_MAX_PER_DAY: u32 = 3;
/// How far back [`effective_times`] looks at when the learner really studies.
pub const LEARN_DAYS: u64 = 14;
/// Study starts this close to a chosen time count towards it.
pub const LEARN_RADIUS_MIN: i32 = 180;
/// Days of evidence needed before a time is nudged.
pub const LEARN_MIN_DAYS: usize = 4;
/// The most a chosen time is nudged.
pub const MAX_SHIFT_MIN: i32 = 60;

const MIN_MS: u64 = 60_000;
const DAY_MS: u64 = 86_400_000;
const DAY_MIN: i32 = 1440;
/// How many days ahead [`next_at`] looks.
const LOOKAHEAD_DAYS: i32 = 8;

pub fn decide(ctx: &ReminderContext) -> Decision {
    if ctx.profile.reminder_style == ReminderStyle::Off {
        return Decision::Silent(Silence::StyleOff);
    }
    let times = candidate_times(ctx.profile, &effective_times(ctx.profile, ctx.events, ctx.now_ms));
    decide_with(ctx, &times)
}

/// [`decide`] with the candidate times already worked out, so [`next_at`] can
/// simulate later moments against the same times.
fn decide_with(ctx: &ReminderContext, times: &[LocalTime]) -> Decision {
    let p = ctx.profile;
    if p.reminder_style == ReminderStyle::Off {
        return Decision::Silent(Silence::StyleOff);
    }
    let off = p.utc_offset_min;
    let now = ctx.now_ms;
    let today = LocalDate::from_unix_ms(now, off);
    let stats = history::day(ctx.events, today);
    let (goal_minutes, plan_items) = goal(p, ctx.plan, today);
    if stats.goal_met(goal_minutes, plan_items) {
        return Decision::Silent(Silence::GoalMet);
    }
    if ctx.state.snoozed_until_ms.is_some_and(|until| until > now) {
        return Decision::Silent(Silence::Snoozed);
    }
    if p.quiet_hours.contains(LocalTime::from_unix_ms(now, off)) {
        return Decision::Silent(Silence::QuietHours);
    }
    let lapsed = !stats.studied()
        && history::last_study_day(ctx.events, today)
            .is_some_and(|last| today.days_since(last) > COMEBACK_AFTER_DAYS);
    let limit = if lapsed || p.reminder_style == ReminderStyle::Gentle {
        1
    } else {
        (times.len() as u32).clamp(1, STEADY_MAX_PER_DAY)
    };
    if sent_on(ctx.state, today, off) >= limit {
        return Decision::Silent(Silence::DailyLimit);
    }
    let midnight = to_unix_ms(today.add_days(1), LocalTime(0), off);
    let due = times.iter().any(|&t| {
        let start = to_unix_ms(today, t, off);
        let end = (start + u64::from(WINDOW_MIN) * MIN_MS).min(midnight);
        start <= now && now < end && ctx.state.last_sent_ms.is_none_or(|sent| sent < start)
    });
    if !due {
        return Decision::Silent(Silence::NotYet);
    }
    let reminder = if lapsed {
        Reminder { kind: ReminderKind::WelcomeBack, line: Line::RemindWelcomeBack }
    } else if p.reminder_style == ReminderStyle::Gentle {
        Reminder { kind: ReminderKind::Study, line: Line::RemindGentle }
    } else {
        Reminder { kind: ReminderKind::Study, line: Line::RemindStudy }
    };
    Decision::Send(reminder)
}

/// Today's goal as (minutes, plan items): today's plan if there is one,
/// otherwise the learner's usual daily size.
fn goal(profile: &CoachProfile, plan: Option<&DailyPlan>, today: LocalDate) -> (u16, usize) {
    match plan.filter(|p| p.date == today) {
        Some(plan) => {
            let minutes: u16 = plan.items.iter().map(|i| i.minutes).fold(0, u16::saturating_add);
            (if minutes == 0 { plan.size.minutes() } else { minutes }, plan.items.len())
        }
        None => (profile.daily_size.minutes(), 0),
    }
}

/// Reminders already sent on `today`.
fn sent_on(state: &ReminderState, today: LocalDate, utc_offset_min: i16) -> u32 {
    match state.last_sent_ms {
        Some(ms) if LocalDate::from_unix_ms(ms, utc_offset_min) == today => state.sent_today,
        _ => 0,
    }
}

/// The times a reminder may go out at: effective times outside quiet hours;
/// Gentle keeps only the first.
fn candidate_times(profile: &CoachProfile, effective: &[LocalTime]) -> Vec<LocalTime> {
    let mut times: Vec<LocalTime> =
        effective.iter().copied().filter(|&t| !profile.quiet_hours.contains(t)).collect();
    if profile.reminder_style == ReminderStyle::Gentle {
        times.truncate(1);
    }
    times
}

/// When the next reminder would fire if nothing changes (for scheduling local
/// notifications on the device).
pub fn next_at(ctx: &ReminderContext) -> Option<u64> {
    if ctx.profile.reminder_style == ReminderStyle::Off {
        return None;
    }
    let off = ctx.profile.utc_offset_min;
    let now = ctx.now_ms;
    let times = candidate_times(ctx.profile, &effective_times(ctx.profile, ctx.events, now));
    if times.is_empty() {
        return None;
    }
    // A Send window can only open at one of these moments.
    let today = LocalDate::from_unix_ms(now, off);
    let mut moments = BTreeSet::from([now]);
    moments.extend(ctx.state.snoozed_until_ms);
    for n in 0..=LOOKAHEAD_DAYS {
        let day = today.add_days(n);
        moments.extend(times.iter().map(|&t| to_unix_ms(day, t, off)));
        moments.insert(to_unix_ms(day, ctx.profile.quiet_hours.end, off));
    }
    moments
        .into_iter()
        .filter(|&m| m >= now)
        .find(|&m| matches!(decide_with(&ReminderContext { now_ms: m, ..*ctx }, &times), Decision::Send(_)))
}

/// The learner's chosen times, nudged toward when they actually study.
///
/// Each study session in the last [`LEARN_DAYS`] days (start = `at_ms` minus
/// its active seconds) is matched to the nearest chosen time within
/// [`LEARN_RADIUS_MIN`]. With at least [`LEARN_MIN_DAYS`] days of evidence, the
/// time moves toward the median of each day's first start, by at most
/// [`MAX_SHIFT_MIN`], rounded to 5 minutes, and never into quiet hours.
/// Sorted, without duplicates.
pub fn effective_times(profile: &CoachProfile, events: &[ActivityEvent], now_ms: u64) -> Vec<LocalTime> {
    let off = profile.utc_offset_min;
    let mut chosen = profile.study_times.clone();
    chosen.sort();
    chosen.dedup();
    if chosen.is_empty() {
        return chosen;
    }
    let since = now_ms.saturating_sub(LEARN_DAYS * DAY_MS);
    // (chosen index, day) → that day's earliest start, as minutes from the chosen time.
    let mut firsts: BTreeMap<(usize, LocalDate), i32> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for e in events {
        if !seen.insert(e.id.as_str())
            || matches!(e.kind, ActivityKind::Purchase { .. })
            || e.at_ms <= since
            || e.at_ms > now_ms
        {
            continue;
        }
        let start = e.at_ms.saturating_sub(u64::from(e.seconds) * 1000);
        let t = LocalTime::from_unix_ms(start, off);
        let nearest = chosen
            .iter()
            .enumerate()
            .map(|(i, &c)| (i, minutes_between(c, t)))
            .min_by_key(|&(i, diff)| (diff.abs(), i));
        if let Some((i, diff)) = nearest.filter(|&(_, diff)| diff.abs() <= LEARN_RADIUS_MIN) {
            let first = firsts.entry((i, LocalDate::from_unix_ms(start, off))).or_insert(diff);
            *first = (*first).min(diff);
        }
    }
    let mut out: Vec<LocalTime> = chosen
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let mut offsets: Vec<i32> =
                firsts.range((i, LocalDate(i32::MIN))..=(i, LocalDate(i32::MAX))).map(|(_, &o)| o).collect();
            if offsets.len() < LEARN_MIN_DAYS {
                return c;
            }
            offsets.sort_unstable();
            let mid = offsets.len() / 2;
            let median = if offsets.len().is_multiple_of(2) {
                (offsets[mid - 1] + offsets[mid]).div_euclid(2)
            } else {
                offsets[mid]
            };
            let mut shift = round_to_5(median.clamp(-MAX_SHIFT_MIN, MAX_SHIFT_MIN));
            while shift != 0 && profile.quiet_hours.contains(shifted(c, shift)) {
                shift -= 5 * shift.signum();
            }
            shifted(c, shift)
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Signed minutes from `from` to `to` the short way round the clock, in [-720, 720).
fn minutes_between(from: LocalTime, to: LocalTime) -> i32 {
    (i32::from(to.0) - i32::from(from.0) + DAY_MIN + DAY_MIN / 2).rem_euclid(DAY_MIN) - DAY_MIN / 2
}

fn shifted(t: LocalTime, minutes: i32) -> LocalTime {
    LocalTime((i32::from(t.0) + minutes).rem_euclid(DAY_MIN) as u16)
}

/// Rounds to the nearest multiple of 5, halves away from zero.
fn round_to_5(m: i32) -> i32 {
    (m.abs() + 2) / 5 * 5 * m.signum()
}

/// One tap to snooze.
pub fn snooze(state: &ReminderState, now_ms: u64, minutes: u32) -> ReminderState {
    ReminderState { snoozed_until_ms: Some(now_ms + u64::from(minutes) * MIN_MS), ..state.clone() }
}

/// Records a sent reminder (resets the daily count on a new local day).
pub fn mark_sent(state: &ReminderState, now_ms: u64, utc_offset_min: i16) -> ReminderState {
    let today = LocalDate::from_unix_ms(now_ms, utc_offset_min);
    ReminderState {
        snoozed_until_ms: state.snoozed_until_ms,
        last_sent_ms: Some(now_ms),
        sent_today: sent_on(state, today, utc_offset_min) + 1,
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::{PlanItem, PlanItemKind, PlanSize, QuietHours};

    use super::*;
    use crate::coach::history::tests::{d, event};

    /// Cairo-like offset used throughout.
    const OFF: i16 = 120;

    fn at(date: LocalDate, h: u16, m: u16) -> u64 {
        to_unix_ms(date, LocalTime::hm(h, m), OFF)
    }

    fn profile(style: ReminderStyle, times: &[(u16, u16)]) -> CoachProfile {
        CoachProfile {
            name: "Mohamed".into(),
            study_times: times.iter().map(|&(h, m)| LocalTime::hm(h, m)).collect(),
            reminder_style: style,
            utc_offset_min: OFF,
            ..Default::default()
        }
    }

    /// A study session of `minutes` that started at local `h:m` on `date`.
    fn session(id: &str, date: LocalDate, h: u16, m: u16, minutes: u32) -> ActivityEvent {
        let mut e = event(id, date, minutes * 60, ActivityKind::Reviews { cards: 5, correct: 4 });
        e.at_ms = at(date, h, m) + u64::from(minutes) * MIN_MS;
        e
    }

    fn ctx<'a>(
        p: &'a CoachProfile,
        s: &'a ReminderState,
        events: &'a [ActivityEvent],
        now_ms: u64,
    ) -> ReminderContext<'a> {
        ReminderContext { profile: p, state: s, events, plan: None, now_ms }
    }

    fn study() -> Decision {
        Decision::Send(Reminder { kind: ReminderKind::Study, line: Line::RemindStudy })
    }

    #[test]
    fn sends_at_the_chosen_time_and_not_before() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(20, 0)]);
        let s = ReminderState::default();
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 19, 45))), Decision::Silent(Silence::NotYet));
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 20, 0))), study());
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 21, 59))), study());
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 22, 0))), Decision::Silent(Silence::NotYet));
    }

    #[test]
    fn style_off_is_always_silent() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Off, &[(20, 0)]);
        let s = ReminderState::default();
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 20, 0))), Decision::Silent(Silence::StyleOff));
        assert_eq!(next_at(&ctx(&p, &s, &[], at(today, 9, 0))), None);
    }

    #[test]
    fn goal_met_on_any_device_silences_every_device() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(20, 0)]);
        let s = ReminderState::default();
        // 15 minutes studied on the tablet this morning: the phone stays quiet.
        let events = [session("tab", today, 9, 0, 15)];
        assert_eq!(decide(&ctx(&p, &s, &events, at(today, 20, 0))), Decision::Silent(Silence::GoalMet));
        // Ten minutes isn't the Steady 15 yet.
        let events = [session("tab", today, 9, 0, 10)];
        assert_eq!(decide(&ctx(&p, &s, &events, at(today, 20, 0))), study());
    }

    #[test]
    fn goal_follows_todays_plan() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(20, 0)]);
        let s = ReminderState::default();
        let plan = DailyPlan {
            date: today,
            size: PlanSize::Quick,
            comeback: false,
            items: vec![PlanItem {
                id: "review".into(),
                kind: PlanItemKind::Review { cards: 10 },
                minutes: 5,
            }],
        };
        let events = [session("a", today, 9, 0, 5)];
        let c = ReminderContext { plan: Some(&plan), ..ctx(&p, &s, &events, at(today, 20, 0)) };
        assert_eq!(decide(&c), Decision::Silent(Silence::GoalMet));
        // All plan items done counts too, whatever the minutes.
        let done = [event("i", today, 0, ActivityKind::PlanItemDone { item_id: "review".into() })];
        let c = ReminderContext { plan: Some(&plan), ..ctx(&p, &s, &done, at(today, 20, 0)) };
        assert_eq!(decide(&c), Decision::Silent(Silence::GoalMet));
        // Yesterday's plan doesn't count for today.
        let old = DailyPlan { date: today.add_days(-1), ..plan.clone() };
        let c = ReminderContext { plan: Some(&old), ..ctx(&p, &s, &events, at(today, 20, 0)) };
        assert_eq!(decide(&c), study());
    }

    #[test]
    fn snooze_quiets_until_it_ends_and_keeps_counters() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(20, 0)]);
        let now = at(today, 20, 0);
        let s = snooze(&ReminderState::default(), now, 30);
        assert_eq!(s.snoozed_until_ms, Some(now + 30 * MIN_MS));
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 20, 15))), Decision::Silent(Silence::Snoozed));
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 20, 30))), study());
        assert_eq!(next_at(&ctx(&p, &s, &[], at(today, 20, 5))), Some(at(today, 20, 30)));
        let sent = mark_sent(&ReminderState::default(), now, OFF);
        let snoozed = snooze(&sent, now, 10);
        assert_eq!((snoozed.last_sent_ms, snoozed.sent_today), (Some(now), 1));
    }

    #[test]
    fn quiet_hours_are_respected() {
        let today = d(2026, 10, 26);
        let mut p = profile(ReminderStyle::Steady, &[(21, 0)]);
        p.quiet_hours = QuietHours { start: LocalTime::hm(21, 30), end: LocalTime::hm(7, 0) };
        let s = ReminderState::default();
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 21, 15))), study());
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 21, 45))), Decision::Silent(Silence::QuietHours));
        // A chosen time inside quiet hours never fires.
        let p = profile(ReminderStyle::Steady, &[(23, 0)]);
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 23, 0))), Decision::Silent(Silence::QuietHours));
        assert_eq!(next_at(&ctx(&p, &s, &[], at(today, 9, 0))), None);
    }

    #[test]
    fn steady_sends_once_per_time_up_to_three() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(9, 0), (13, 0), (17, 0), (20, 0)]);
        let mut s = ReminderState::default();
        for (h, expected) in [(9, true), (13, true), (17, true), (20, false)] {
            let now = at(today, h, 0);
            let d = decide(&ctx(&p, &s, &[], now));
            assert_eq!(d == study(), expected, "at {h}:00 got {d:?}");
            if expected {
                s = mark_sent(&s, now, OFF);
                // Covered: the next cron run in the same window stays quiet.
                let again = decide(&ctx(&p, &s, &[], now + 15 * MIN_MS));
                assert!(matches!(again, Decision::Silent(Silence::NotYet | Silence::DailyLimit)));
            } else {
                assert_eq!(d, Decision::Silent(Silence::DailyLimit));
            }
        }
        assert_eq!(s.sent_today, 3);
        // A new day starts the count again.
        let tomorrow = at(today.add_days(1), 9, 0);
        assert_eq!(decide(&ctx(&p, &s, &[], tomorrow)), study());
        assert_eq!(mark_sent(&s, tomorrow, OFF).sent_today, 1);
    }

    #[test]
    fn gentle_sends_one_a_day_at_the_first_time() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Gentle, &[(9, 0), (20, 0)]);
        let gentle = Decision::Send(Reminder { kind: ReminderKind::Study, line: Line::RemindGentle });
        let s = ReminderState::default();
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 9, 0))), gentle);
        // Missed the morning window: the evening time isn't used by Gentle.
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 20, 0))), Decision::Silent(Silence::NotYet));
        let s = mark_sent(&s, at(today, 9, 0), OFF);
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 9, 30))), Decision::Silent(Silence::DailyLimit));
    }

    #[test]
    fn a_lapsed_learner_gets_one_welcome_back_a_day() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(9, 0), (20, 0)]);
        let events = [session("old", today.add_days(-5), 20, 0, 10)];
        let s = ReminderState::default();
        let welcome =
            Decision::Send(Reminder { kind: ReminderKind::WelcomeBack, line: Line::RemindWelcomeBack });
        assert_eq!(decide(&ctx(&p, &s, &events, at(today, 9, 0))), welcome);
        let s = mark_sent(&s, at(today, 9, 0), OFF);
        assert_eq!(decide(&ctx(&p, &s, &events, at(today, 20, 0))), Decision::Silent(Silence::DailyLimit));
        // Three days away isn't a break yet.
        let events = [session("old", today.add_days(-3), 20, 0, 10)];
        let s = ReminderState::default();
        assert_eq!(decide(&ctx(&p, &s, &events, at(today, 9, 0))), study());
        // Back studying today: an ordinary nudge, not a welcome back.
        let events = [session("old", today.add_days(-5), 20, 0, 10), session("now", today, 8, 0, 2)];
        assert_eq!(decide(&ctx(&p, &s, &events, at(today, 9, 0))), study());
    }

    #[test]
    fn window_never_runs_past_midnight() {
        let today = d(2026, 10, 26);
        let mut p = profile(ReminderStyle::Steady, &[(23, 0)]);
        p.quiet_hours = QuietHours { start: LocalTime::hm(3, 0), end: LocalTime::hm(5, 0) };
        let s = ReminderState::default();
        assert_eq!(decide(&ctx(&p, &s, &[], at(today, 23, 45))), study());
        let after = at(today.add_days(1), 0, 15);
        assert_eq!(decide(&ctx(&p, &s, &[], after)), Decision::Silent(Silence::NotYet));
    }

    #[test]
    fn learns_when_the_learner_really_studies() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(20, 0)]);
        let now = at(today, 12, 0);
        // Five evenings at about 20:20–20:50.
        let events: Vec<ActivityEvent> = [(1, 20, 30), (2, 20, 40), (3, 20, 50), (4, 20, 20), (5, 20, 35)]
            .iter()
            .map(|&(ago, h, m)| session(&format!("e{ago}"), today.add_days(-ago), h, m, 10))
            .collect();
        assert_eq!(effective_times(&p, &events, now), [LocalTime::hm(20, 35)]);
        // Three days aren't consistent enough.
        assert_eq!(effective_times(&p, &events[..3], now), [LocalTime::hm(20, 0)]);
        // Studying much later still moves the time by at most an hour.
        let late: Vec<ActivityEvent> =
            (1..=5).map(|ago| session(&format!("l{ago}"), today.add_days(-ago), 22, 40, 5)).collect();
        assert_eq!(effective_times(&p, &late, now), [LocalTime::hm(21, 0)]);
        // Too far away (over three hours) or too long ago: ignored.
        let far: Vec<ActivityEvent> =
            (1..=5).map(|ago| session(&format!("f{ago}"), today.add_days(-ago), 9, 0, 5)).collect();
        assert_eq!(effective_times(&p, &far, now), [LocalTime::hm(20, 0)]);
        let old: Vec<ActivityEvent> =
            (15..=20).map(|ago| session(&format!("o{ago}"), today.add_days(-ago), 21, 0, 5)).collect();
        assert_eq!(effective_times(&p, &old, now), [LocalTime::hm(20, 0)]);
        // The decision uses the learned time.
        let s = ReminderState::default();
        assert_eq!(decide(&ctx(&p, &s, &events, at(today, 20, 0))), Decision::Silent(Silence::NotYet));
        assert_eq!(decide(&ctx(&p, &s, &events, at(today, 20, 35))), study());
    }

    #[test]
    fn learned_times_stay_out_of_quiet_hours() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(22, 0)]); // quiet from 22:30
        let late: Vec<ActivityEvent> =
            (1..=5).map(|ago| session(&format!("l{ago}"), today.add_days(-ago), 23, 0, 5)).collect();
        assert_eq!(effective_times(&p, &late, at(today, 12, 0)), [LocalTime::hm(22, 25)]);
    }

    #[test]
    fn several_times_learn_separately_and_handle_midnight() {
        let today = d(2026, 10, 26);
        let mut p = profile(ReminderStyle::Steady, &[(7, 0), (19, 0)]);
        p.quiet_hours = QuietHours { start: LocalTime::hm(1, 0), end: LocalTime::hm(5, 0) };
        let mut events = Vec::new();
        for ago in 1..=4 {
            events.push(session(&format!("m{ago}"), today.add_days(-ago), 6, 30, 5));
            events.push(session(&format!("n{ago}"), today.add_days(-ago), 19, 20, 5));
        }
        let now = at(today, 6, 0);
        assert_eq!(effective_times(&p, &events, now), [LocalTime::hm(6, 30), LocalTime::hm(19, 20)]);
        // 23:30 studies after midnight nudge forward across it.
        let p = CoachProfile { study_times: vec![LocalTime::hm(23, 30)], ..p };
        let events: Vec<ActivityEvent> =
            (1..=4).map(|ago| session(&format!("x{ago}"), today.add_days(-ago), 0, 10, 5)).collect();
        assert_eq!(effective_times(&p, &events, now), [LocalTime::hm(0, 10)]);
    }

    #[test]
    fn next_at_finds_the_next_send() {
        let today = d(2026, 10, 26);
        let p = profile(ReminderStyle::Steady, &[(9, 0), (20, 0)]);
        let s = ReminderState::default();
        assert_eq!(next_at(&ctx(&p, &s, &[], at(today, 7, 0))), Some(at(today, 9, 0)));
        // Inside a window: right now.
        assert_eq!(next_at(&ctx(&p, &s, &[], at(today, 9, 30))), Some(at(today, 9, 30)));
        // Already sent for this window: the evening.
        let sent = mark_sent(&s, at(today, 9, 0), OFF);
        assert_eq!(next_at(&ctx(&p, &sent, &[], at(today, 9, 30))), Some(at(today, 20, 0)));
        // Goal met today: tomorrow morning.
        let events = [session("a", today, 8, 0, 20)];
        assert_eq!(next_at(&ctx(&p, &s, &events, at(today, 8, 30))), Some(at(today.add_days(1), 9, 0)));
        // Late evening: tomorrow.
        assert_eq!(next_at(&ctx(&p, &s, &[], at(today, 23, 0))), Some(at(today.add_days(1), 9, 0)));
    }

    #[test]
    fn next_at_after_a_window_that_opened_in_quiet_hours() {
        let today = d(2026, 10, 26);
        let mut p = profile(ReminderStyle::Steady, &[(7, 0)]);
        p.quiet_hours = QuietHours { start: LocalTime::hm(22, 0), end: LocalTime::hm(7, 30) };
        let s = ReminderState::default();
        // 07:00 itself is quiet, so it's not a candidate time at all.
        assert_eq!(next_at(&ctx(&p, &s, &[], at(today, 6, 0))), None);
        let p = CoachProfile { study_times: vec![LocalTime::hm(7, 30), LocalTime::hm(12, 0)], ..p };
        let snoozed = snooze(&s, at(today, 7, 30), 60);
        assert_eq!(next_at(&ctx(&p, &snoozed, &[], at(today, 7, 31))), Some(at(today, 8, 30)));
    }

    #[test]
    fn mark_sent_counts_per_local_day() {
        let today = d(2026, 10, 26);
        let s = mark_sent(&ReminderState::default(), at(today, 9, 0), OFF);
        assert_eq!((s.last_sent_ms, s.sent_today), (Some(at(today, 9, 0)), 1));
        let s = mark_sent(&s, at(today, 13, 0), OFF);
        assert_eq!(s.sent_today, 2);
        // 23:30 UTC on the 26th is already the 27th at +2.
        let s = mark_sent(&s, at(today.add_days(1), 1, 30), OFF);
        assert_eq!(s.sent_today, 1);
    }

    #[test]
    fn helpers() {
        assert_eq!(minutes_between(LocalTime::hm(23, 30), LocalTime::hm(0, 10)), 40);
        assert_eq!(minutes_between(LocalTime::hm(0, 10), LocalTime::hm(23, 30)), -40);
        assert_eq!((round_to_5(-3), round_to_5(2), round_to_5(62), round_to_5(0)), (-5, 0, 60, 0));
    }
}
