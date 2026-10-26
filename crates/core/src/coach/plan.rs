//! *Rencana Hari Ini*: today's plan from due reviews, the current unit and
//! weak spots, sized by "5, 15 or 30 minutes today?".

use kabar_protocol::coach::{
    ActivityEvent, DailyPlan, Focus, LocalDate, PlanItem, PlanItemKind, PlanSize, Skill,
};

use super::history::{self, DayStats};

/// Days without study after which the plan becomes a short, light comeback.
pub const COMEBACK_AFTER_DAYS: i32 = 3;
/// Reviews a learner gets through in a minute, for sizing the review item.
pub const CARDS_PER_MINUTE: u32 = 6;
/// Reviews in a comeback plan: enough to warm up, never a wall of cards.
pub const COMEBACK_MAX_CARDS: u32 = 12;
/// Practice answers on a topic before it can count as a weak spot.
pub const MIN_WEAK_ATTEMPTS: u32 = 3;
/// A topic answered at least this well is not a weak spot.
pub const WEAK_BELOW: f32 = 0.8;
/// The shortest lesson, weak-spot or reading item worth starting.
const MIN_ITEM_MINUTES: u16 = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanInputs<'a> {
    pub date: LocalDate,
    pub size: PlanSize,
    /// Cards due today (from the review engine).
    pub due_reviews: u32,
    /// The unit the learner is on, if any (from the Journey).
    pub current_unit: Option<&'a str>,
    /// Weak spots, most urgent first (see [`weak_spots`]).
    pub weak_spots: &'a [Focus],
    /// The last day with any study before `date`.
    pub last_study_day: Option<LocalDate>,
}

/// Whether `date` comes after a break long enough for a welcome-back plan.
/// The very first day (no study yet) is not a comeback.
pub fn is_comeback(date: LocalDate, last_study_day: Option<LocalDate>) -> bool {
    last_study_day.is_some_and(|last| date.days_since(last) > COMEBACK_AFTER_DAYS)
}

/// The stable plan item id for an item kind.
pub fn item_id(kind: &PlanItemKind) -> String {
    match kind {
        PlanItemKind::Review { .. } => "review".into(),
        PlanItemKind::Lesson { unit_id } => format!("lesson:{unit_id}"),
        PlanItemKind::WeakSpot { skill, topic } => format!("weak:{}:{topic}", skill_id(*skill)),
        PlanItemKind::Reading => "reading".into(),
    }
}

/// The skill's wire name (matches its serde form).
pub fn skill_id(skill: Skill) -> &'static str {
    match skill {
        Skill::Reading => "reading",
        Skill::Listening => "listening",
        Skill::Writing => "writing",
        Skill::Speaking => "speaking",
        Skill::Vocabulary => "vocabulary",
    }
}

/// How the minutes get shared out.
struct Budget {
    minutes: u16,
    /// Most minutes reviews may take.
    review_cap: u16,
    max_cards: u32,
    lesson_minutes: u16,
    /// `None`: no weak-spot drilling in this plan.
    weak_minutes: Option<u16>,
}

impl Budget {
    fn new(size: PlanSize, comeback: bool, has_lesson: bool, has_weak: bool) -> Self {
        let m = size.minutes();
        if comeback {
            // Short and light: a few reviews, then a small lesson bit or a read.
            let review_cap = COMEBACK_MAX_CARDS.div_ceil(CARDS_PER_MINUTE) as u16;
            return Self {
                minutes: m,
                review_cap,
                max_cards: COMEBACK_MAX_CARDS,
                lesson_minutes: m,
                weak_minutes: None,
            };
        }
        if size == PlanSize::Quick {
            // Reviews first; whatever they leave goes to a lesson bit.
            return Self {
                minutes: m,
                review_cap: m,
                max_cards: u32::MAX,
                lesson_minutes: m,
                weak_minutes: None,
            };
        }
        let weak = m / 5;
        let review_cap = if has_lesson {
            m * 2 / 5
        } else if has_weak {
            m - weak
        } else {
            m
        };
        Self {
            minutes: m,
            review_cap,
            max_cards: u32::MAX,
            lesson_minutes: m * 2 / 5,
            weak_minutes: Some(weak),
        }
    }
}

