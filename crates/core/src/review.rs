//! Review events: what every study screen reports after an answer, how they
//! are stamped for sync (HLC), which of them move the schedule, and the seam
//! between the study screens and the store ([`StudyStore`]).
//!
//! Scheduling itself is in [`crate::srs`]; Mohamed's ✓ / ✎ / ✗ verdicts on
//! the content are in [`verify`].

pub mod verify;

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::content::{Body, Item};

/// The four FSRS ratings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Again,
    Hard,
    Good,
    Easy,
}

impl Grade {
    pub const ALL: [Grade; 4] = [Self::Again, Self::Hard, Self::Good, Self::Easy];

    /// FSRS's rating number (1–4).
    pub fn rating(self) -> u32 {
        match self {
            Self::Again => 1,
            Self::Hard => 2,
            Self::Good => 3,
            Self::Easy => 4,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Again => "again",
            Self::Hard => "hard",
            Self::Good => "good",
            Self::Easy => "easy",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.as_str() == s)
    }
}

/// Which screen produced the answer, for stats and for weighting later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Study,
    Review,
    Bank,
    Mock,
    Case,
    Oral,
}

impl Origin {
    pub const ALL: [Origin; 6] = [Self::Study, Self::Review, Self::Bank, Self::Mock, Self::Case, Self::Oral];

    /// Answers from the question bank and mock exams are evidence about an
    /// item, not a review of it: they move its schedule only under the rules
    /// in [`crate::srs::Scheduler::admits`].
    pub fn is_evidence(self) -> bool {
        matches!(self, Self::Bank | Self::Mock)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Study => "study",
            Self::Review => "review",
            Self::Bank => "bank",
            Self::Mock => "mock",
            Self::Case => "case",
            Self::Oral => "oral",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.as_str() == s)
    }
}

/// One answer to one item. Append-only: never edited, never deleted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewEvent {
    pub item_id: String,
    pub grade: Grade,
    pub origin: Origin,
    /// Unix time in milliseconds.
    pub at_ms: i64,
    /// Time spent on the item, in milliseconds.
    pub took_ms: u32,
}

/// Where answers go. The store implements it (`crate::store` in the app).
pub trait ReviewSink {
    fn record(&mut self, event: ReviewEvent);
}

/// Keeps events in memory: for tests and for screens that land before the store.
#[derive(Debug, Default)]
pub struct MemorySink {
    pub events: Vec<ReviewEvent>,
}

impl ReviewSink for MemorySink {
    fn record(&mut self, event: ReviewEvent) {
        self.events.push(event);
    }
}

// --- Sync stamps ----------------------------------------------------------
// adapted from kabar-app main: crates/protocol/src/hlc.rs

/// Hybrid logical clock timestamp. Ordered by wall time, then counter, then
/// device, so two devices never produce equal stamps and last-writer-wins is
/// deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hlc {
    /// Milliseconds since the Unix epoch.
    pub wall_ms: i64,
    /// Logical counter for stamps within the same millisecond.
    pub counter: u32,
    pub device: String,
}

impl Ord for Hlc {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.wall_ms, self.counter, &self.device).cmp(&(other.wall_ms, other.counter, &other.device))
    }
}

impl PartialOrd for Hlc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Hands out increasing [`Hlc`]s for one device, even when the wall clock
/// goes backwards.
#[derive(Debug, Clone)]
pub struct HlcClock {
    device: String,
    last_ms: i64,
    counter: u32,
}

impl HlcClock {
    pub fn new(device: impl Into<String>) -> Self {
        Self { device: device.into(), last_ms: i64::MIN, counter: 0 }
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    pub fn tick(&mut self, now_ms: i64) -> Hlc {
        if now_ms > self.last_ms {
            self.last_ms = now_ms;
            self.counter = 0;
        } else {
            self.counter += 1;
        }
        Hlc { wall_ms: self.last_ms, counter: self.counter, device: self.device.clone() }
    }

    /// Moves past a stamp seen elsewhere (another device, or the store on
    /// start-up), so the next local stamp sorts after it.
    pub fn observe(&mut self, seen: &Hlc) {
        if (seen.wall_ms, seen.counter) > (self.last_ms, self.counter) {
            self.last_ms = seen.wall_ms;
            self.counter = seen.counter;
        }
    }
}

/// A review event as stored and synced: with its own id and HLC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggedReview {
    /// Unique per event; replays ignore duplicates.
    pub id: String,
    pub hlc: Hlc,
    #[serde(flatten)]
    pub event: ReviewEvent,
}

// --- Self-rating ----------------------------------------------------------

/// Kinds with no single right answer to check: Mohamed rates his own recall
/// (Again / Hard / Good / Easy) after seeing the model answer.
pub fn self_rated(body: &Body) -> bool {
    matches!(body, Body::Enumerate { .. } | Body::Essay { .. } | Body::Case { .. })
}

