//! *Rencana Hari Ini*: today's plan from due reviews, the current unit and
//! weak spots, sized by "5, 15 or 30 minutes today?".
//!
//! Sizing (the session-length standard; sources in `docs/PEDAGOGY.md`):
//!
//! | Size           | Reviews             | Lessons       | Weak spot | Read/listen  | New words |
//! |----------------|---------------------|---------------|-----------|--------------|-----------|
//! | 5 min (Quick)  | ≤ 2 min, ≤ 12 cards | the next part | none      | if no lesson | ≤ 5       |
//! | 15 min (Steady)| ≤ 6 min (40%)       | 1 × ~6 min    | ~3 min    | what's left  | ≤ 12      |
//! | 30 min (Deep)  | ≤ 12 min (40%)      | 2 × ~6 min    | ~3 min    | ~3 min       | ≤ 20      |
//!
//! A 30-minute plan is two 15-minute blocks with a break offered between
//! them (see [`break_after`]). At most [`MAX_NEW_GRAMMAR_PER_DAY`] new
//! grammar points a day whatever the size. After a break (see
//! [`is_comeback`]) the plan is the light 5-minute one, whatever size was
//! asked, unless the learner picks a bigger one themselves that day.

use kabar_protocol::coach::{
    ActivityEvent, DailyPlan, Focus, LocalDate, PlanItem, PlanItemKind, PlanSize, Skill,
};

use super::history::{self, DayStats};

/// Days without study after which the plan becomes a short, light comeback.
pub const COMEBACK_AFTER_DAYS: i32 = 3;
/// Reviews a learner gets through in a minute, for sizing the review item.
pub const CARDS_PER_MINUTE: u32 = 6;
/// Reviews in a 5-minute or comeback plan: enough to warm up, never a wall
/// of cards.
pub const COMEBACK_MAX_CARDS: u32 = 12;
/// Most review minutes in a 5-minute or comeback plan.
pub const QUICK_REVIEW_MINUTES: u16 = 2;
/// Minutes planned for one lesson. Lessons are written to take at most
/// [`MAX_LESSON_MINUTES`], so a lesson item is one whole lesson.
pub const LESSON_MINUTES: u16 = 6;
/// The longest a lesson is estimated to take.
pub const MAX_LESSON_MINUTES: u16 = 7;
/// Minutes for the weak-spot item in 15- and 30-minute plans.
pub const WEAK_MINUTES: u16 = 3;
/// Minutes kept for a read or listen in a 30-minute plan.
pub const READING_MINUTES: u16 = 3;
/// New words a day, by plan size (a comeback day counts as Quick).
pub const NEW_WORDS_QUICK: u32 = 5;
pub const NEW_WORDS_STEADY: u32 = 12;
pub const NEW_WORDS_DEEP: u32 = 20;
/// New grammar points a day, whatever the plan size.
pub const MAX_NEW_GRAMMAR_PER_DAY: u32 = 2;
/// Practice answers on a topic before it can count as a weak spot.
pub const MIN_WEAK_ATTEMPTS: u32 = 3;
/// A topic answered at least this well is not a weak spot.
pub const WEAK_BELOW: f32 = 0.8;
/// The shortest lesson, weak-spot or reading item worth starting.
const MIN_ITEM_MINUTES: u16 = 3;
/// Neither block around a break may be shorter than this.
const MIN_BLOCK_MINUTES: u16 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanInputs<'a> {
    pub date: LocalDate,
    pub size: PlanSize,
    /// The learner picked `size` themselves today (not the profile's usual
    /// size). On a comeback day a picked size bigger than Quick is honoured;
    /// otherwise a comeback stays light.
    pub picked: bool,
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