/// Builds the plan. Deterministic: the same inputs give the same plan with
/// the same item ids on every device.
///
/// Order: due reviews, the current unit's lesson, the top weak spot, then a
/// short read with whatever time is left. Item minutes always add up to the
/// size's minutes. After a break (see [`is_comeback`]) the plan is the Quick
/// size whatever was asked, with few reviews and no weak-spot drilling.
pub fn build_plan(inputs: &PlanInputs) -> DailyPlan {
    let comeback = is_comeback(inputs.date, inputs.last_study_day);
    let size = if comeback { PlanSize::Quick } else { inputs.size };
    let weak = inputs.weak_spots.first();
    let budget = Budget::new(size, comeback, inputs.current_unit.is_some(), weak.is_some());

    let mut items: Vec<PlanItem> = Vec::new();
    let mut left = budget.minutes;
    let push = |items: &mut Vec<PlanItem>, kind: PlanItemKind, minutes: u16| {
        items.push(PlanItem { id: item_id(&kind), kind, minutes });
    };

    if inputs.due_reviews > 0 && budget.review_cap > 0 {
        let cards = inputs.due_reviews.min(budget.max_cards);
        let wanted = cards.div_ceil(CARDS_PER_MINUTE).min(u32::from(budget.review_cap)) as u16;
        let cards = cards.min(u32::from(wanted) * CARDS_PER_MINUTE);
        push(&mut items, PlanItemKind::Review { cards }, wanted);
        left -= wanted;
    }
    if let Some(unit) = inputs.current_unit
        && left >= MIN_ITEM_MINUTES
    {
        let minutes = left.min(budget.lesson_minutes.max(MIN_ITEM_MINUTES));
        push(&mut items, PlanItemKind::Lesson { unit_id: unit.to_owned() }, minutes);
        left -= minutes;
    }
    if let (Some(focus), Some(weak_minutes)) = (weak, budget.weak_minutes)
        && left >= MIN_ITEM_MINUTES
    {
        let minutes = left.min(weak_minutes.max(MIN_ITEM_MINUTES));
        let kind = PlanItemKind::WeakSpot { skill: focus.skill, topic: focus.topic.clone() };
        push(&mut items, kind, minutes);
        left -= minutes;
    }
    if left >= MIN_ITEM_MINUTES || (left > 0 && items.is_empty()) {
        push(&mut items, PlanItemKind::Reading, left);
        left = 0;
    }
    if left > 0 {
        // Too little for its own item: the lesson (or else the first item) gets it.
        let i = items.iter().position(|it| matches!(it.kind, PlanItemKind::Lesson { .. })).unwrap_or(0);
        items[i].minutes += left;
    }

    DailyPlan { date: inputs.date, size, comeback, items }
}

/// Topics answered worst over the `window_days` before `today`, worst first.
///
/// A topic needs at least [`MIN_WEAK_ATTEMPTS`] answers and an accuracy below
/// [`WEAK_BELOW`]. Ties keep the order the topics were first practised in.
pub fn weak_spots(events: &[ActivityEvent], today: LocalDate, window_days: i32) -> Vec<Focus> {
    weak_spots_between(events, today.add_days(-window_days.max(0)), today)
}

