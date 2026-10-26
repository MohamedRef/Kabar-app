//! The monthly level check's pure parts (SPEC §6): the 10-minute mini check
//! cut from the current level's test, when the check is due, the skill radar,
//! and the inputs to Kak Nur's decision.
//!
//! The decision itself is the coach's ([`crate::coach::level::decide`]): up on
//! a passed *full* level test, down only on a clear miss or sustained
//! struggle, one level at a time. Following the `level_check` golden rule,
//! the mini check feeds the radar and the evidence, not the decision; a
//! strong mini check invites the full test ("Take a flight") instead.

use std::collections::BTreeMap;

use kabar_protocol::coach::{ActivityEvent, ActivityKind, LocalDate, Skill};
use kabar_protocol::level_check::{LevelMove, RadarPoint};
use kabar_protocol::test::{TestContent, TestItemKind, TestKind, TestSection, TestSkill};

use crate::coach::history;
use crate::coach::level::{CheckStats, LevelTest, Moved};
use crate::coach::report::MIN_SKILL_ATTEMPTS;
use crate::journey::{Curriculum, Progress};

/// How long the mini check aims to take.
pub const CHECK_MINUTES: f32 = 10.0;
/// A check is due this many days after the last one...
pub const CHECK_EVERY_DAYS: i32 = 30;
/// ...or, before the first one, this many days after the first study day.
pub const FIRST_CHECK_AFTER_DAYS: i32 = 28;
/// What the check looks back over (reviews, practice, active days).
pub const LOOKBACK_DAYS: i32 = 30;
/// A full level test this recent counts in the decision.
pub const LEVEL_TEST_FRESH_DAYS: i32 = 30;
/// Estimated minutes per item, for cutting the mini check to size.
pub const CLOSED_ITEM_MINUTES: f32 = 0.5;
pub const WRITING_TASK_MINUTES: f32 = 3.0;
pub const SPEAKING_TASK_MINUTES: f32 = 2.0;
/// Weight of the mini check against recent practice on a radar axis.
pub const CHECK_WEIGHT: f32 = 0.6;

fn minutes_of(kind: &TestItemKind) -> f32 {
    match kind {
        TestItemKind::Exercise(_) => CLOSED_ITEM_MINUTES,
        TestItemKind::WritingTask(_) => WRITING_TASK_MINUTES,
        TestItemKind::SpeakingTask(_) => SPEAKING_TASK_MINUTES,
    }
}

/// A different, stable selection each month.
pub fn month_seed(date: LocalDate) -> u64 {
    let (y, m, _) = date.ymd();
    (y as u64) * 12 + u64::from(m)
}

/// The mini check: items taken round-robin across the sections (so every
/// skill the test measures is in it), each section starting at a
/// month-dependent item, until about [`CHECK_MINUTES`]. At most one open task
/// of each kind. Items keep their test order and points; the pass mark is
/// the full test's.
pub fn mini_check(test: &TestContent, seed: u64) -> TestContent {
    let n = test.sections.len();
    let mut picked: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut cursor: Vec<usize> = test
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| if s.items.is_empty() { 0 } else { (seed as usize + i * 7) % s.items.len() })
        .collect();
    let mut tried = vec![0usize; n];
    let (mut minutes, mut writing, mut speaking) = (0.0, false, false);
    loop {
        let mut progressed = false;
        for (si, s) in test.sections.iter().enumerate() {
            while tried[si] < s.items.len() {
                let ii = cursor[si];
                cursor[si] = (ii + 1) % s.items.len();
                tried[si] += 1;
                let kind = &s.items[ii].kind;
                let open_taken = match kind {
                    TestItemKind::WritingTask(_) => writing,
                    TestItemKind::SpeakingTask(_) => speaking,
                    TestItemKind::Exercise(_) => false,
                };
                let cost = minutes_of(kind);
                if open_taken || minutes + cost > CHECK_MINUTES {
                    continue;
                }
                minutes += cost;
                writing |= matches!(kind, TestItemKind::WritingTask(_));
                speaking |= matches!(kind, TestItemKind::SpeakingTask(_));
                picked[si].push(ii);
                progressed = true;
                break;
            }
        }
        if !progressed {
            break;
        }
    }
    let sections = test
        .sections
        .iter()
        .zip(picked)
        .filter(|(_, p)| !p.is_empty())
        .map(|(src, mut p)| {
            p.sort_unstable();
            let items = p.into_iter().map(|i| src.items[i].clone()).collect();
            TestSection { items, ..src.clone() }
        })
        .collect();
    TestContent {
        id: format!("level_check.{}", test.id.trim_end_matches(".test")),
        kind: TestKind::Level,
        intro: None,
        minutes: CHECK_MINUTES as u32,
        sections,
        ..test.clone()
    }
}