/// Most new words to introduce today for a plan of this size. Whatever picks
/// new words (lessons, Kabar Pagi, the word deck) should stop at
/// [`new_words_left`].
pub fn new_words_cap(size: PlanSize, comeback: bool) -> u32 {
    match (size, comeback) {
        (PlanSize::Quick, _) | (_, true) => NEW_WORDS_QUICK,
        (PlanSize::Steady, false) => NEW_WORDS_STEADY,
        (PlanSize::Deep, false) => NEW_WORDS_DEEP,
    }
}

/// New words still welcome today under the plan's cap.
pub fn new_words_left(plan: &DailyPlan, today: &DayStats) -> u32 {
    new_words_cap(plan.size, plan.comeback).saturating_sub(today.new_words.len() as u32)
}

/// The stable plan item id for an item kind (for a lesson, the unit's first
/// lesson item; see [`lesson_part_id`] for the second).
pub fn item_id(kind: &PlanItemKind) -> String {
    match kind {
        PlanItemKind::Review { .. } => "review".into(),
        PlanItemKind::Lesson { unit_id } => lesson_part_id(unit_id, 1),
        PlanItemKind::WeakSpot { skill, topic } => format!("weak:{}:{topic}", skill_id(*skill)),
        PlanItemKind::Reading => "reading".into(),
    }
}

/// The id of the `part`-th lesson item (from 1) of one unit in a plan.
/// Part 1 keeps the plain `lesson:<unit>` id, so plans saved before 30-minute
/// plans had two lessons tick off as before; part 2 is `lesson:<unit>:2`.
pub fn lesson_part_id(unit_id: &str, part: u32) -> String {
    if part <= 1 { format!("lesson:{unit_id}") } else { format!("lesson:{unit_id}:{part}") }
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
    /// How many lesson items, and the most minutes each may take.
    lessons: u32,
    lesson_minutes: u16,
    /// `None`: no weak-spot drilling in this plan.
    weak_minutes: Option<u16>,
    /// Minutes kept back for a read at the end.
    reading_minutes: u16,
}

impl Budget {
    fn new(size: PlanSize, comeback: bool, has_lesson: bool, has_weak: bool) -> Self {
        let m = size.minutes();
        if comeback || size == PlanSize::Quick {
            // Short and light: a few reviews, then the next part of a lesson
            // (or a read when there is no lesson).
            return Self {
                minutes: m,
                review_cap: QUICK_REVIEW_MINUTES,
                max_cards: COMEBACK_MAX_CARDS,
                lessons: 1,
                lesson_minutes: m,
                weak_minutes: None,
                reading_minutes: 0,
            };
        }
        let (lessons, reading) = if size == PlanSize::Deep { (2, READING_MINUTES) } else { (1, 0) };
        let weak = if has_weak { WEAK_MINUTES } else { 0 };
        // Reviews get 40% next to lessons; without a lesson they may take the
        // lesson time too.
        let review_cap = if has_lesson { m * 2 / 5 } else { m - weak - reading };
        Self {
            minutes: m,
            review_cap,
            max_cards: u32::MAX,
            lessons,
            lesson_minutes: LESSON_MINUTES,
            weak_minutes: Some(WEAK_MINUTES),
            reading_minutes: reading,
        }
    }
}