/// Weak spots from practice on days in `from..until`.
pub(crate) fn weak_spots_between(events: &[ActivityEvent], from: LocalDate, until: LocalDate) -> Vec<Focus> {
    let mut tallies: Vec<(Skill, String, u32, u32)> = Vec::new();
    for (_, stats) in history::by_day(events).range(from..until) {
        for p in &stats.practice {
            match tallies.iter_mut().find(|t| t.0 == p.skill && t.1 == p.topic) {
                Some(t) => {
                    t.2 += p.correct;
                    t.3 += p.total;
                }
                None => tallies.push((p.skill, p.topic.clone(), p.correct, p.total)),
            }
        }
    }
    tallies.retain(|&(_, _, correct, total)| {
        total >= MIN_WEAK_ATTEMPTS && (correct as f32) < WEAK_BELOW * total as f32
    });
    // Compare correct/total without floats: a/b < c/d  ⇔  a·d < c·b.
    tallies.sort_by(|a, b| (u64::from(a.2) * u64::from(b.3)).cmp(&(u64::from(b.2) * u64::from(a.3))));
    tallies.into_iter().map(|(skill, topic, _, _)| Focus { skill, topic }).collect()
}

/// How far today's plan has come.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanProgress {
    /// One flag per plan item, in order.
    pub done: Vec<bool>,
    pub minutes_done: u32,
    pub goal_met: bool,
}