/// A grade from an "Enumerate" item: how many of `total` points he recalled,
/// against the `pass` mark. Below the pass mark is Again; all of them (when
/// that is more than the pass mark) is Easy; a bare pass is Hard.
pub fn enumerate_grade(recalled: usize, total: usize, pass: usize) -> Grade {
    let pass = pass.clamp(1, total.max(1));
    if recalled < pass {
        Grade::Again
    } else if recalled >= total && total > pass {
        Grade::Easy
    } else if recalled == pass && pass < total {
        Grade::Hard
    } else {
        Grade::Good
    }
}

// --- The seam with the study player ---------------------------------------

/// What the study player (chunk 2) and the other study screens need from the
/// review store. The app provides one [`StudyStoreHandle`] through context.
pub trait StudyStore {
    fn record(&mut self, event: ReviewEvent);
    /// Item ids due now, most overdue (most forgotten) first. No new items,
    /// nothing Mohamed marked ✗.
    fn due(&self, now_ms: i64) -> Vec<String>;
    /// Never answered yet.
    fn is_new(&self, item_id: &str) -> bool;
    /// Marked ✗ wrong: out of review until the content is fixed.
    fn is_hidden(&self, item_id: &str) -> bool;
    /// The item as Mohamed should see it: with his ✎ edit applied.
    fn effective(&self, item: &Item) -> Item {
        item.clone()
    }
    /// When the next item falls due, if any.
    fn next_due_ms(&self, now_ms: i64) -> Option<i64>;
    /// How many new items may still start today (the daily new-item cap).
    fn new_left_today(&self, _now_ms: i64) -> usize {
        usize::MAX
    }
    /// The interval in days each grade (Again, Hard, Good, Easy) would give
    /// the item now, for "1d · 3d · 9d" on the grade buttons.
    fn intervals(&self, _item_id: &str, _now_ms: i64) -> Option<[f32; 4]> {
        None
    }
}

/// The shared store, as the app hands it out through Dioxus context.
#[derive(Clone)]
pub struct StudyStoreHandle(pub Rc<RefCell<dyn StudyStore>>);

impl StudyStoreHandle {
    pub fn new(store: impl StudyStore + 'static) -> Self {
        Self(Rc::new(RefCell::new(store)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_sink_keeps_order() {
        let mut sink = MemorySink::default();
        for (i, grade) in [Grade::Again, Grade::Good].into_iter().enumerate() {
            sink.record(ReviewEvent {
                item_id: format!("hb.L01.00{i}"),
                grade,
                origin: Origin::Study,
                at_ms: i as i64,
                took_ms: 1000,
            });
        }
        assert_eq!(sink.events.len(), 2);
        assert_eq!(sink.events[1].grade, Grade::Good);
    }

    #[test]
    fn hlc_orders_by_wall_then_counter_then_device() {
        let h = |w, c, d: &str| Hlc { wall_ms: w, counter: c, device: d.into() };
        assert!(h(1, 9, "z") < h(2, 0, "a"));
        assert!(h(2, 0, "z") < h(2, 1, "a"));
        assert!(h(2, 1, "a") < h(2, 1, "b"));
    }

    #[test]
    fn hlc_clock_never_goes_back() {
        let mut c = HlcClock::new("s24");
        let a = c.tick(1000);
        let b = c.tick(1000);
        let back = c.tick(500); // the wall clock jumped back
        assert!(a < b && b < back);
        c.observe(&Hlc { wall_ms: 5000, counter: 3, device: "tab".into() });
        let after = c.tick(2000);
        assert_eq!((after.wall_ms, after.counter), (5000, 4));
    }

    #[test]
    fn enumerate_grades_against_the_pass_mark() {
        assert_eq!(enumerate_grade(2, 6, 4), Grade::Again);
        assert_eq!(enumerate_grade(4, 6, 4), Grade::Hard);
        assert_eq!(enumerate_grade(5, 6, 4), Grade::Good);
        assert_eq!(enumerate_grade(6, 6, 4), Grade::Easy);
        // When every point is needed to pass, full recall is Good.
        assert_eq!(enumerate_grade(3, 3, 3), Grade::Good);
        assert_eq!(enumerate_grade(0, 3, 0), Grade::Again);
    }

    #[test]
    fn names_round_trip() {
        for g in Grade::ALL {
            assert_eq!(Grade::parse(g.as_str()), Some(g));
        }
        for o in Origin::ALL {
            assert_eq!(Origin::parse(o.as_str()), Some(o));
        }
    }

    #[test]
    fn logged_review_flattens_the_event() {
        let l = LoggedReview {
            id: "s24-1-0".into(),
            hlc: Hlc { wall_ms: 1, counter: 0, device: "s24".into() },
            event: ReviewEvent {
                item_id: "hb.L01.001".into(),
                grade: Grade::Good,
                origin: Origin::Review,
                at_ms: 1,
                took_ms: 900,
            },
        };
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains("\"item_id\":\"hb.L01.001\""));
        assert_eq!(serde_json::from_str::<LoggedReview>(&json).unwrap(), l);
    }
}
