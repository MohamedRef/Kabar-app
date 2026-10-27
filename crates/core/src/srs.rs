//! FSRS-6 scheduling, with each item's state derived from the review log.
//! Every device replays the same events in HLC order and gets the same
//! state, so reviews on the phone and the tablet never conflict.
// adapted from kabar-app main: crates/core/src/srs.rs

use std::collections::{BTreeMap, HashMap, HashSet};

use fsrs::{FSRS, FSRSItem, FSRSReview, MemoryState};
use serde::{Deserialize, Serialize};

use crate::review::{Grade, LoggedReview, ReviewEvent};

const DAY_MS: i64 = 86_400_000;
const MIN_SHORT_INTERVAL_MS: i64 = 10 * 60_000;
/// Below this many training reviews, personal parameters are noise; keep the defaults.
pub const MIN_REVIEWS_TO_OPTIMIZE: usize = 400;
/// Personal parameters are refitted at most once a week.
pub const REFIT_EVERY_MS: i64 = 7 * DAY_MS;
/// Reviews before 4 am still count for the day before, like Anki.
pub const ROLLOVER_HOUR: u8 = 4;

/// Where Mohamed's day starts, so "due today" matches his evening, not UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayClock {
    /// Minutes east of UTC (Cairo: 120, or 180 in summer time).
    pub utc_offset_minutes: i32,
    /// Answers after midnight but before this hour still count for yesterday.
    pub rollover_hour: u8,
}

impl Default for DayClock {
    fn default() -> Self {
        Self::local(120)
    }
}

impl DayClock {
    pub fn local(utc_offset_minutes: i32) -> Self {
        Self { utc_offset_minutes, rollover_hour: ROLLOVER_HOUR }
    }

    fn shift_ms(&self) -> i64 {
        i64::from(self.utc_offset_minutes) * 60_000 - i64::from(self.rollover_hour) * 3_600_000
    }

    /// The study day number for a timestamp.
    pub fn day(&self, at_ms: i64) -> i64 {
        (at_ms + self.shift_ms()).div_euclid(DAY_MS)
    }

    /// UTC ms at which study day `day` starts.
    pub fn day_start_ms(&self, day: i64) -> i64 {
        day * DAY_MS - self.shift_ms()
    }
}

/// Desired retention: how likely he should be to recall an item when it
/// comes due. Higher means more reviews.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Retention {
    #[serde(rename = "85")]
    R85,
    #[default]
    #[serde(rename = "90")]
    R90,
    #[serde(rename = "95")]
    R95,
}

impl Retention {
    pub const ALL: [Retention; 3] = [Self::R85, Self::R90, Self::R95];

    pub fn as_f32(self) -> f32 {
        match self {
            Self::R85 => 0.85,
            Self::R90 => 0.90,
            Self::R95 => 0.95,
        }
    }

    pub fn percent(self) -> u32 {
        (self.as_f32() * 100.0).round() as u32
    }
}

/// FSRS parameters fitted on his own review log ([`Scheduler::optimize`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FsrsParams {
    pub weights: Vec<f32>,
    /// Training items (reviews after an item's first) they were fitted on.
    pub reviews: u32,
    pub trained_at_ms: i64,
}

/// Review settings; synced last-writer-wins as one row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewSettings {
    pub retention: Retention,
    /// New items a day, across all lectures.
    pub new_per_day: u32,
    pub fsrs: Option<FsrsParams>,
}

pub const NEW_PER_DAY_CHOICES: [u32; 4] = [15, 30, 45, 60];

impl Default for ReviewSettings {
    fn default() -> Self {
        Self { retention: Retention::default(), new_per_day: 30, fsrs: None }
    }
}

impl ReviewSettings {
    /// A refit is due: never fitted, or the last fit is a week old.
    pub fn refit_due(&self, now_ms: i64) -> bool {
        self.fsrs.as_ref().is_none_or(|p| now_ms - p.trained_at_ms >= REFIT_EVERY_MS)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub stability: f32,
    pub difficulty: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CardState {
    pub item_id: String,
    pub memory: Option<Memory>,
    pub first_review_ms: Option<i64>,
    pub last_review_ms: Option<i64>,
    /// `None` for a new item.
    pub due_ms: Option<i64>,
    pub reps: u32,
    pub lapses: u32,
}

impl CardState {
    pub fn new(item_id: impl Into<String>) -> Self {
        Self { item_id: item_id.into(), ..Self::default() }
    }

    pub fn is_new(&self) -> bool {
        self.reps == 0
    }
}

pub struct Scheduler {
    fsrs: FSRS,
    decay: f32,
    pub desired_retention: f32,
    pub day: DayClock,
    pub max_interval_days: f32,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(&fsrs::DEFAULT_PARAMETERS, 0.9, DayClock::default()).expect("default parameters are valid")
    }
}

impl Scheduler {
    pub fn new(parameters: &[f32], desired_retention: f32, day: DayClock) -> Result<Self, fsrs::FSRSError> {
        let fsrs = FSRS::new(parameters)?;
        let decay = parameters.get(20).copied().unwrap_or(fsrs::DEFAULT_PARAMETERS[20]);
        Ok(Self { fsrs, decay, desired_retention, day, max_interval_days: 3650.0 })
    }

