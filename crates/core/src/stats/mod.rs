//! Stats (SPEC §16): the study heatmap, known words per language, true
//! retention, time studied and the review forecast; plus the monthly level
//! check's pure parts ([`level_check`]) and the pace forecast ([`pace`]).
//!
//! Pure functions of the synced logs (Kak Nur's activity log, the review log)
//! and the learner's local calendar, so every device draws the same charts.
//!
//! Rules, stated once:
//! - **Minutes on a day** are the larger of the coach's logged seconds and the
//!   review log's answer time that day: a finished review session logs both,
//!   an interrupted one only the review log, so neither is double counted.
//! - **Known word:** some card of the word has FSRS stability of at least
//!   [`KNOWN_STABILITY_DAYS`] (it would still be recalled with 90% odds three
//!   weeks from its last review). **Learning:** reviewed, not yet known.
//!   **New:** never reviewed. A word is its (language, word) pair, so the
//!   recognition and Bridge cards of "kabar" count once.
//! - **True retention** is the share of reviews of already-learned cards not
//!   answered "Again". **Mature** reviews are those whose scheduled interval
//!   before the answer was at least [`MATURE_INTERVAL_DAYS`] (Anki's rule).

pub mod level_check;
pub mod pace;

use std::collections::{BTreeMap, HashSet};

use kabar_protocol::coach::{ActivityEvent, LocalDate};
use kabar_protocol::srs::{Card, Rating, ReviewEvent, StudyLanguage};

use crate::coach::history;
use crate::srs::{CardState, Scheduler};

const DAY_MS: u64 = 86_400_000;
/// Stability (days) from which a word counts as known.
pub const KNOWN_STABILITY_DAYS: f32 = 21.0;
/// Scheduled interval (days) from which a review counts as mature.
pub const MATURE_INTERVAL_DAYS: f32 = 21.0;
/// The heatmap's span.
pub const HEATMAP_WEEKS: u32 = 20;
/// Review forecast span on the Stats screen.
pub const FORECAST_DAYS: usize = 30;

/// One local day's study.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DayTotal {
    pub minutes: u32,
    pub reviews: u32,
    /// Any learning at all (a lesson step, a plan item, a review...).
    pub studied: bool,
}

/// Study per local day, from both logs (see the module rules).
pub fn day_totals(
    events: &[ActivityEvent],
    log: &[ReviewEvent],
    utc_offset_min: i16,
) -> BTreeMap<LocalDate, DayTotal> {
    let mut out: BTreeMap<LocalDate, DayTotal> = BTreeMap::new();
    for (date, s) in history::by_day(events) {
        out.insert(date, DayTotal { minutes: s.minutes(), reviews: s.reviews, studied: s.studied() });
    }
    // The review log: answer time and count per day, each event once.
    let mut seen = HashSet::new();
    let mut by_day: BTreeMap<LocalDate, (u64, u32)> = BTreeMap::new();
    // Lesson and lookup evidence is study the activity log already counts.
    for e in log.iter().filter(|e| seen.insert(e.id.as_str()) && e.evidence.is_review()) {
        let d = by_day.entry(LocalDate::from_unix_ms(e.at_ms, utc_offset_min)).or_default();
        d.0 += u64::from(e.duration_ms);
        d.1 += 1;
    }
    for (date, (ms, n)) in by_day {
        let t = out.entry(date).or_default();
        t.minutes = t.minutes.max(ms.div_ceil(60_000) as u32);
        t.reviews = t.reviews.max(n);
        t.studied = true;
    }
    out
}

/// One heatmap cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeatDay {
    pub date: LocalDate,
    pub minutes: u32,
    pub reviews: u32,
    /// 0 (nothing) to 4; see [`heat_level`].
    pub level: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heatmap {
    /// The Monday of the first column.
    pub start: LocalDate,
    /// Every day from `start` to today inclusive (the last column may be partial).
    pub days: Vec<HeatDay>,
    pub active_days: u32,
    pub minutes: u32,
}

impl Heatmap {
    pub fn weeks(&self) -> usize {
        self.days.len().div_ceil(7)
    }
}

/// The shade of a day: the plan sizes (5, 15, 30 minutes) are the steps, and
/// any study at all, however short, lights the first one.
pub fn heat_level(t: &DayTotal) -> u8 {
    match t.minutes {
        30.. => 4,
        15.. => 3,
        5.. => 2,
        1.. => 1,
        0 if t.studied || t.reviews > 0 => 1,
        0 => 0,
    }
}

/// The last `weeks` calendar weeks (Monday first) up to `today`.
pub fn heatmap(totals: &BTreeMap<LocalDate, DayTotal>, today: LocalDate, weeks: u32) -> Heatmap {
    let start = today.week_start().add_days(-7 * (weeks.max(1) as i32 - 1));
    let mut days = Vec::new();
    let (mut active_days, mut minutes) = (0, 0);
    let mut d = start;
    while d <= today {
        let t = totals.get(&d).copied().unwrap_or_default();
        let level = heat_level(&t);
        active_days += u32::from(level > 0);
        minutes += t.minutes;
        days.push(HeatDay { date: d, minutes: t.minutes, reviews: t.reviews, level });
        d = d.add_days(1);
    }
    Heatmap { start, days, active_days, minutes }
}