/// Builds the plan. Deterministic: the same inputs give the same plan with
/// the same item ids on every device.
///
/// Order: due reviews, the current unit's lesson, the top weak spot, then a
/// short read with whatever time is left. A 30-minute plan puts the weak spot
/// right after the reviews, so its first block is practice and its second
/// (after the break) is new learning: two lessons and a read. Item minutes
/// always add up to the size's minutes. After a break (see [`is_comeback`])
/// the plan is the light Quick one whatever was asked, unless the learner
/// picked a bigger size today.
pub fn build_plan(inputs: &PlanInputs) -> DailyPlan {
    let comeback =
        is_comeback(inputs.date, inputs.last_study_day) && !(inputs.picked && inputs.size != PlanSize::Quick);
    let size = if comeback { PlanSize::Quick } else { inputs.size };
    let weak = inputs.weak_spots.first();
    let budget = Budget::new(size, comeback, inputs.current_unit.is_some(), weak.is_some());

    let mut items: Vec<PlanItem> = Vec::new();
    let mut left = budget.minutes;
    let add_weak = |items: &mut Vec<PlanItem>, left: &mut u16| {
        if let (Some(focus), Some(weak_minutes)) = (weak, budget.weak_minutes)
            && *left >= MIN_ITEM_MINUTES
        {
            let minutes = (*left).min(weak_minutes.max(MIN_ITEM_MINUTES));
            let kind = PlanItemKind::WeakSpot { skill: focus.skill, topic: focus.topic.clone() };
            items.push(PlanItem { id: item_id(&kind), kind, minutes });
            *left -= minutes;
        }
    };

    if inputs.due_reviews > 0 && budget.review_cap > 0 {
        let cards = inputs.due_reviews.min(budget.max_cards);
        let wanted = cards.div_ceil(CARDS_PER_MINUTE).min(u32::from(budget.review_cap)) as u16;
        let cards = cards.min(u32::from(wanted) * CARDS_PER_MINUTE);
        let kind = PlanItemKind::Review { cards };
        items.push(PlanItem { id: item_id(&kind), kind, minutes: wanted });
        left -= wanted;
    }
    if size == PlanSize::Deep {
        add_weak(&mut items, &mut left);
    }
    if let Some(unit) = inputs.current_unit {
        for part in 1..=budget.lessons {
            // A second lesson leaves room for the read.
            let keep = if part == 1 { 0 } else { budget.reading_minutes };
            if left < MIN_ITEM_MINUTES + keep {
                break;
            }
            let minutes = (left - keep).min(budget.lesson_minutes.max(MIN_ITEM_MINUTES));
            let kind = PlanItemKind::Lesson { unit_id: unit.to_owned() };
            items.push(PlanItem { id: lesson_part_id(unit, part), kind, minutes });
            left -= minutes;
        }
    }
    if size != PlanSize::Deep {
        add_weak(&mut items, &mut left);
    }
    if left >= MIN_ITEM_MINUTES || (left > 0 && items.is_empty()) {
        let kind = PlanItemKind::Reading;
        items.push(PlanItem { id: item_id(&kind), kind, minutes: left });
        left = 0;
    }
    if left > 0 {
        // Too little for its own item: the lesson (or else the first item) gets it.
        let i = items.iter().position(|it| matches!(it.kind, PlanItemKind::Lesson { .. })).unwrap_or(0);
        items[i].minutes += left;
    }

    DailyPlan { date: inputs.date, size, comeback, items }
}

