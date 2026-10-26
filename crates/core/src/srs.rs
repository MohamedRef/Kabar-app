//! Review engine: FSRS scheduling with card state derived from the review
//! log. Every device replays the same events in HLC order and gets the same
//! state, so concurrent reviews on the phone and the tablet never conflict.

use std::collections::{BTreeMap, HashSet};

use fsrs::{FSRS, FSRSItem, FSRSReview, MemoryState};
use kabar_protocol::srs::{Rating, ReviewEvent};
use serde::{Deserialize, Serialize};

const DAY_MS: u64 = 86_400_000;
const MIN_SHORT_INTERVAL_MS: u64 = 10 * 60_000;
/// Below this many reviews, personal parameters are noise; keep the defaults.
pub const MIN_REVIEWS_TO_OPTIMIZE: usize = 400;

/// Where the learner's day starts, so "due today" matches their evening, not UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayClock {
    pub utc_offset_minutes: i32,
    /// Reviews after midnight but before this hour still count for yesterday.
    pub rollover_hour: u8,
}

impl Default for DayClock {
    fn default() -> Self {
        // Cairo, rolling over at 4 am like Anki.
        Self { utc_offset_minutes: 180, rollover_hour: 4 }
    }
}

impl DayClock {
    fn shift_ms(&self) -> i64 {
        i64::from(self.utc_offset_minutes) * 60_000 - i64::from(self.rollover_hour) * 3_600_000
    }

    /// The learner's day number for a timestamp.
    pub fn day(&self, at_ms: u64) -> i64 {
        (at_ms as i64 + self.shift_ms()).div_euclid(DAY_MS as i64)
    }

    /// UTC ms at which learner day `day` starts.
    pub fn day_start_ms(&self, day: i64) -> u64 {
        (day * DAY_MS as i64 - self.shift_ms()).max(0) as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub stability: f32,
    pub difficulty: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CardState {
    pub card_id: String,
    pub memory: Option<Memory>,
    pub last_review_ms: Option<u64>,
    /// `None` for a new card.
    pub due_ms: Option<u64>,
    pub reps: u32,
    pub lapses: u32,
}

impl CardState {
    pub fn new(card_id: impl Into<String>) -> Self {
        Self { card_id: card_id.into(), ..Self::default() }
    }

    pub fn is_new(&self) -> bool {
        self.reps == 0
    }
}

/// One answer button's outcome, for showing "1d · 3d · 9d" before answering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preview {
    pub rating: Rating,
    pub interval_days: f32,
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

    fn elapsed_days(&self, state: &CardState, at_ms: u64) -> u32 {
        state.last_review_ms.map_or(0, |last| (self.day.day(at_ms) - self.day.day(last)).max(0) as u32)
    }

    fn next_states(&self, state: &CardState, at_ms: u64) -> fsrs::NextStates {
        let memory = state.memory.map(|m| MemoryState { stability: m.stability, difficulty: m.difficulty });
        self.fsrs
            .next_states(memory, self.desired_retention, self.elapsed_days(state, at_ms))
            .expect("FSRS states stay finite for valid parameters")
    }

    /// Applies one answer.
    pub fn apply(&self, state: &CardState, rating: Rating, at_ms: u64) -> CardState {
        let next = self.next_states(state, at_ms);
        let chosen = match rating {
            Rating::Again => next.again,
            Rating::Hard => next.hard,
            Rating::Good => next.good,
            Rating::Easy => next.easy,
        };
        let interval = chosen.interval.min(self.max_interval_days);
        CardState {
            card_id: state.card_id.clone(),
            memory: Some(Memory { stability: chosen.memory.stability, difficulty: chosen.memory.difficulty }),
            last_review_ms: Some(at_ms),
            due_ms: Some(self.due_after(at_ms, interval)),
            reps: state.reps + 1,
            lapses: state.lapses + u32::from(rating == Rating::Again && !state.is_new()),
        }
    }

    /// Under a day: due after the exact interval (at least 10 min).
    /// Otherwise: due at the start of the learner's day `interval` days later.
    fn due_after(&self, at_ms: u64, interval_days: f32) -> u64 {
        if interval_days < 1.0 {
            at_ms + ((interval_days * DAY_MS as f32) as u64).max(MIN_SHORT_INTERVAL_MS)
        } else {
            let days = interval_days.round() as i64;
            self.day.day_start_ms(self.day.day(at_ms) + days)
        }
    }

    /// Rebuilds a card from its events: sorted by HLC, duplicates ignored.
    pub fn replay<'a>(&self, card_id: &str, events: impl IntoIterator<Item = &'a ReviewEvent>) -> CardState {
        let mut evs: Vec<&ReviewEvent> = events.into_iter().filter(|e| e.card_id == card_id).collect();
        evs.sort_by(|a, b| a.hlc.cmp(&b.hlc));
        let mut seen = HashSet::new();
        evs.into_iter()
            .filter(|e| seen.insert(e.id.as_str()))
            .fold(CardState::new(card_id), |s, e| self.apply(&s, e.rating, e.at_ms))
    }