/// Whether to offer the monthly check on Today.
pub fn check_due(
    today: LocalDate,
    first_study_day: Option<LocalDate>,
    last_check: Option<LocalDate>,
) -> bool {
    match (last_check, first_study_day) {
        (Some(last), _) => today.days_since(last) >= CHECK_EVERY_DAYS,
        (None, Some(first)) => today.days_since(first) >= FIRST_CHECK_AFTER_DAYS,
        (None, None) => false,
    }
}

/// The radar axis a test skill feeds: grammar counts toward vocabulary, the
/// radar's "language knowledge" axis (same mapping as the test player).
pub fn radar_skill(s: TestSkill) -> Skill {
    match s {
        TestSkill::Listening => Skill::Listening,
        TestSkill::Reading => Skill::Reading,
        TestSkill::Writing => Skill::Writing,
        TestSkill::Speaking => Skill::Speaking,
        TestSkill::Vocabulary | TestSkill::Grammar => Skill::Vocabulary,
    }
}

fn skill_index(skill: Skill) -> usize {
    Skill::ALL.iter().position(|&s| s == skill).unwrap_or(0)
}

/// (correct, total) per radar axis in [`Skill::ALL`] order over the last
/// `days` days: practice results, and reviews as vocabulary.
pub fn recent_skills(events: &[ActivityEvent], today: LocalDate, days: i32) -> [(u32, u32); 5] {
    let mut out = [(0u32, 0u32); 5];
    for (_, s) in history::by_day(events).range(today.add_days(1 - days)..=today) {
        for p in &s.practice {
            let t = &mut out[skill_index(p.skill)];
            t.0 += p.correct;
            t.1 += p.total;
        }
        let v = &mut out[skill_index(Skill::Vocabulary)];
        v.0 += s.reviews_correct;
        v.1 += s.reviews;
    }
    out
}

/// The radar: each axis blends the mini check's share for it with recent
/// practice ([`CHECK_WEIGHT`] to the check), or uses whichever exists.
/// Practice needs [`MIN_SKILL_ATTEMPTS`] answers to count. Unmeasured axes
/// are left out rather than drawn as zero.
pub fn radar(check: &[(TestSkill, f32)], recent: &[(u32, u32); 5]) -> Vec<RadarPoint> {
    let mut sums: [(f32, u32); 5] = [(0.0, 0); 5];
    for &(s, share) in check {
        let t = &mut sums[skill_index(radar_skill(s))];
        t.0 += share.clamp(0.0, 1.0);
        t.1 += 1;
    }
    Skill::ALL
        .iter()
        .enumerate()
        .filter_map(|(i, &skill)| {
            let c = (sums[i].1 > 0).then(|| sums[i].0 / sums[i].1 as f32);
            let (correct, total) = recent[i];
            let h = (total >= MIN_SKILL_ATTEMPTS).then(|| correct.min(total) as f32 / total as f32);
            let value = match (c, h) {
                (Some(c), Some(h)) => CHECK_WEIGHT * c + (1.0 - CHECK_WEIGHT) * h,
                (c, h) => c.or(h)?,
            };
            Some(RadarPoint { skill, value })
        })
        .collect()
}

/// Change per axis since the last check (`None` when either side is unmeasured).
pub fn radar_change(now: &[RadarPoint], before: &[RadarPoint]) -> Vec<(Skill, Option<f32>)> {
    let get = |r: &[RadarPoint], s| r.iter().find(|p| p.skill == s).map(|p| p.value);
    Skill::ALL.iter().map(|&s| (s, get(now, s).zip(get(before, s)).map(|(a, b)| a - b))).collect()
}

/// What Kak Nur's rule looks at, from the last [`LOOKBACK_DAYS`].
/// `level_test` is the current level's full test, if taken within
/// [`LEVEL_TEST_FRESH_DAYS`]; `retention` is true retention over the window.
pub fn check_stats(
    events: &[ActivityEvent],
    active_days: u32,
    retention: Option<f32>,
    level_test: Option<LevelTest>,
    today: LocalDate,
) -> CheckStats {
    let days = history::by_day(events);
    let (correct, total) = days
        .range(today.add_days(1 - LOOKBACK_DAYS)..=today)
        .fold((0u32, 0u32), |(c, t), (_, s)| (c + s.reviews_correct, t + s.reviews));
    CheckStats {
        level_test,
        review_accuracy: (total > 0).then(|| correct as f32 / total as f32),
        retention,
        active_days,
    }
}

