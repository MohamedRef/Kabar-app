//! Folds the append-only activity log into per-day numbers. Every other coach
//! module reads activity through here, so XP and "studied today" mean the same
//! thing everywhere.

use std::collections::{BTreeMap, BTreeSet};

use kabar_protocol::coach::{ActivityEvent, ActivityKind, LocalDate, Skill};

/// What one local day added up to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DayStats {
    pub seconds: u32,
    pub reviews: u32,
    pub reviews_correct: u32,
    /// In first-seen order, without duplicates.
    pub new_words: Vec<String>,
    pub lessons_done: Vec<String>,
    /// Lessons finished per unit today (a unit has several short lessons, so
    /// a 30-minute plan can ask for two from the same unit).
    pub lessons_finished: BTreeMap<String, u32>,
    pub items_done: BTreeSet<String>,
    pub practice: Vec<PracticeTally>,
    pub xp: u32,
    pub koin_spent: u32,
}

/// Practice results on one (skill, topic), summed over a day.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PracticeTally {
    pub skill: Skill,
    pub topic: String,
    pub correct: u32,
    pub total: u32,
}

impl DayStats {
    pub fn minutes(&self) -> u32 {
        self.seconds.div_ceil(60)
    }

    /// Any learning at all, however small (a purchase doesn't count).
    pub fn studied(&self) -> bool {
        self.seconds > 0 || self.reviews > 0 || !self.items_done.is_empty() || !self.lessons_done.is_empty()
    }

    /// Today's goal: the plan's minutes reached, or every plan item done.
    pub fn goal_met(&self, goal_minutes: u16, plan_items: usize) -> bool {
        self.minutes() >= u32::from(goal_minutes) || (plan_items > 0 && self.items_done.len() >= plan_items)
    }

    fn add(&mut self, event: &ActivityEvent) {
        self.seconds += event.seconds;
        self.xp += xp(&event.kind);
        match &event.kind {
            ActivityKind::Reviews { cards, correct } => {
                self.reviews += cards;
                self.reviews_correct += correct.min(cards);
            }
            ActivityKind::NewWords { words } => {
                for w in words {
                    if !self.new_words.contains(w) {
                        self.new_words.push(w.clone());
                    }
                }
            }
            ActivityKind::LessonStep { unit_id, done } => {
                if *done {
                    if !self.lessons_done.contains(unit_id) {
                        self.lessons_done.push(unit_id.clone());
                    }
                    *self.lessons_finished.entry(unit_id.clone()).or_default() += 1;
                }
            }
            ActivityKind::Practice { skill, topic, correct, total } => {
                match self.practice.iter_mut().find(|p| p.skill == *skill && &p.topic == topic) {
                    Some(p) => {
                        p.correct += correct.min(total);
                        p.total += total;
                    }
                    None => self.practice.push(PracticeTally {
                        skill: *skill,
                        topic: topic.clone(),
                        correct: *correct.min(total),
                        total: *total,
                    }),
                }
            }
            ActivityKind::PlanItemDone { item_id } => {
                self.items_done.insert(item_id.clone());
            }
            ActivityKind::Purchase { koin, .. } => self.koin_spent += koin,
        }
    }
}

/// XP for one event. Effort counts, not only perfection: a wrong review still
/// earns something.
pub fn xp(kind: &ActivityKind) -> u32 {
    match kind {
        ActivityKind::Reviews { cards, correct } => cards + correct.min(cards),
        ActivityKind::NewWords { words } => 2 * words.len() as u32,
        ActivityKind::LessonStep { done, .. } => {
            if *done {
                10
            } else {
                2
            }
        }
        ActivityKind::Practice { correct, total, .. } => total + correct.min(total),
        ActivityKind::PlanItemDone { .. } => 5,
        ActivityKind::Purchase { .. } => 0,
    }
}

/// Every day that has events, oldest first. Duplicate event ids (a replayed
/// sync op) are counted once.
pub fn by_day(events: &[ActivityEvent]) -> BTreeMap<LocalDate, DayStats> {
    let mut seen = BTreeSet::new();
    let mut days: BTreeMap<LocalDate, DayStats> = BTreeMap::new();
    for e in events {
        if seen.insert(e.id.as_str()) {
            days.entry(e.date).or_default().add(e);
        }
    }
    days
}

pub fn day(events: &[ActivityEvent], date: LocalDate) -> DayStats {
    let only: Vec<ActivityEvent> = events.iter().filter(|e| e.date == date).cloned().collect();
    by_day(&only).remove(&date).unwrap_or_default()
}

/// The most recent day with any study before `before`.
pub fn last_study_day(events: &[ActivityEvent], before: LocalDate) -> Option<LocalDate> {
    by_day(events).into_iter().rev().find(|(d, s)| *d < before && s.studied()).map(|(d, _)| d)
}

#[cfg(test)]
pub(crate) mod tests {
    use kabar_protocol::{DeviceId, Hlc};

    use super::*;

    /// Test helper shared by the coach modules.
    pub fn event(id: &str, date: LocalDate, seconds: u32, kind: ActivityKind) -> ActivityEvent {
        ActivityEvent {
            id: id.into(),
            hlc: Hlc { wall_ms: 0, counter: 0, device: DeviceId("test".into()) },
            date,
            at_ms: (i64::from(date.0) * 86_400_000 + 18 * 3_600_000) as u64,
            kind,
            seconds,
        }
    }

    pub fn d(y: i32, m: u32, day: u32) -> LocalDate {
        LocalDate::from_ymd(y, m, day).unwrap()
    }

    #[test]
    fn folds_a_day_and_ignores_replays() {
        let today = d(2026, 10, 26);
        let words = ActivityKind::NewWords { words: vec!["membaca".into(), "buku".into()] };
        let events = vec![
            event("a", today, 300, ActivityKind::Reviews { cards: 10, correct: 8 }),
            event("b", today, 120, words.clone()),
            event("b", today, 120, words),
            event("c", today, 0, ActivityKind::PlanItemDone { item_id: "review".into() }),
            event("d", today.add_days(-1), 60, ActivityKind::Reviews { cards: 1, correct: 1 }),
        ];
        let s = day(&events, today);
        assert_eq!((s.seconds, s.reviews, s.reviews_correct), (420, 10, 8));
        assert_eq!(s.new_words, ["membaca", "buku"]);
        assert_eq!(s.minutes(), 7);
        assert_eq!(s.xp, 18 + 4 + 5);
        assert!(s.goal_met(5, 3));
        assert!(!s.goal_met(15, 3));
        assert!(s.goal_met(15, 1));
        assert_eq!(last_study_day(&events, today), Some(today.add_days(-1)));
    }

    #[test]
    fn a_purchase_is_not_study() {
        let today = d(2026, 10, 26);
        let s = day(&[event("p", today, 0, ActivityKind::Purchase { item_id: "x".into(), koin: 50 })], today);
        assert!(!s.studied());
        assert_eq!(s.koin_spent, 50);
    }
}