/// An item counts as done when it was ticked off, or when the work it asks
/// for is visibly done (enough reviews, the lesson finished).
pub fn progress(plan: &DailyPlan, today: &DayStats) -> PlanProgress {
    let done: Vec<bool> = plan
        .items
        .iter()
        .map(|item| {
            today.items_done.contains(&item.id)
                || match &item.kind {
                    PlanItemKind::Review { cards } => today.reviews >= *cards,
                    PlanItemKind::Lesson { unit_id } => today.lessons_done.contains(unit_id),
                    PlanItemKind::WeakSpot { .. } | PlanItemKind::Reading => false,
                }
        })
        .collect();
    let all_done = !done.is_empty() && done.iter().all(|&d| d);
    PlanProgress {
        minutes_done: today.minutes(),
        goal_met: all_done || today.goal_met(plan.size.minutes(), plan.items.len()),
        done,
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::ActivityKind;

    use super::*;
    use crate::coach::history::tests::{d, event};

    fn kan() -> Focus {
        Focus { skill: Skill::Writing, topic: "-kan verbs".into() }
    }

    fn inputs<'a>(size: PlanSize, due: u32, unit: Option<&'a str>, weak: &'a [Focus]) -> PlanInputs<'a> {
        let date = d(2026, 10, 26);
        PlanInputs {
            date,
            size,
            due_reviews: due,
            current_unit: unit,
            weak_spots: weak,
            last_study_day: Some(date.add_days(-1)),
        }
    }

    fn total(plan: &DailyPlan) -> u16 {
        plan.items.iter().map(|i| i.minutes).sum()
    }

    fn ids(plan: &DailyPlan) -> Vec<&str> {
        plan.items.iter().map(|i| i.id.as_str()).collect()
    }

    #[test]
    fn steady_plan_has_every_part_in_order() {
        let weak = [kan()];
        let plan = build_plan(&inputs(PlanSize::Steady, 10, Some("bipa1.u03"), &weak));
        assert_eq!(ids(&plan), ["review", "lesson:bipa1.u03", "weak:writing:-kan verbs", "reading"]);
        assert_eq!(plan.items[0].kind, PlanItemKind::Review { cards: 10 });
        assert_eq!(plan.items.iter().map(|i| i.minutes).collect::<Vec<_>>(), [2, 6, 3, 4]);
        assert_eq!(total(&plan), 15);
        assert!(!plan.comeback);
        assert_eq!(plan.size, PlanSize::Steady);
    }

    #[test]
    fn reviews_leave_room_for_the_lesson() {
        let weak = [kan()];
        for (size, cap) in [(PlanSize::Steady, 6), (PlanSize::Deep, 12)] {
            let plan = build_plan(&inputs(size, 500, Some("bipa1.u03"), &weak));
            assert_eq!(plan.items[0].minutes, cap);
            assert_eq!(plan.items[0].kind, PlanItemKind::Review { cards: u32::from(cap) * 6 });
            assert!(ids(&plan).contains(&"lesson:bipa1.u03"));
            assert!(ids(&plan).contains(&"weak:writing:-kan verbs"));
            assert_eq!(total(&plan), size.minutes());
        }
    }

    #[test]
    fn without_a_lesson_reviews_may_fill_the_plan() {
        let plan = build_plan(&inputs(PlanSize::Steady, 500, None, &[]));
        assert_eq!(ids(&plan), ["review"]);
        assert_eq!(plan.items[0].kind, PlanItemKind::Review { cards: 90 });
        assert_eq!(total(&plan), 15);
        // A weak spot still keeps its slot.
        let weak = [kan()];
        let plan = build_plan(&inputs(PlanSize::Steady, 500, None, &weak));
        assert_eq!(ids(&plan), ["review", "weak:writing:-kan verbs"]);
        assert_eq!(total(&plan), 15);
    }

    #[test]
    fn quick_is_reviews_or_a_lesson_bit() {
        let weak = [kan()];
        let plan = build_plan(&inputs(PlanSize::Quick, 40, Some("bipa1.u03"), &weak));
        assert_eq!(ids(&plan), ["review"]);
        assert_eq!((plan.items[0].minutes, &plan.items[0].kind), (5, &PlanItemKind::Review { cards: 30 }));

        let plan = build_plan(&inputs(PlanSize::Quick, 0, Some("bipa1.u03"), &weak));
        assert_eq!(ids(&plan), ["lesson:bipa1.u03"]);
        assert_eq!(total(&plan), 5);

        // A couple of cards leave time for a lesson bit; a few more just fold in.
        let plan = build_plan(&inputs(PlanSize::Quick, 6, Some("bipa1.u03"), &weak));
        assert_eq!(ids(&plan), ["review", "lesson:bipa1.u03"]);
        assert_eq!(plan.items.iter().map(|i| i.minutes).collect::<Vec<_>>(), [1, 4]);
        let plan = build_plan(&inputs(PlanSize::Quick, 20, None, &weak));
        assert_eq!(ids(&plan), ["review"]);
        assert_eq!((plan.items[0].minutes, &plan.items[0].kind), (5, &PlanItemKind::Review { cards: 20 }));
    }

    #[test]
    fn with_nothing_due_there_is_still_something_to_read() {
        for size in PlanSize::ALL {
            let plan = build_plan(&inputs(size, 0, None, &[]));
            assert_eq!(ids(&plan), ["reading"]);
            assert_eq!(total(&plan), size.minutes());
        }
    }

    #[test]
    fn a_break_brings_a_light_comeback() {
        let weak = [kan()];
        let mut i = inputs(PlanSize::Deep, 200, Some("bipa1.u03"), &weak);
        i.last_study_day = Some(i.date.add_days(-10));
        let plan = build_plan(&i);
        assert!(plan.comeback);
        assert_eq!(plan.size, PlanSize::Quick);
        assert_eq!(ids(&plan), ["review", "lesson:bipa1.u03"]);
        assert_eq!(plan.items[0].kind, PlanItemKind::Review { cards: COMEBACK_MAX_CARDS });
        assert_eq!(total(&plan), 5);
    }

    #[test]
    fn comeback_threshold_and_first_day() {
        let today = d(2026, 10, 26);
        assert!(!is_comeback(today, None));
        assert!(!is_comeback(today, Some(today.add_days(-1))));
        assert!(!is_comeback(today, Some(today.add_days(-COMEBACK_AFTER_DAYS))));
        assert!(is_comeback(today, Some(today.add_days(-COMEBACK_AFTER_DAYS - 1))));
        let mut i = inputs(PlanSize::Deep, 0, Some("bipa1.u01"), &[]);
        i.last_study_day = None;
        let plan = build_plan(&i);
        assert!(!plan.comeback);
        assert_eq!(total(&plan), 30);
    }

    #[test]
    fn plans_are_deterministic() {
        let weak = [kan(), Focus { skill: Skill::Reading, topic: "di- passive".into() }];
        for size in PlanSize::ALL {
            for due in [0, 1, 7, 13, 31, 77, 1000] {
                let i = inputs(size, due, Some("bipa2.u04"), &weak);
                let plan = build_plan(&i);
                assert_eq!(plan, build_plan(&i));
                assert_eq!(total(&plan), size.minutes(), "{size:?} {due}");
                assert!(plan.items.iter().all(|it| it.minutes > 0));
                assert!(!ids(&plan).contains(&"weak:reading:di- passive"), "only the top weak spot");
            }
        }
    }

    #[test]
    fn weak_spots_rank_the_worst_topics() {
        let today = d(2026, 10, 26);
        let practice = |skill, topic: &str, correct, total| ActivityKind::Practice {
            skill,
            topic: topic.into(),
            correct,
            total,
        };
        let events = vec![
            event("a", today.add_days(-2), 60, practice(Skill::Writing, "-kan verbs", 1, 4)),
            event("b", today.add_days(-1), 60, practice(Skill::Writing, "-kan verbs", 1, 2)),
            event("c", today.add_days(-1), 60, practice(Skill::Reading, "di- passive", 2, 5)),
            // Same accuracy as the passive, practised later: stays after it.
            event("d", today.add_days(-1), 60, practice(Skill::Listening, "numbers", 4, 10)),
            // Too few answers.
            event("e", today.add_days(-1), 60, practice(Skill::Speaking, "apa kabar", 0, 2)),
            // Strong enough.
            event("f", today.add_days(-1), 60, practice(Skill::Vocabulary, "buku", 9, 10)),
            // Outside the window, and today.
            event("g", today.add_days(-9), 60, practice(Skill::Writing, "ber- verbs", 0, 5)),
            event("h", today, 60, practice(Skill::Writing, "me- verbs", 0, 5)),
            // A replayed event counts once.
            event("a", today.add_days(-2), 60, practice(Skill::Writing, "-kan verbs", 1, 4)),
        ];
        let spots = weak_spots(&events, today, 7);
        let names: Vec<&str> = spots.iter().map(|f| f.topic.as_str()).collect();
        assert_eq!(names, ["-kan verbs", "di- passive", "numbers"]);
        assert_eq!(spots[0], kan());
        assert!(weak_spots(&events, today, 0).is_empty());
    }

    #[test]
    fn progress_follows_ticks_and_real_work() {
        let weak = [kan()];
        let plan = build_plan(&inputs(PlanSize::Steady, 10, Some("bipa1.u03"), &weak));
        let today = plan.date;
        let mut events = vec![event("r", today, 120, ActivityKind::Reviews { cards: 10, correct: 7 })];
        let p = progress(&plan, &history::day(&events, today));
        assert_eq!(p.done, [true, false, false, false]);
        assert_eq!(p.minutes_done, 2);
        assert!(!p.goal_met);

        events.push(event(
            "l",
            today,
            300,
            ActivityKind::LessonStep { unit_id: "bipa1.u03".into(), done: true },
        ));
        events.push(event(
            "w",
            today,
            0,
            ActivityKind::PlanItemDone { item_id: "weak:writing:-kan verbs".into() },
        ));
        let p = progress(&plan, &history::day(&events, today));
        assert_eq!(p.done, [true, true, true, false]);
        assert!(!p.goal_met);

        events.push(event("x", today, 0, ActivityKind::PlanItemDone { item_id: "reading".into() }));
        let p = progress(&plan, &history::day(&events, today));
        assert!(p.done.iter().all(|&d| d));
        assert!(p.goal_met, "every item done meets the goal even short of 15 minutes");

        let long = [event("t", today, 15 * 60, ActivityKind::Reviews { cards: 3, correct: 3 })];
        let p = progress(&plan, &history::day(&long, today));
        assert!(p.goal_met && !p.done[0]);
    }
}