pub fn level_move(m: Moved) -> LevelMove {
    match m {
        Moved::Up => LevelMove::Up,
        Moved::Down => LevelMove::Down,
        Moved::Stay => LevelMove::Stay,
    }
}

/// Where the learner is on a route: the level holding the next lesson (the
/// last level once everything is done) and the share of its lessons done.
pub fn journey_level(curriculum: &Curriculum, progress: &Progress) -> Option<(String, f32)> {
    let level = match progress.current() {
        Some(cur) => curriculum.levels.iter().find(|l| l.lessons().any(|x| x.id == cur)),
        None => curriculum.levels.last(),
    }?;
    let (done, total) = progress.level_counts(level);
    let share = if total == 0 { 0.0 } else { done as f32 / total as f32 };
    Some((level.id.clone(), share))
}

/// `(track, number)` for `bipa3` / `bipa-3` / `j2`; `None` for anything else.
fn level_key(level: &str) -> Option<(&str, u32)> {
    let digits = level.trim_start_matches(|c: char| !c.is_ascii_digit());
    let track = level[..level.len() - digits.len()].trim_end_matches('-');
    Some((track, digits.parse().ok()?))
}

/// The level the check starts from. The journey (lessons done, flights,
/// placement) is the default. The last check adjusts it:
/// - after a move up, the higher of the two (the learner may not have opened
///   the new level's lessons yet);
/// - after a move down (consolidating), the check's level, until a level
///   test is passed or a placement is taken after it (`last_reset`).
pub fn current_level(
    journey: &str,
    last_check: Option<(&str, LevelMove, LocalDate)>,
    last_reset: Option<LocalDate>,
) -> String {
    match last_check {
        Some((level, LevelMove::Up, _)) => match (level_key(level), level_key(journey)) {
            (Some((t, a)), Some((u, b))) if t == u && a > b => level.to_string(),
            _ => journey.to_string(),
        },
        Some((level, LevelMove::Down, at)) if last_reset.is_none_or(|r| r < at) => level.to_string(),
        _ => journey.to_string(),
    }
}

/// The freshest full level-test score for `level`, among `(score, date)`
/// results, if recent enough to count.
pub fn fresh_level_test(results: &[(f32, LocalDate)], pass_mark: f32, today: LocalDate) -> Option<LevelTest> {
    results
        .iter()
        .filter(|(_, d)| today.days_since(*d) < LEVEL_TEST_FRESH_DAYS && *d <= today)
        .max_by_key(|(_, d)| *d)
        .map(|&(score, _)| LevelTest { score, pass_mark })
}

