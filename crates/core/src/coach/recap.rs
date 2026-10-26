//! The 30-second end-of-day recap: what I learned, one thing to remember,
//! tomorrow's first step.

use kabar_protocol::coach::{ActivityEvent, DayRecap, Focus, LocalDate, LocalTime, PlanItemKind};

use super::history::{self, DayStats};

/// From this time on the recap is offered to anyone who studied today.
pub const EVENING: LocalTime = LocalTime(21 * 60);
/// How long after the last study time the recap waits.
pub const AFTER_LAST_TIME_MIN: u16 = 60;

/// The recap for `date`. `tomorrow_first` is the first item of tomorrow's
/// plan, if one has been built.
pub fn day_recap(
    events: &[ActivityEvent],
    date: LocalDate,
    goal_minutes: u16,
    plan_items: usize,
    tomorrow_first: Option<PlanItemKind>,
) -> DayRecap {
    let stats = history::day(events, date);
    DayRecap {
        date,
        minutes: stats.minutes(),
        goal_met: stats.goal_met(goal_minutes, plan_items),
        items_done: stats.items_done.len() as u32,
        reviews: stats.reviews,
        remember: most_missed(&stats),
        new_words: stats.new_words,
        tomorrow: tomorrow_first,
    }
}

/// The practice topic with the most wrong answers; ties go to the topic
/// practised first. `None` on a day without mistakes.
fn most_missed(stats: &DayStats) -> Option<Focus> {
    let mut best: Option<(u32, Focus)> = None;
    for p in &stats.practice {
        let missed = p.total - p.correct;
        if missed > 0 && best.as_ref().is_none_or(|(m, _)| missed > *m) {
            best = Some((missed, Focus { skill: p.skill, topic: p.topic.clone() }));
        }
    }
    best.map(|(_, f)| f)
}

/// Whether to offer the recap now: after the goal is met, or in the evening
/// once the learner's last study time has passed.
///
/// Never before the learner studied today. Otherwise: as soon as the goal is
/// met; else an hour after the last preferred study time; else from 21:00.
pub fn recap_ready(goal_met: bool, studied: bool, now: LocalTime, study_times: &[LocalTime]) -> bool {
    if !studied {
        return false;
    }
    if goal_met || now >= EVENING {
        return true;
    }
    study_times
        .iter()
        .max()
        .is_some_and(|last| u32::from(now.0) >= u32::from(last.0) + u32::from(AFTER_LAST_TIME_MIN))
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::{ActivityKind, Skill};

    use super::*;
    use crate::coach::history::tests::{d, event};

    fn practice(skill: Skill, topic: &str, correct: u32, total: u32) -> ActivityKind {
        ActivityKind::Practice { skill, topic: topic.into(), correct, total }
    }

    #[test]
    fn recap_sums_the_day() {
        let today = d(2026, 10, 26);
        let events = vec![
            event("a", today, 240, ActivityKind::Reviews { cards: 20, correct: 17 }),
            event("b", today, 120, ActivityKind::NewWords { words: vec!["membaca".into(), "buku".into()] }),
            event("c", today, 180, practice(Skill::Writing, "-kan verbs", 2, 5)),
            event("d", today, 120, practice(Skill::Reading, "di- passive", 3, 5)),
            event("e", today, 60, practice(Skill::Writing, "-kan verbs", 1, 2)),
            event("f", today, 0, ActivityKind::PlanItemDone { item_id: "review".into() }),
            event("g", today, 0, ActivityKind::PlanItemDone { item_id: "lesson:bipa1.u03".into() }),
            event("y", today.add_days(-1), 900, ActivityKind::Reviews { cards: 50, correct: 50 }),
        ];
        let tomorrow = PlanItemKind::Review { cards: 25 };
        let r = day_recap(&events, today, 15, 4, Some(tomorrow.clone()));
        assert_eq!(r.date, today);
        assert_eq!(r.minutes, 12);
        assert!(!r.goal_met);
        assert_eq!((r.items_done, r.reviews), (2, 20));
        assert_eq!(r.new_words, ["membaca", "buku"]);
        assert_eq!(r.remember, Some(Focus { skill: Skill::Writing, topic: "-kan verbs".into() }));
        assert_eq!(r.tomorrow, Some(tomorrow));
        assert!(day_recap(&events, today, 10, 4, None).goal_met);
    }

    #[test]
    fn nothing_to_remember_without_mistakes() {
        let today = d(2026, 10, 26);
        let events = vec![event("a", today, 300, practice(Skill::Listening, "apa kabar", 4, 4))];
        let r = day_recap(&events, today, 5, 0, None);
        assert_eq!(r.remember, None);
        assert!(r.goal_met);
        let empty = day_recap(&[], today, 5, 3, None);
        assert_eq!((empty.minutes, empty.goal_met, empty.remember), (0, false, None));
    }

    #[test]
    fn ties_go_to_the_topic_practised_first() {
        let today = d(2026, 10, 26);
        let events = vec![
            event("a", today, 60, practice(Skill::Reading, "di- passive", 1, 3)),
            event("b", today, 60, practice(Skill::Writing, "-kan verbs", 3, 5)),
        ];
        assert_eq!(day_recap(&events, today, 15, 0, None).remember.unwrap().topic, "di- passive");
    }

    #[test]
    fn recap_waits_for_the_evening_or_the_goal() {
        let times = [LocalTime::hm(7, 30), LocalTime::hm(19, 0)];
        let at = LocalTime::hm;
        // Never before studying, even late.
        assert!(!recap_ready(false, false, at(23, 0), &times));
        // Goal met: right away.
        assert!(recap_ready(true, true, at(8, 0), &times));
        // Studied, goal not met: an hour after the last study time.
        assert!(!recap_ready(false, true, at(19, 59), &times));
        assert!(recap_ready(false, true, at(20, 0), &times));
        // A late study time: 21:00 comes first.
        assert!(!recap_ready(false, true, at(20, 59), &[at(22, 30)]));
        assert!(recap_ready(false, true, at(21, 0), &[at(22, 30)]));
        // No study times: only the evening.
        assert!(!recap_ready(false, true, at(18, 0), &[]));
        assert!(recap_ready(false, true, at(21, 30), &[]));
        // Order of study times doesn't matter.
        assert!(recap_ready(false, true, at(20, 0), &[at(19, 0), at(7, 30)]));
    }
}
