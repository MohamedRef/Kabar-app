//! The top of the Stats screen: one period at a time (the last 7 days, day
//! by day, or the last 4 weeks, week by week), its activity buckets and
//! totals, and the skills that need attention in it.
//!
//! The period switch changes the aggregation, not only a label: a week is
//! seven one-day buckets, a month is four seven-day buckets, and every total
//! (minutes, study days, reviews, retention, weak spots) covers that window.

use std::collections::BTreeMap;

use kabar_protocol::coach::{LocalDate, Skill};

use super::{DayTotal, heat_level};
use crate::coach::report::MIN_SKILL_ATTEMPTS;

/// A skill below this share of right answers is a weak spot.
pub const WEAK_SHARE: f32 = 0.75;
/// At most this many weak spots are shown, weakest first.
pub const MAX_WEAK: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Period {
    #[default]
    Week,
    Month,
}

impl Period {
    pub const ALL: [Self; 2] = [Self::Week, Self::Month];

    /// Days the period covers, today included.
    pub fn days(self) -> i32 {
        match self {
            Self::Week => 7,
            Self::Month => 28,
        }
    }

    /// Days per bar in the activity chart.
    pub fn bucket_days(self) -> i32 {
        match self {
            Self::Week => 1,
            Self::Month => 7,
        }
    }

    /// Stable id, for the remembered choice.
    pub fn id(self) -> &'static str {
        match self {
            Self::Week => "week",
            Self::Month => "month",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
}

/// One bar: `days` days from `start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bucket {
    pub start: LocalDate,
    pub days: i32,
    pub minutes: u32,
    pub reviews: u32,
    pub active_days: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overview {
    pub period: Period,
    /// Oldest first; the last one ends today.
    pub buckets: Vec<Bucket>,
    pub minutes: u32,
    pub reviews: u32,
    pub active_days: u32,
    /// Minutes in the same-length window just before, for "more than last".
    pub previous_minutes: u32,
}

fn bucket(totals: &BTreeMap<LocalDate, DayTotal>, start: LocalDate, days: i32) -> Bucket {
    let end = start.add_days(days - 1);
    totals.range(start..=end).fold(
        Bucket { start, days, minutes: 0, reviews: 0, active_days: 0 },
        |mut b, (_, t)| {
            b.minutes += t.minutes;
            b.reviews += t.reviews;
            b.active_days += u32::from(heat_level(t) > 0);
            b
        },
    )
}

/// The period ending today, bucketed for its chart.
pub fn overview(totals: &BTreeMap<LocalDate, DayTotal>, today: LocalDate, period: Period) -> Overview {
    let (days, step) = (period.days(), period.bucket_days());
    let first = today.add_days(1 - days);
    let buckets: Vec<Bucket> =
        (0..days / step).map(|i| bucket(totals, first.add_days(i * step), step)).collect();
    let previous = bucket(totals, first.add_days(-days), days);
    Overview {
        period,
        minutes: buckets.iter().map(|b| b.minutes).sum(),
        reviews: buckets.iter().map(|b| b.reviews).sum(),
        active_days: buckets.iter().map(|b| b.active_days).sum(),
        previous_minutes: previous.minutes,
        buckets,
    }
}

/// A skill with too many misses in the period.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeakSpot {
    pub skill: Skill,
    pub correct: u32,
    pub total: u32,
}

impl WeakSpot {
    pub fn share(&self) -> f32 {
        self.correct as f32 / self.total.max(1) as f32
    }
}

/// Skills with at least [`MIN_SKILL_ATTEMPTS`] answers and under
/// [`WEAK_SHARE`] right, weakest first (ties keep [`Skill::ALL`] order), at
/// most [`MAX_WEAK`]. `skills` is (correct, total) in [`Skill::ALL`] order,
/// as `level_check::recent_skills` returns.
pub fn weak_spots(skills: &[(u32, u32); 5]) -> Vec<WeakSpot> {
    let mut out: Vec<WeakSpot> = Skill::ALL
        .iter()
        .zip(skills)
        .filter(|(_, (_, total))| *total >= MIN_SKILL_ATTEMPTS)
        .map(|(&skill, &(correct, total))| WeakSpot { skill, correct: correct.min(total), total })
        .filter(|w| w.share() < WEAK_SHARE)
        .collect();
    // a/b < c/d without floats; stable, so ties keep skill order.
    out.sort_by(|a, b| {
        (u64::from(a.correct) * u64::from(b.total)).cmp(&(u64::from(b.correct) * u64::from(a.total)))
    });
    out.truncate(MAX_WEAK);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> LocalDate {
        LocalDate::from_ymd(y, m, day).unwrap()
    }

    fn totals(days: &[(LocalDate, u32, u32)]) -> BTreeMap<LocalDate, DayTotal> {
        days.iter()
            .map(|&(date, minutes, reviews)| (date, DayTotal { minutes, reviews, studied: true }))
            .collect()
    }

    #[test]
    fn week_is_seven_days_month_is_four_weeks() {
        let today = d(2026, 10, 29);
        let t = totals(&[
            (today, 10, 4),
            (d(2026, 10, 27), 5, 2),
            (d(2026, 10, 10), 20, 9),
            (d(2026, 10, 1), 7, 1),
        ]);
        let w = overview(&t, today, Period::Week);
        assert_eq!(w.buckets.len(), 7);
        assert_eq!(w.buckets.last().unwrap().start, today);
        assert_eq!(w.buckets[0].start, d(2026, 10, 23));
        assert_eq!((w.minutes, w.reviews, w.active_days), (15, 6, 2));

        let m = overview(&t, today, Period::Month);
        assert_eq!(m.buckets.len(), 4);
        assert!(m.buckets.iter().all(|b| b.days == 7));
        assert_eq!(m.buckets[0].start, d(2026, 10, 2), "28 days ending today");
        assert_eq!((m.minutes, m.reviews, m.active_days), (35, 15, 3), "Oct 1 is outside");
        assert_eq!(m.buckets[3].minutes, 15);
        assert_eq!(m.previous_minutes, 7, "the 28 days before hold Oct 1");
        assert_ne!(w, m, "switching the period changes the numbers");
    }

    #[test]
    fn previous_window_is_the_same_length() {
        let today = d(2026, 10, 29);
        let t = totals(&[(d(2026, 10, 22), 12, 0), (d(2026, 10, 16), 30, 0)]);
        assert_eq!(overview(&t, today, Period::Week).previous_minutes, 42);
        assert_eq!(overview(&t, today, Period::Week).minutes, 0);
    }

    #[test]
    fn weak_spots_need_enough_answers_and_sort_weakest_first() {
        // Reading, Listening, Writing, Speaking, Vocabulary.
        let s = [(2, 4), (5, 10), (9, 10), (1, 6), (30, 50)];
        let w = weak_spots(&s);
        let skills: Vec<Skill> = w.iter().map(|w| w.skill).collect();
        assert_eq!(skills, [Skill::Speaking, Skill::Listening, Skill::Vocabulary]);
        assert!(weak_spots(&[(10, 10); 5]).is_empty());
    }

    #[test]
    fn period_ids_round_trip() {
        for p in Period::ALL {
            assert_eq!(Period::from_id(p.id()), Some(p));
        }
        assert_eq!(Period::from_id("year"), None);
    }
}