/// Minutes and study days over a window ending today.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeSpan {
    pub minutes: u32,
    pub days: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeStudied {
    /// The last 7 days, today included.
    pub week: TimeSpan,
    /// The last 30 days, today included.
    pub month: TimeSpan,
    /// Everything ever logged.
    pub total_minutes: u32,
}

pub fn window(totals: &BTreeMap<LocalDate, DayTotal>, today: LocalDate, days: i32) -> TimeSpan {
    totals.range(today.add_days(1 - days)..=today).fold(TimeSpan::default(), |acc, (_, t)| TimeSpan {
        minutes: acc.minutes + t.minutes,
        days: acc.days + u32::from(heat_level(t) > 0),
    })
}

pub fn time_studied(totals: &BTreeMap<LocalDate, DayTotal>, today: LocalDate) -> TimeStudied {
    TimeStudied {
        week: window(totals, today, 7),
        month: window(totals, today, 30),
        total_minutes: totals.values().map(|t| t.minutes).sum(),
    }
}

/// Word counts for one language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordCounts {
    pub language: StudyLanguage,
    pub known: u32,
    pub learning: u32,
    pub new: u32,
}

impl WordCounts {
    pub fn total(&self) -> u32 {
        self.known + self.learning + self.new
    }
}

/// Known / learning / new words per language (see the module rules), in
/// [`StudyLanguage`] order, only languages that have cards.
pub fn word_counts(cards: &[Card], state: impl Fn(&str) -> CardState) -> Vec<WordCounts> {
    // Best stability per word, `None` while never reviewed.
    let mut words: BTreeMap<(StudyLanguage, &str), Option<f32>> = BTreeMap::new();
    for c in cards {
        let s = state(&c.id);
        let stability = (!s.is_new()).then(|| s.memory.map_or(0.0, |m| m.stability));
        let slot = words.entry(c.sibling_key()).or_insert(None);
        *slot = match (*slot, stability) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }
    let mut out: BTreeMap<StudyLanguage, WordCounts> = BTreeMap::new();
    for ((language, _), best) in words {
        let w = out.entry(language).or_insert(WordCounts { language, known: 0, learning: 0, new: 0 });
        match best {
            Some(s) if s >= KNOWN_STABILITY_DAYS => w.known += 1,
            Some(_) => w.learning += 1,
            None => w.new += 1,
        }
    }
    out.into_values().collect()
}

/// Passed out of total reviews.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub passed: u32,
    pub total: u32,
}

impl Tally {
    pub fn share(&self) -> Option<f32> {
        (self.total > 0).then(|| self.passed as f32 / self.total as f32)
    }

    fn add(&mut self, passed: bool) {
        self.total += 1;
        self.passed += u32::from(passed);
    }
}

/// True retention over one window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Retention {
    pub mature: Tally,
    pub young: Tally,
}

impl Retention {
    /// Every learned-card review, mature or young.
    pub fn all(&self) -> Tally {
        Tally { passed: self.mature.passed + self.young.passed, total: self.mature.total + self.young.total }
    }
}

/// True retention of the reviews in the last `days` days, split into mature
/// and young by the interval the card was scheduled at before the answer.
pub fn retention(scheduler: &Scheduler, log: &[ReviewEvent], now_ms: u64, days: u64) -> Retention {
    replay_with_retention(scheduler, log, now_ms, &[days]).1[0]
}

/// One pass over the log for everything Stats needs from it: every card's
/// state (as [`Scheduler::replay_all`]) and [`retention`] for each window in
/// `windows_days`, in that order.
pub fn replay_with_retention(
    scheduler: &Scheduler,
    log: &[ReviewEvent],
    now_ms: u64,
    windows_days: &[u64],
) -> (BTreeMap<String, CardState>, Vec<Retention>) {
    let since: Vec<u64> = windows_days.iter().map(|d| now_ms.saturating_sub(d * DAY_MS)).collect();
    let mut out = vec![Retention::default(); windows_days.len()];
    let states = scheduler.fold_log(log, |e, state| {
        if state.is_new() || !e.evidence.is_review() {
            return;
        }
        let interval_days = match (state.last_review_ms, state.due_ms) {
            (Some(last), Some(due)) => due.saturating_sub(last) as f32 / DAY_MS as f32,
            _ => 0.0,
        };
        let passed = e.rating != Rating::Again;
        for (r, &since) in out.iter_mut().zip(&since) {
            if e.at_ms < since {
                continue;
            }
            if interval_days >= MATURE_INTERVAL_DAYS {
                r.mature.add(passed);
            } else {
                r.young.add(passed);
            }
        }
    });
    (states.into_iter().map(|(id, s)| (id.to_string(), s)).collect(), out)
}

/// The review forecast's headline numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ForecastSummary {
    pub total: u32,
    /// Busiest day (0 = today) and its count; ties go to the earlier day.
    pub peak_day: usize,
    pub peak: u32,
    /// Average per day, rounded (at least 1 while anything is due).
    pub per_day: u32,
}

pub fn forecast_summary(days: &[u32]) -> ForecastSummary {
    let total = days.iter().sum();
    let (peak_day, peak) =
        days.iter().copied().enumerate().fold((0, 0), |best, (i, n)| if n > best.1 { (i, n) } else { best });
    // Never "about 0 a day" while something is due.
    let per_day = if days.is_empty() {
        0
    } else {
        ((total as f32 / days.len() as f32).round() as u32).max(u32::from(total > 0))
    };
    ForecastSummary { total, peak_day, peak, per_day }
}

#[cfg(test)]
mod tests;