    /// His scheduler: personal parameters when he has valid ones (else the
    /// FSRS defaults), his desired retention, and his day.
    pub fn from_settings(settings: &ReviewSettings, utc_offset_minutes: i32) -> Self {
        let day = DayClock::local(utc_offset_minutes);
        let retention = settings.retention.as_f32();
        if let Some(params) = &settings.fsrs
            && let Ok(s) = Self::new(&params.weights, retention, day)
        {
            return s;
        }
        Self::new(&fsrs::DEFAULT_PARAMETERS, retention, day).expect("default parameters are valid")
    }

    fn elapsed_days(&self, state: &CardState, at_ms: i64) -> u32 {
        state.last_review_ms.map_or(0, |last| (self.day.day(at_ms) - self.day.day(last)).max(0) as u32)
    }

    fn next_states(&self, state: &CardState, at_ms: i64) -> fsrs::NextStates {
        let memory = state.memory.map(|m| MemoryState { stability: m.stability, difficulty: m.difficulty });
        self.fsrs
            .next_states(memory, self.desired_retention, self.elapsed_days(state, at_ms))
            .expect("FSRS states stay finite for valid parameters")
    }

    /// Applies one answer.
    pub fn apply(&self, state: &CardState, grade: Grade, at_ms: i64) -> CardState {
        let next = self.next_states(state, at_ms);
        let chosen = match grade {
            Grade::Again => next.again,
            Grade::Hard => next.hard,
            Grade::Good => next.good,
            Grade::Easy => next.easy,
        };
        let interval = chosen.interval.min(self.max_interval_days);
        CardState {
            item_id: state.item_id.clone(),
            memory: Some(Memory { stability: chosen.memory.stability, difficulty: chosen.memory.difficulty }),
            first_review_ms: state.first_review_ms.or(Some(at_ms)),
            last_review_ms: Some(at_ms),
            due_ms: Some(self.due_after(at_ms, interval)),
            reps: state.reps + 1,
            lapses: state.lapses + u32::from(grade == Grade::Again && !state.is_new()),
        }
    }

    /// Under a day: due after the exact interval (at least 10 min).
    /// Otherwise: due at the start of his day `interval` days later.
    fn due_after(&self, at_ms: i64, interval_days: f32) -> i64 {
        if interval_days < 1.0 {
            at_ms + ((interval_days * DAY_MS as f32) as i64).max(MIN_SHORT_INTERVAL_MS)
        } else {
            let days = interval_days.round() as i64;
            self.day.day_start_ms(self.day.day(at_ms) + days)
        }
    }

    /// Whether an answer moves the item's schedule. Study, review, case and
    /// oral answers always do. A question-bank or mock-exam answer is
    /// evidence about an item, and counts only when it says something real:
    ///
    /// - not for a new item (it waits for its first proper study);
    /// - not on a day the item was already answered (same-day repeats say
    ///   little about memory);
    /// - a right answer only once the item is due, so a mock the day after a
    ///   review doesn't inflate its interval. A miss always counts.
    ///
    /// Answers that don't count stay in the log for stats.
    pub fn admits(&self, state: &CardState, event: &ReviewEvent) -> bool {
        if !event.origin.is_evidence() {
            return true;
        }
        if state.is_new() {
            return false;
        }
        if state.last_review_ms.is_some_and(|l| self.day.day(l) == self.day.day(event.at_ms)) {
            return false;
        }
        event.grade == Grade::Again || state.due_ms.is_some_and(|d| d <= event.at_ms)
    }