    /// Replays every card in a log.
    pub fn replay_all(&self, events: &[ReviewEvent]) -> BTreeMap<String, CardState> {
        let mut ids: Vec<&str> = events.iter().map(|e| e.card_id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().map(|id| (id.to_string(), self.replay(id, events))).collect()
    }

    pub fn preview(&self, state: &CardState, at_ms: u64) -> [Preview; 4] {
        let n = self.next_states(state, at_ms);
        let p = |rating, s: fsrs::ItemState| Preview {
            rating,
            interval_days: s.interval.min(self.max_interval_days),
        };
        [p(Rating::Again, n.again), p(Rating::Hard, n.hard), p(Rating::Good, n.good), p(Rating::Easy, n.easy)]
    }

    /// Probability of recall now, `None` for new cards.
    pub fn retrievability(&self, state: &CardState, at_ms: u64) -> Option<f32> {
        let m = state.memory?;
        let last = state.last_review_ms?;
        let days = at_ms.saturating_sub(last) as f32 / DAY_MS as f32;
        Some(fsrs::current_retrievability(
            MemoryState { stability: m.stability, difficulty: m.difficulty },
            days,
            self.decay,
        ))
    }

    /// Cards due now, most-forgotten first. New cards are not included.
    pub fn due<'a>(&self, states: impl IntoIterator<Item = &'a CardState>, at_ms: u64) -> Vec<&'a CardState> {
        let mut due: Vec<(&CardState, f32)> = states
            .into_iter()
            .filter(|s| s.due_ms.is_some_and(|d| d <= at_ms))
            .map(|s| (s, self.retrievability(s, at_ms).unwrap_or(0.0)))
            .collect();
        due.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.card_id.cmp(&b.0.card_id)));
        due.into_iter().map(|(s, _)| s).collect()
    }

    /// How many cards fall due on each of the next `days` learner days.
    pub fn forecast<'a>(
        &self,
        states: impl IntoIterator<Item = &'a CardState>,
        at_ms: u64,
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

    /// FSRS training items from the log: every review after a card's first,
    /// with the history before it. Day gaps use the learner's day clock.
    pub fn training_items(&self, events: &[ReviewEvent]) -> Vec<FSRSItem> {
        let mut by_card: BTreeMap<&str, Vec<&ReviewEvent>> = BTreeMap::new();
        let mut seen = HashSet::new();
        for e in events.iter().filter(|e| seen.insert(e.id.as_str())) {
            by_card.entry(e.card_id.as_str()).or_default().push(e);
        }
        let mut items = Vec::new();
        for evs in by_card.values_mut() {
            evs.sort_by(|a, b| a.hlc.cmp(&b.hlc));
            let reviews: Vec<FSRSReview> = evs
                .iter()
                .enumerate()
                .map(|(i, e)| FSRSReview {
                    rating: e.rating as u32,
                    delta_t: if i == 0 {
                        0
                    } else {
                        (self.day.day(e.at_ms) - self.day.day(evs[i - 1].at_ms)).max(0) as u32
                    },
                })
                .collect();
            for end in 2..=reviews.len() {
                items.push(FSRSItem { reviews: reviews[..end].to_vec() });
            }
        }
        items
    }

    /// Personal parameters from the learner's own history, once there is enough of it.
    pub fn optimize(&self, events: &[ReviewEvent]) -> Option<Vec<f32>> {
        let items = self.training_items(events);
        if items.len() < MIN_REVIEWS_TO_OPTIMIZE {
            return None;
        }
        fsrs::compute_parameters(fsrs::ComputeParametersInput { train_set: items, ..Default::default() }).ok()
    }
}

/// Card Wall piles on the tablet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pile {
    /// tahu: I know it.
    Tahu,
    /// belum: not yet.
    Belum,
}