/// Where a 30-minute plan offers a break: after the item at this index, the
/// boundary closest to halfway (ties go to the earlier one). `None` for
/// shorter plans, comebacks, and when either block would be under five
/// minutes.
///
/// Compatibility: derived from the plan, not stored in it, so
/// [`DailyPlan`] (synced and serde'd in `kabar_protocol::coach`) keeps its
/// shape. Older clients read and write the same plans and simply don't offer
/// the break; plans saved before this existed get one too.
pub fn break_after(plan: &DailyPlan) -> Option<usize> {
    if plan.size != PlanSize::Deep || plan.comeback || plan.items.len() < 2 {
        return None;
    }
    let total: u16 = plan.items.iter().map(|i| i.minutes).sum();
    let half = total / 2;
    let mut cum = 0u16;
    let mut best: Option<(usize, u16)> = None;
    for (i, item) in plan.items[..plan.items.len() - 1].iter().enumerate() {
        cum += item.minutes;
        if cum < MIN_BLOCK_MINUTES || total - cum < MIN_BLOCK_MINUTES {
            continue;
        }
        let gap = cum.abs_diff(half);
        if best.is_none_or(|(_, g)| gap < g) {
            best = Some((i, gap));
        }
    }
    best.map(|(i, _)| i)
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
///
/// The n-th lesson item of a unit is done once n lessons of that unit were
/// finished today.
pub fn progress(plan: &DailyPlan, today: &DayStats) -> PlanProgress {
    let done: Vec<bool> = plan
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            today.items_done.contains(&item.id)
                || match &item.kind {
                    PlanItemKind::Review { cards } => today.reviews >= *cards,
                    PlanItemKind::Lesson { unit_id } => {
                        let part = plan.items[..=i]
                            .iter()
                            .filter(
                                |it| matches!(&it.kind, PlanItemKind::Lesson { unit_id: u } if u == unit_id),
                            )
                            .count() as u32;
                        today.lessons_finished.get(unit_id).copied().unwrap_or(0) >= part
                    }
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
            picked: false,
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
    fn quick_is_a_few_reviews_then_the_next_lesson_part() {
        let weak = [kan()];
        // At most 2 minutes and 12 cards of reviews, whatever is due.
        let plan = build_plan(&inputs(PlanSize::Quick, 40, Some("bipa1.u03"), &weak));
        assert_eq!(ids(&plan), ["review", "lesson:bipa1.u03"]);
        assert_eq!(plan.items[0].kind, PlanItemKind::Review { cards: COMEBACK_MAX_CARDS });
        assert_eq!(plan.items.iter().map(|i| i.minutes).collect::<Vec<_>>(), [2, 3]);

        let plan = build_plan(&inputs(PlanSize::Quick, 0, Some("bipa1.u03"), &weak));
        assert_eq!(ids(&plan), ["lesson:bipa1.u03"]);
        assert_eq!(total(&plan), 5);

        let plan = build_plan(&inputs(PlanSize::Quick, 6, Some("bipa1.u03"), &weak));
        assert_eq!(ids(&plan), ["review", "lesson:bipa1.u03"]);
        assert_eq!(plan.items.iter().map(|i| i.minutes).collect::<Vec<_>>(), [1, 4]);

        // No lesson: the rest is a short read, never more reviews or a weak spot.
        let plan = build_plan(&inputs(PlanSize::Quick, 20, None, &weak));
        assert_eq!(ids(&plan), ["review", "reading"]);
        assert_eq!(plan.items[0].kind, PlanItemKind::Review { cards: 12 });
        assert_eq!(plan.items.iter().map(|i| i.minutes).collect::<Vec<_>>(), [2, 3]);
    }

    #[test]
    fn deep_is_two_blocks_with_two_lessons_and_a_read() {
        let weak = [kan()];
        let plan = build_plan(&inputs(PlanSize::Deep, 500, Some("bipa1.u03"), &weak));
        assert_eq!(
            ids(&plan),
            ["review", "weak:writing:-kan verbs", "lesson:bipa1.u03", "lesson:bipa1.u03:2", "reading"]
        );
        assert_eq!(plan.items.iter().map(|i| i.minutes).collect::<Vec<_>>(), [12, 3, 6, 6, 3]);
        for size in PlanSize::ALL {
            let plan = build_plan(&inputs(size, 3, Some("bipa1.u03"), &weak));
            let lessons = plan.items.iter().filter(|i| matches!(i.kind, PlanItemKind::Lesson { .. }));
            assert!(lessons.clone().all(|i| i.minutes <= MAX_LESSON_MINUTES), "{size:?}: whole lessons");
            assert_eq!(lessons.count(), if size == PlanSize::Deep { 2 } else { 1 });
        }
        // Practice block, a break, then the learning block: 15 + 15.
        assert_eq!(break_after(&plan), Some(1));

        // Few reviews: the break moves to the boundary nearest halfway.
        let plan = build_plan(&inputs(PlanSize::Deep, 4, Some("bipa1.u03"), &weak));
        assert_eq!(plan.items.iter().map(|i| i.minutes).collect::<Vec<_>>(), [1, 3, 6, 6, 14]);
        assert_eq!(break_after(&plan), Some(3));

        // No lesson: reviews take its time but the read keeps its 3 minutes.
        let plan = build_plan(&inputs(PlanSize::Deep, 500, None, &weak));
        assert_eq!(ids(&plan), ["review", "weak:writing:-kan verbs", "reading"]);
        assert_eq!(plan.items.iter().map(|i| i.minutes).collect::<Vec<_>>(), [24, 3, 3]);
    }

    #[test]
    fn only_deep_plans_offer_a_break() {
        let weak = [kan()];
        for size in [PlanSize::Quick, PlanSize::Steady] {
            assert_eq!(break_after(&build_plan(&inputs(size, 500, Some("bipa1.u03"), &weak))), None);
        }
        // One long item: nothing to break between.
        assert_eq!(break_after(&build_plan(&inputs(PlanSize::Deep, 0, None, &[]))), None);
        let mut i = inputs(PlanSize::Deep, 500, Some("bipa1.u03"), &weak);
        i.last_study_day = Some(i.date.add_days(-10));
        assert_eq!(break_after(&build_plan(&i)), None, "a comeback is one short block");
    }

    #[test]
    fn two_lessons_of_one_unit_tick_off_one_by_one() {
        let plan = build_plan(&inputs(PlanSize::Deep, 0, Some("bipa1.u03"), &[]));
        assert_eq!(ids(&plan), ["lesson:bipa1.u03", "lesson:bipa1.u03:2", "reading"]);
        assert_eq!(lesson_part_id("bipa1.u03", 1), item_id(&plan.items[0].kind), "old ids stay stable");
        let today = plan.date;
        let lesson = |id: &str| {
            event(id, today, 360, ActivityKind::LessonStep { unit_id: "bipa1.u03".into(), done: true })
        };
        let mut events = vec![lesson("a")];
        assert_eq!(progress(&plan, &history::day(&events, today)).done, [true, false, false]);
        events.push(lesson("a")); // a replay counts once
        assert_eq!(progress(&plan, &history::day(&events, today)).done, [true, false, false]);
        events.push(lesson("b"));
        assert_eq!(progress(&plan, &history::day(&events, today)).done, [true, true, false]);
    }

    #[test]
    fn new_word_caps_follow_the_size() {
        assert_eq!(new_words_cap(PlanSize::Quick, false), 5);
        assert_eq!(new_words_cap(PlanSize::Steady, false), 12);
        assert_eq!(new_words_cap(PlanSize::Deep, false), 20);
        for size in PlanSize::ALL {
            assert_eq!(new_words_cap(size, true), NEW_WORDS_QUICK, "a comeback day stays light");
        }
        assert_eq!(MAX_NEW_GRAMMAR_PER_DAY, 2);

        let plan = build_plan(&inputs(PlanSize::Steady, 0, Some("bipa1.u03"), &[]));
        let today = plan.date;
        let words: Vec<String> = (0..10).map(|n| format!("kata{n}")).collect();
        let events = vec![event("w", today, 0, ActivityKind::NewWords { words })];
        assert_eq!(new_words_left(&plan, &history::day(&events, today)), 2);
        let light = DailyPlan { comeback: true, size: PlanSize::Quick, ..plan };
        assert_eq!(new_words_left(&light, &history::day(&events, today)), 0);
    }

    #[test]
    fn a_bigger_size_picked_on_a_comeback_day_is_honoured() {
        let weak = [kan()];
        let mut i = inputs(PlanSize::Steady, 100, Some("bipa1.u03"), &weak);
        i.last_study_day = Some(i.date.add_days(-10));
        i.picked = true;
        let plan = build_plan(&i);
        assert!(!plan.comeback);
        assert_eq!(plan.size, PlanSize::Steady);
        assert_eq!(total(&plan), 15);
        // Picking 5 minutes keeps the light welcome-back plan.
        i.size = PlanSize::Quick;
        let plan = build_plan(&i);
        assert!(plan.comeback);
        assert_eq!(plan.size, PlanSize::Quick);
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