    /// Replays a whole log in one pass: sorted by HLC, duplicate ids
    /// ignored, each admitted answer applied to its item's running state.
    /// `visit` sees every admitted answer with the state just before it.
    pub fn fold_log<'a>(
        &self,
        log: &'a [LoggedReview],
        mut visit: impl FnMut(&'a LoggedReview, &CardState),
    ) -> HashMap<&'a str, CardState> {
        let mut sorted: Vec<&LoggedReview> = log.iter().collect();
        sorted.sort_by(|a, b| a.hlc.cmp(&b.hlc));
        let mut seen = HashSet::with_capacity(log.len());
        let mut states: HashMap<&str, CardState> = HashMap::new();
        for e in sorted.into_iter().filter(|e| seen.insert(e.id.as_str())) {
            let id = e.event.item_id.as_str();
            let state = states.entry(id).or_insert_with(|| CardState::new(id));
            if self.admits(state, &e.event) {
                visit(e, state);
                *state = self.apply(state, e.event.grade, e.event.at_ms);
            }
        }
        states
    }

    /// Every item's state from the log (items never answered are absent).
    pub fn replay_all(&self, log: &[LoggedReview]) -> BTreeMap<String, CardState> {
        self.fold_log(log, |_, _| {}).into_iter().map(|(id, s)| (id.to_string(), s)).collect()
    }

    /// The interval in days each grade (Again, Hard, Good, Easy) would give now.
    pub fn intervals(&self, state: &CardState, at_ms: i64) -> [f32; 4] {
        let n = self.next_states(state, at_ms);
        [n.again, n.hard, n.good, n.easy].map(|s| s.interval.min(self.max_interval_days))
    }

    /// Probability of recall now, `None` for new items.
    pub fn retrievability(&self, state: &CardState, at_ms: i64) -> Option<f32> {
        let m = state.memory?;
        let last = state.last_review_ms?;
        let days = (at_ms - last).max(0) as f32 / DAY_MS as f32;
        Some(fsrs::current_retrievability(
            MemoryState { stability: m.stability, difficulty: m.difficulty },
            days,
            self.decay,
        ))
    }

    /// Items due now, most forgotten first. New items are not included.
    pub fn due<'a>(&self, states: impl IntoIterator<Item = &'a CardState>, at_ms: i64) -> Vec<&'a CardState> {
        let mut due: Vec<(&CardState, f32)> = states
            .into_iter()
            .filter(|s| s.due_ms.is_some_and(|d| d <= at_ms))
            .map(|s| (s, self.retrievability(s, at_ms).unwrap_or(0.0)))
            .collect();
        due.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.item_id.cmp(&b.0.item_id)));
        due.into_iter().map(|(s, _)| s).collect()
    }

    /// Items that started (were first answered) on today's study day.
    pub fn started_today<'a>(&self, states: impl IntoIterator<Item = &'a CardState>, at_ms: i64) -> usize {
        let today = self.day.day(at_ms);
        states.into_iter().filter(|s| s.first_review_ms.is_some_and(|f| self.day.day(f) == today)).count()
    }

    /// How many items fall due on each of the next `days` study days
    /// (index 0 = today, overdue included).
    pub fn forecast<'a>(
        &self,
        states: impl IntoIterator<Item = &'a CardState>,
        at_ms: i64,
        days: usize,
    ) -> Vec<u32> {
        let today = self.day.day(at_ms);
        let mut out = vec![0; days];
        for due in states.into_iter().filter_map(|s| s.due_ms) {
            let d = (self.day.day(due) - today).max(0) as usize;
            if d < days {
                out[d] += 1;
            }
        }
        out
    }

    /// FSRS training items from the log: every admitted answer after an
    /// item's first that came on a later study day, with the history
    /// before it.
    pub fn training_items(&self, log: &[LoggedReview]) -> Vec<FSRSItem> {
        let mut by_item: BTreeMap<&str, Vec<&ReviewEvent>> = BTreeMap::new();
        self.fold_log(log, |e, _| by_item.entry(e.event.item_id.as_str()).or_default().push(&e.event));
        let mut items = Vec::new();
        for evs in by_item.values() {
            let reviews: Vec<FSRSReview> = evs
                .iter()
                .enumerate()
                .map(|(i, e)| FSRSReview {
                    rating: e.grade.rating(),
                    delta_t: if i == 0 {
                        0
                    } else {
                        (self.day.day(e.at_ms) - self.day.day(evs[i - 1].at_ms)).max(0) as u32
                    },
                })
                .collect();
            // Same-day repeats are short-term steps FSRS can't fit, as in Anki.
            for end in (2..=reviews.len()).filter(|&end| reviews[end - 1].delta_t > 0) {
                items.push(FSRSItem { reviews: reviews[..end].to_vec() });
            }
        }
        items
    }

    /// Personal parameters from his own history, once there is enough of it.
    pub fn optimize(&self, log: &[LoggedReview], now_ms: i64) -> Option<FsrsParams> {
        let items = self.training_items(log);
        if items.len() < MIN_REVIEWS_TO_OPTIMIZE {
            return None;
        }
        let reviews = items.len() as u32;
        let weights =
            fsrs::compute_parameters(fsrs::ComputeParametersInput { train_set: items, ..Default::default() })
                .ok()?;
        // Keep only parameters FSRS itself accepts.
        FSRS::new(&weights).ok()?;
        Some(FsrsParams { weights, reviews, trained_at_ms: now_ms })
    }
}