/// Practice entries the finished check adds to Kak Nur's log: one per skill.
pub fn practice_entries(by_skill: &BTreeMap<TestSkill, (u32, u32)>, level: &str) -> Vec<ActivityKind> {
    by_skill
        .iter()
        .filter(|(_, (_, total))| *total > 0)
        .map(|(skill, &(correct, total))| ActivityKind::Practice {
            skill: radar_skill(*skill),
            topic: match skill {
                TestSkill::Grammar => format!("level_check.{level} · grammar"),
                _ => format!("level_check.{level}"),
            },
            correct,
            total,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use kabar_protocol::test::TestItemKind;

    use super::*;
    use crate::coach::history::tests::{d, event};
    use crate::lesson::level_test::{SelfGrade, TestRunner, TestStep, parse_test};

    const BIPA1: &str = include_str!("../../../../content/tests/id/bipa1.test.yaml");
    const BIPA2: &str = include_str!("../../../../content/tests/id/bipa2.test.yaml");

    fn cost(t: &TestContent) -> f32 {
        t.sections.iter().flat_map(|s| &s.items).map(|i| minutes_of(&i.kind)).sum()
    }

    #[test]
    fn mini_check_is_about_ten_minutes_and_covers_every_section() {
        for yaml in [BIPA1, BIPA2] {
            let full = parse_test(yaml).unwrap();
            for seed in [0, 1, 7, month_seed(d(2026, 10, 26))] {
                let mini = mini_check(&full, seed);
                assert!(cost(&mini) <= CHECK_MINUTES + 1e-3, "{}", cost(&mini));
                assert!(cost(&mini) >= CHECK_MINUTES - 1.0, "{}", cost(&mini));
                assert_eq!(mini.sections.len(), full.sections.len(), "every skill is checked");
                assert_eq!(mini.pass_mark, full.pass_mark);
                assert_eq!(mini.minutes, 10);
                assert!(mini.id.starts_with("level_check.bipa"));
                let open = |f: fn(&TestItemKind) -> bool| {
                    mini.sections.iter().flat_map(|s| &s.items).filter(|i| f(&i.kind)).count()
                };
                assert!(open(|k| matches!(k, TestItemKind::WritingTask(_))) <= 1);
                assert!(open(|k| matches!(k, TestItemKind::SpeakingTask(_))) <= 1);
                // Items keep test order and stay unique.
                for (ms, fs) in mini.sections.iter().zip(&full.sections) {
                    let pos: Vec<usize> = ms
                        .items
                        .iter()
                        .map(|i| fs.items.iter().position(|x| x.id == i.id).unwrap())
                        .collect();
                    assert!(pos.windows(2).all(|w| w[0] < w[1]), "{pos:?}");
                }
                assert!(crate::lesson::level_test::validate_test(&mini).is_empty());
            }
        }
    }

    #[test]
    fn mini_check_changes_month_to_month_and_runs() {
        let full = parse_test(BIPA2).unwrap();
        let oct = mini_check(&full, month_seed(d(2026, 10, 1)));
        let nov = mini_check(&full, month_seed(d(2026, 11, 1)));
        let ids = |t: &TestContent| {
            t.sections.iter().flat_map(|s| &s.items).map(|i| i.id.clone()).collect::<Vec<_>>()
        };
        assert_ne!(ids(&oct), ids(&nov));
        assert_eq!(oct, mini_check(&full, month_seed(d(2026, 10, 31))), "stable within a month");
        // The TestRunner takes it start to finish.
        let mut r = TestRunner::new(oct);
        r.start();
        let mut guard = 0;
        while let TestStep::Item { .. } = r.step() {
            if r.exercise().is_some() {
                r.skip();
            } else {
                r.answer_open("Saya belajar bahasa Indonesia.");
            }
            guard += 1;
            assert!(guard < 100);
        }
        if r.step() == TestStep::SelfCheck {
            let ids: Vec<String> = r.open_answers().map(|x| x.id.clone()).collect();
            for id in ids {
                r.self_assess(&id, SelfGrade::Partly);
            }
            r.finish_self_check();
        }
        assert_eq!(r.step(), TestStep::Results);
    }

    #[test]
    fn due_once_a_month() {
        let today = d(2026, 10, 26);
        assert!(!check_due(today, None, None));
        assert!(!check_due(today, Some(today.add_days(-27)), None));
        assert!(check_due(today, Some(today.add_days(-28)), None));
        assert!(!check_due(today, Some(today.add_days(-90)), Some(today.add_days(-29))));
        assert!(check_due(today, Some(today.add_days(-90)), Some(today.add_days(-30))));
    }

    #[test]
    fn radar_blends_check_and_practice_and_skips_the_unmeasured() {
        let today = d(2026, 10, 26);
        let events = vec![
            event("a", today, 300, ActivityKind::Reviews { cards: 10, correct: 8 }),
            event(
                "b",
                today.add_days(-3),
                300,
                ActivityKind::Practice { skill: Skill::Writing, topic: "-kan".into(), correct: 3, total: 6 },
            ),
            event(
                "c",
                today.add_days(-2),
                60,
                ActivityKind::Practice { skill: Skill::Speaking, topic: "x".into(), correct: 1, total: 2 },
            ),
            // Too old to count.
            event("z", today.add_days(-40), 300, ActivityKind::Reviews { cards: 50, correct: 0 }),
        ];
        let recent = recent_skills(&events, today, LOOKBACK_DAYS);
        assert_eq!(recent[skill_index(Skill::Vocabulary)], (8, 10));
        let check = [
            (TestSkill::Reading, 1.0),
            (TestSkill::Vocabulary, 0.6),
            (TestSkill::Grammar, 0.8),
            (TestSkill::Writing, 0.5),
        ];
        let r = radar(&check, &recent);
        let get = |s| r.iter().find(|p| p.skill == s).map(|p| p.value);
        assert_eq!(get(Skill::Reading), Some(1.0));
        // Vocabulary: check (0.6 + 0.8) / 2 = 0.7 blended with reviews 0.8.
        assert!((get(Skill::Vocabulary).unwrap() - (0.6 * 0.7 + 0.4 * 0.8)).abs() < 1e-5);
        assert!((get(Skill::Writing).unwrap() - 0.5).abs() < 1e-5);
        // Listening never measured; speaking has too few answers.
        assert_eq!((get(Skill::Listening), get(Skill::Speaking)), (None, None));

        let before = vec![RadarPoint { skill: Skill::Reading, value: 0.75 }];
        let change = radar_change(&r, &before);
        assert_eq!(change[0], (Skill::Reading, Some(0.25)));
        assert_eq!(change[1], (Skill::Listening, None));
    }

    #[test]
    fn check_stats_feed_kak_nurs_rule() {
        use crate::coach::level::decide;
        let today = d(2026, 10, 26);
        // A month of hard reviews and low retention: consolidate.
        let events: Vec<_> = (0..20)
            .map(|i| {
                event(
                    &format!("e{i}"),
                    today.add_days(-i),
                    600,
                    ActivityKind::Reviews { cards: 20, correct: 9 },
                )
            })
            .collect();
        let s = check_stats(&events, 20, Some(0.5), None, today);
        assert_eq!(s.review_accuracy, Some(0.45));
        assert_eq!(decide("bipa2", &s).moved, Moved::Down);
        assert_eq!(level_move(decide("bipa2", &s).moved), LevelMove::Down);
        // A passed full test this month moves up.
        let t = fresh_level_test(&[(0.8, today.add_days(-3)), (0.5, today.add_days(-20))], 0.75, today);
        assert_eq!(t, Some(LevelTest { score: 0.8, pass_mark: 0.75 }));
        let s = check_stats(&events, 20, Some(0.9), t, today);
        assert_eq!(decide("bipa2", &s).level, "bipa3");
        // An old test doesn't count.
        assert_eq!(fresh_level_test(&[(0.9, today.add_days(-31))], 0.75, today), None);
        // No evidence: stay.
        let s = check_stats(&[], 0, None, None, today);
        assert_eq!(decide("bipa1", &s).moved, Moved::Stay);
    }

    #[test]
    fn current_level_follows_the_journey_and_the_last_check() {
        let today = d(2026, 10, 26);
        let (up, down, stay) = (LevelMove::Up, LevelMove::Down, LevelMove::Stay);
        assert_eq!(current_level("bipa3", None, None), "bipa3");
        // Staying never overrides the journey (placement, lessons, flights).
        assert_eq!(current_level("bipa3", Some(("bipa1", stay, today)), None), "bipa3");
        // Moved up before opening the new level's lessons.
        assert_eq!(current_level("bipa1", Some(("bipa2", up, today)), None), "bipa2");
        assert_eq!(current_level("bipa3", Some(("bipa2", up, today)), None), "bipa3");
        // Consolidating holds until a flight or placement comes after it.
        assert_eq!(current_level("bipa3", Some(("bipa2", down, today)), None), "bipa2");
        assert_eq!(current_level("bipa3", Some(("bipa2", down, today)), Some(today.add_days(-5))), "bipa2");
        assert_eq!(current_level("bipa3", Some(("bipa2", down, today.add_days(-10))), Some(today)), "bipa3");
        assert_eq!(level_key("bipa-2"), Some(("bipa", 2)));
        assert_eq!(level_key("j4"), Some(("j", 4)));
        assert_eq!(level_key("x"), None);
    }

    #[test]
    fn journey_level_and_share() {
        let cur =
            crate::journey::parse_curriculum(include_str!("../../../../curriculum/indonesian.yaml")).unwrap();
        let none = Progress::new(&cur, &Default::default());
        let (level, share) = journey_level(&cur, &none).unwrap();
        assert_eq!((level.as_str(), share), ("bipa1", 0.0));
        let flown: std::collections::BTreeSet<String> = ["bipa1.test".to_string()].into();
        let (level, _) = journey_level(&cur, &Progress::new(&cur, &flown)).unwrap();
        assert_eq!(level, "bipa2");
    }

    #[test]
    fn practice_entries_per_skill() {
        let mut m = BTreeMap::new();
        m.insert(TestSkill::Reading, (3, 4));
        m.insert(TestSkill::Grammar, (1, 2));
        m.insert(TestSkill::Speaking, (0, 0));
        let e = practice_entries(&m, "bipa2");
        assert_eq!(e.len(), 2);
        assert!(
            matches!(&e[1], ActivityKind::Practice { skill: Skill::Vocabulary, topic, .. } if topic.ends_with("grammar"))
        );
    }
}
