//! "At this pace you reach BIPA 3 around <date>" (SPEC §6), from the
//! learner's actual study minutes, not the plan they hoped for.
//!
//! The coach owns the hours a level takes ([`BIPA_HOURS`], also behind the
//! onboarding target); this only divides what is left by the real pace. It
//! is an estimate, and the UI says so (Kak Nur's `level_forecast` line).
//!
//! "Reach BIPA N" means finishing BIPA N, the same reading as the onboarding
//! target ("BIPA 1 by March").

use std::collections::BTreeMap;

use kabar_protocol::coach::LocalDate;

use super::DayTotal;
use crate::coach::onboarding::BIPA_HOURS;

/// The pace looks back this far...
pub const PACE_WINDOW_DAYS: i32 = 28;
/// ...but never divides by fewer days than this, so a keen first day
/// doesn't promise BIPA 3 next month.
pub const MIN_PACE_DAYS: i32 = 7;
/// Further away than this is not a forecast worth showing.
pub const MAX_FORECAST_DAYS: i32 = 5 * 365;

/// Average study minutes per calendar day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    pub minutes_per_day: f32,
    /// How many days the average is over.
    pub days: u32,
}

/// The pace over the last [`PACE_WINDOW_DAYS`] (or since the first study day,
/// at least [`MIN_PACE_DAYS`]). `None` without any minutes in that window.
pub fn pace(totals: &BTreeMap<LocalDate, DayTotal>, today: LocalDate) -> Option<Pace> {
    let first = totals.iter().find(|(_, t)| t.minutes > 0).map(|(d, _)| *d)?;
    let days = (today.days_since(first) + 1).clamp(MIN_PACE_DAYS, PACE_WINDOW_DAYS);
    let minutes: u32 = totals.range(today.add_days(1 - days)..=today).map(|(_, t)| t.minutes).sum();
    (minutes > 0).then(|| Pace { minutes_per_day: minutes as f32 / days as f32, days: days as u32 })
}

/// 1–7 for `bipa1`…`bipa7` (also `bipa-2`); `None` for other tracks.
pub fn bipa_number(level: &str) -> Option<usize> {
    let n: usize = level.strip_prefix("bipa")?.trim_start_matches('-').parse().ok()?;
    (1..=BIPA_HOURS.len()).contains(&n).then_some(n)
}

/// Hours of study left to finish `goal`, being in `current` with `done_share`
/// (0–1) of its lessons behind. `None` off the BIPA track or if `goal` is
/// already behind.
pub fn hours_left(current: &str, done_share: f32, goal: &str) -> Option<f32> {
    let (c, g) = (bipa_number(current)?, bipa_number(goal)?);
    if g < c {
        return None;
    }
    let this = BIPA_HOURS[c - 1] as f32 * (1.0 - done_share.clamp(0.0, 1.0));
    let later: u32 = BIPA_HOURS[c..g].iter().sum();
    Some(this + later as f32)
}

/// The day the learner would finish `goal` at `pace`.
pub fn eta(current: &str, done_share: f32, goal: &str, pace: Pace, today: LocalDate) -> Option<LocalDate> {
    let hours = hours_left(current, done_share, goal)?;
    if pace.minutes_per_day <= 0.0 {
        return None;
    }
    let days = (hours * 60.0 / pace.minutes_per_day).ceil();
    (days <= MAX_FORECAST_DAYS as f32).then(|| today.add_days(days as i32))
}

/// What to forecast: the learner's target if it is ahead, else the level
/// they are in.
pub fn goal_level<'a>(current: &'a str, target: Option<&'a str>) -> &'a str {
    match (bipa_number(current), target.and_then(|t| bipa_number(t).map(|n| (t, n)))) {
        (Some(c), Some((t, n))) if n >= c => t,
        _ => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::history::tests::d;

    fn totals(days: &[(LocalDate, u32)]) -> BTreeMap<LocalDate, DayTotal> {
        days.iter().map(|&(d, m)| (d, DayTotal { minutes: m, reviews: 0, studied: m > 0 })).collect()
    }

    #[test]
    fn pace_averages_calendar_days_with_a_floor() {
        let today = d(2026, 10, 26);
        assert_eq!(pace(&BTreeMap::new(), today), None);
        // One keen first day: divided by 7, not 1.
        let p = pace(&totals(&[(today, 70)]), today).unwrap();
        assert_eq!((p.minutes_per_day, p.days), (10.0, 7));
        // 40 days of 15 minutes: the last 28 count.
        let long: Vec<_> = (0..40).map(|i| (today.add_days(-i), 15)).collect();
        let p = pace(&totals(&long), today).unwrap();
        assert_eq!((p.minutes_per_day, p.days), (15.0, 28));
        // Studied long ago, nothing lately: no pace to speak of.
        assert_eq!(pace(&totals(&[(today.add_days(-60), 30)]), today), None);
    }

    #[test]
    fn hours_left_counts_this_level_and_the_ones_between() {
        let h = |level: usize| BIPA_HOURS[level - 1] as f32;
        assert_eq!(hours_left("bipa1", 0.0, "bipa1"), Some(h(1)));
        assert_eq!(hours_left("bipa1", 0.5, "bipa1"), Some(h(1) / 2.0));
        assert_eq!(hours_left("bipa2", 0.25, "bipa3"), Some(h(2) * 0.75 + h(3)));
        assert_eq!(hours_left("bipa3", 0.0, "bipa2"), None);
        assert_eq!(hours_left("j1", 0.0, "j2"), None);
        assert_eq!(bipa_number("bipa-2"), Some(2));
        assert_eq!(bipa_number("bipa9"), None);
    }

    #[test]
    fn eta_divides_by_the_real_pace() {
        let today = d(2026, 10, 26);
        // All of BIPA 1 left at 30 min/day = 2 days per hour.
        let hours = BIPA_HOURS[0] as i32;
        let p = Pace { minutes_per_day: 30.0, days: 28 };
        assert_eq!(eta("bipa1", 0.0, "bipa1", p, today), Some(today.add_days(2 * hours)));
        // Half done: half as long.
        assert_eq!(eta("bipa1", 0.5, "bipa1", p, today), Some(today.add_days(hours)));
        // A trickle: too far away to promise anything.
        let slow = Pace { minutes_per_day: 0.5, days: 28 };
        assert_eq!(eta("bipa1", 0.0, "bipa7", slow, today), None);
    }

    #[test]
    fn goal_is_the_target_when_ahead() {
        assert_eq!(goal_level("bipa1", Some("bipa3")), "bipa3");
        assert_eq!(goal_level("bipa2", Some("bipa1")), "bipa2");
        assert_eq!(goal_level("bipa2", None), "bipa2");
        assert_eq!(goal_level("bipa2", Some("j1")), "bipa2");
    }
}