/// How many new items may still start today.
pub fn new_left_today(settings: &ReviewSettings, started_today: usize) -> usize {
    (settings.new_per_day as usize).saturating_sub(started_today)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::{Hlc, Origin};

    const T0: i64 = 1_793_610_000_000; // 2026-11-02 09:00 UTC = 11:00 Cairo

    fn ev(id: &str, item: &str, at_ms: i64, grade: Grade, origin: Origin) -> LoggedReview {
        LoggedReview {
            id: id.into(),
            hlc: Hlc { wall_ms: at_ms, counter: 0, device: "s24".into() },
            event: ReviewEvent { item_id: item.into(), grade, origin, at_ms, took_ms: 4000 },
        }
    }

    fn review(id: &str, item: &str, at_ms: i64, grade: Grade) -> LoggedReview {
        ev(id, item, at_ms, grade, Origin::Review)
    }

    #[test]
    fn good_answers_grow_the_interval() {
        let s = Scheduler::default();
        let a = s.apply(&CardState::new("x"), Grade::Good, T0);
        let b = s.apply(&a, Grade::Good, a.due_ms.unwrap());
        assert!(
            b.due_ms.unwrap() - b.last_review_ms.unwrap() > a.due_ms.unwrap() - a.last_review_ms.unwrap()
        );
        assert_eq!(b.first_review_ms, Some(T0));
        assert_eq!(b.reps, 2);
    }

    #[test]
    fn again_is_a_lapse_only_after_learning() {
        let s = Scheduler::default();
        let a = s.apply(&CardState::new("x"), Grade::Again, T0);
        assert_eq!(a.lapses, 0);
        let b = s.apply(&a, Grade::Again, T0 + DAY_MS);
        assert_eq!(b.lapses, 1);
        // A short interval is at least ten minutes.
        assert!(a.due_ms.unwrap() - T0 >= MIN_SHORT_INTERVAL_MS);
    }

    #[test]
    fn replay_is_order_independent_and_deduplicated() {
        let s = Scheduler::default();
        let log = vec![review("1", "x", T0, Grade::Good), review("2", "x", T0 + 3 * DAY_MS, Grade::Hard)];
        let mut shuffled = vec![log[1].clone(), log[0].clone(), log[0].clone()];
        let a = s.replay_all(&log);
        let b = s.replay_all(&shuffled);
        assert_eq!(a, b);
        shuffled.truncate(1);
        assert_ne!(s.replay_all(&shuffled), a);
    }

    #[test]
    fn day_clock_rolls_over_at_4am_cairo() {
        let c = DayClock::local(120);
        // 01:30 UTC = 03:30 Cairo: still yesterday's study day.
        let before = T0 - 7 * 3_600_000 - 30 * 60_000;
        let after = before + 60 * 60_000; // 04:30 Cairo
        assert_eq!(c.day(before) + 1, c.day(after));
        assert_eq!(c.day(c.day_start_ms(c.day(after))), c.day(after));
        assert_eq!(c.day_start_ms(c.day(T0)), T0 - 9 * 3_600_000 + 2 * 3_600_000);
    }

    #[test]
    fn due_puts_most_forgotten_first() {
        let s = Scheduler::default();
        let weak = s.apply(&CardState::new("weak"), Grade::Again, T0);
        let strong = s.apply(&CardState::new("strong"), Grade::Easy, T0);
        let later = T0 + 400 * DAY_MS;
        let due = s.due([&strong, &weak], later);
        assert_eq!(due.iter().map(|c| c.item_id.as_str()).collect::<Vec<_>>(), ["weak", "strong"]);
        assert!(s.due([&strong], T0 + 60_000).is_empty());
    }

    #[test]
    fn intervals_are_ordered_and_match_apply() {
        let s = Scheduler::default();
        let a = s.apply(&CardState::new("x"), Grade::Good, T0);
        let at = T0 + 5 * DAY_MS;
        let i = s.intervals(&a, at);
        assert!(i[0] <= i[1] && i[1] <= i[2] && i[2] <= i[3], "{i:?}");
        let good = s.apply(&a, Grade::Good, at);
        assert_eq!(s.day.day(good.due_ms.unwrap()) - s.day.day(at), i[2].round() as i64);
    }

    #[test]
    fn higher_retention_means_shorter_intervals() {
        let at = |r: Retention| {
            let s = Scheduler::from_settings(&ReviewSettings { retention: r, ..Default::default() }, 120);
            let a = s.apply(&CardState::new("x"), Grade::Good, T0);
            s.intervals(&a, T0 + 3 * DAY_MS)[2]
        };
        assert!(at(Retention::R95) < at(Retention::R90));
        assert!(at(Retention::R90) < at(Retention::R85));
    }

    #[test]
    fn bank_answers_follow_the_evidence_rules() {
        let s = Scheduler::default();
        let bank = |id, at, g| ev(id, "x", at, g, Origin::Bank);
        // A new item: a bank answer waits for proper study.
        assert!(s.replay_all(&[bank("1", T0, Grade::Good)]).get("x").unwrap().is_new());
        let studied = review("0", "x", T0, Grade::Good);
        let base = s.replay_all(std::slice::from_ref(&studied))["x"].clone();
        // Same day: ignored, even a miss.
        let same = s.replay_all(&[studied.clone(), bank("1", T0 + 3_600_000, Grade::Again)]);
        assert_eq!(same["x"], base);
        // Next day, not due yet: a right answer is ignored, a miss counts.
        let next = T0 + DAY_MS;
        assert!(base.due_ms.unwrap() > next);
        assert_eq!(s.replay_all(&[studied.clone(), bank("1", next, Grade::Good)])["x"], base);
        assert_eq!(s.replay_all(&[studied.clone(), bank("1", next, Grade::Again)])["x"].reps, 2);
        // Once due, a right answer counts.
        let due = base.due_ms.unwrap() + 1;
        assert_eq!(s.replay_all(&[studied, bank("1", due, Grade::Good)])["x"].reps, 2);
    }

    #[test]
    fn started_today_and_the_daily_cap() {
        let s = Scheduler::default();
        let log = vec![
            review("1", "a", T0 - DAY_MS, Grade::Good),
            review("2", "b", T0, Grade::Good),
            review("3", "a", T0, Grade::Good),
        ];
        let states = s.replay_all(&log);
        assert_eq!(s.started_today(states.values(), T0 + 3_600_000), 1);
        let settings = ReviewSettings { new_per_day: 15, ..Default::default() };
        assert_eq!(new_left_today(&settings, 1), 14);
        assert_eq!(new_left_today(&settings, 20), 0);
    }

    #[test]
    fn forecast_counts_by_day() {
        let s = Scheduler::default();
        let a = s.apply(&CardState::new("a"), Grade::Again, T0);
        let b = s.apply(&CardState::new("b"), Grade::Good, T0);
        let f = s.forecast([&a, &b], T0, 30);
        assert_eq!(f.iter().sum::<u32>(), 2);
        assert_eq!(f[0], 1);
    }

    #[test]
    fn training_needs_400_reviews_and_refits_weekly() {
        let s = Scheduler::default();
        let mut log = Vec::new();
        for item in 0..150 {
            for (n, gap) in [0, 1, 4, 11].into_iter().enumerate() {
                let g = if (item + n) % 7 == 0 { Grade::Again } else { Grade::Good };
                log.push(review(
                    &format!("{item}-{n}"),
                    &format!("i{item}"),
                    T0 + gap * DAY_MS + item as i64,
                    g,
                ));
            }
        }
        assert_eq!(s.training_items(&log).len(), 450);
        assert!(s.optimize(&log[..200], T0).is_none());
        let p = s.optimize(&log, T0).expect("fits");
        assert_eq!(p.reviews, 450);
        assert!(Scheduler::new(&p.weights, 0.9, DayClock::default()).is_ok());

        let mut settings = ReviewSettings::default();
        assert!(settings.refit_due(T0));
        settings.fsrs = Some(p);
        assert!(!settings.refit_due(T0 + 6 * DAY_MS));
        assert!(settings.refit_due(T0 + 7 * DAY_MS));
    }

    #[test]
    fn settings_round_trip_and_default_on_missing_fields() {
        let s: ReviewSettings = serde_json::from_str(r#"{"retention":"95"}"#).unwrap();
        assert_eq!(s.retention, Retention::R95);
        assert_eq!(s.new_per_day, 30);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<ReviewSettings>(&json).unwrap(), s);
    }
}