/// The FSRS grade from where and how fast a card was flicked.
pub fn card_wall_rating(pile: Pile, sort_ms: u32) -> Rating {
    match (pile, sort_ms) {
        (Pile::Belum, _) => Rating::Again,
        (Pile::Tahu, 0..=2_500) => Rating::Easy,
        (Pile::Tahu, 2_501..=8_000) => Rating::Good,
        (Pile::Tahu, _) => Rating::Hard,
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::srs::ReviewMode;
    use kabar_protocol::{DeviceId, Hlc};

    use super::*;

    const T0: u64 = 1_761_472_800_000; // 26 Oct 2026, 10:00 UTC

    fn ev(id: &str, card: &str, wall: u64, device: &str, rating: Rating) -> ReviewEvent {
        ReviewEvent {
            id: id.into(),
            card_id: card.into(),
            hlc: Hlc { wall_ms: wall, counter: 0, device: DeviceId(device.into()) },
            at_ms: wall,
            rating,
            duration_ms: 3000,
            mode: ReviewMode::Classic,
        }
    }

    #[test]
    fn good_answers_grow_the_interval() {
        let s = Scheduler::default();
        let mut card = CardState::new("kabar");
        let mut at = T0;
        let mut last_gap = 0;
        for _ in 0..4 {
            card = s.apply(&card, Rating::Good, at);
            let gap = card.due_ms.unwrap() - at;
            assert!(gap > last_gap, "intervals grow");
            last_gap = gap;
            at = card.due_ms.unwrap();
        }
        assert_eq!((card.reps, card.lapses), (4, 0));
    }

    #[test]
    fn again_is_a_lapse_only_after_learning() {
        let s = Scheduler::default();
        let first = s.apply(&CardState::new("x"), Rating::Again, T0);
        assert_eq!(first.lapses, 0);
        assert!(first.due_ms.unwrap() - T0 < DAY_MS, "relearn the same day");
        let later = s.apply(&s.apply(&first, Rating::Good, T0 + DAY_MS), Rating::Again, T0 + 4 * DAY_MS);
        assert_eq!(later.lapses, 1);
    }

    #[test]
    fn replay_is_order_independent_and_deduplicated() {
        let s = Scheduler::default();
        let a = ev("1", "kabar", T0, "s24u", Rating::Good);
        let b = ev("2", "kabar", T0 + 3 * DAY_MS, "tab", Rating::Hard);
        let c = ev("3", "kabar", T0 + 9 * DAY_MS, "s24u", Rating::Good);
        let phone = s.replay("kabar", [&a, &b, &c]);
        let tablet = s.replay("kabar", [&c, &a, &b, &b]);
        assert_eq!(phone, tablet);
        assert_eq!(phone.reps, 3);
    }

    #[test]
    fn due_queue_puts_most_forgotten_first() {
        let s = Scheduler::default();
        let fresh = s.apply(&CardState::new("fresh"), Rating::Easy, T0);
        let shaky = s.apply(&CardState::new("shaky"), Rating::Hard, T0);
        let new = CardState::new("new");
        let later = T0 + 30 * DAY_MS;
        let due: Vec<&str> =
            s.due([&fresh, &shaky, &new], later).iter().map(|c| c.card_id.as_str()).collect();
        assert_eq!(due, ["shaky", "fresh"]);
        assert!(s.due([&fresh], T0).is_empty());
    }

    #[test]
    fn preview_is_ordered_and_matches_apply() {
        let s = Scheduler::default();
        let card = CardState::new("x");
        let p = s.preview(&card, T0);
        assert!(p.windows(2).all(|w| w[0].interval_days <= w[1].interval_days));
        let applied = s.apply(&card, Rating::Easy, T0);
        let expected = s.day.day_start_ms(s.day.day(T0) + p[3].interval_days.round() as i64);
        assert_eq!(applied.due_ms, Some(expected));
    }

    #[test]
    fn day_clock_rolls_over_at_4am_cairo() {
        let d = DayClock::default();
        // 00:30 UTC = 03:30 Cairo, still "yesterday"; 01:30 UTC = 04:30 Cairo, a new day.
        let midnight_utc = 1_761_436_800_000; // 26 Oct 2026 00:00 UTC
        assert_eq!(d.day(midnight_utc + 30 * 60_000) + 1, d.day(midnight_utc + 90 * 60_000));
        let start = d.day_start_ms(d.day(T0));
        assert_eq!(d.day(start), d.day(T0));
        assert_eq!(d.day(start - 1), d.day(T0) - 1);
    }

    #[test]
    fn retrievability_drops_over_time() {
        let s = Scheduler::default();
        let card = s.apply(&CardState::new("x"), Rating::Good, T0);
        let r1 = s.retrievability(&card, T0 + DAY_MS).unwrap();
        let r10 = s.retrievability(&card, T0 + 10 * DAY_MS).unwrap();
        assert!(r1 > r10 && r1 <= 1.0 && r10 > 0.0);
        assert!(s.retrievability(&CardState::new("y"), T0).is_none());
    }

    #[test]
    fn forecast_counts_by_day() {
        let s = Scheduler::default();
        let cards: Vec<CardState> =
            (0..3).map(|i| s.apply(&CardState::new(format!("c{i}")), Rating::Good, T0)).collect();
        let f = s.forecast(&cards, T0, 7);
        assert_eq!(f.iter().sum::<u32>(), 3);
    }

    #[test]
    fn training_items_and_optimizer_threshold() {
        let s = Scheduler::default();
        let log = vec![
            ev("1", "a", T0, "m", Rating::Good),
            ev("2", "a", T0 + 2 * DAY_MS, "m", Rating::Good),
            ev("3", "a", T0 + 7 * DAY_MS, "m", Rating::Again),
            ev("4", "b", T0, "m", Rating::Easy),
        ];
        let items = s.training_items(&log);
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].reviews.iter().map(|r| r.delta_t).collect::<Vec<_>>(), [0, 2, 5]);
        assert!(s.optimize(&log).is_none(), "too little history keeps the defaults");
    }

    #[test]
    fn card_wall_grades() {
        assert_eq!(card_wall_rating(Pile::Belum, 500), Rating::Again);
        assert_eq!(card_wall_rating(Pile::Tahu, 1_200), Rating::Easy);
        assert_eq!(card_wall_rating(Pile::Tahu, 5_000), Rating::Good);
        assert_eq!(card_wall_rating(Pile::Tahu, 20_000), Rating::Hard);
    }
}
