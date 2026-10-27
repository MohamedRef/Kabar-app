use kabar_protocol::coach::ActivityKind;
use kabar_protocol::srs::ReviewMode;
use kabar_protocol::{DeviceId, Hlc};

use super::*;
use crate::coach::history::tests::{d, event};
use crate::review::seed_bridge_deck;

const OFFSET: i16 = 120;

/// 18:00 local on `date`.
fn at(date: LocalDate) -> u64 {
    (i64::from(date.0) * DAY_MS as i64 + 16 * 3_600_000) as u64
}

fn review(id: &str, card: &str, at_ms: u64, rating: Rating, duration_ms: u32) -> ReviewEvent {
    ReviewEvent {
        id: id.into(),
        card_id: card.into(),
        hlc: Hlc { wall_ms: at_ms, counter: 0, device: DeviceId("t".into()) },
        at_ms,
        rating,
        duration_ms,
        mode: ReviewMode::Classic,
    }
}

#[test]
fn day_totals_take_the_larger_of_both_logs() {
    let today = d(2026, 10, 26);
    let events = vec![
        // A finished session: 5 minutes, 4 reviews.
        event("a", today, 300, ActivityKind::Reviews { cards: 4, correct: 3 }),
        // A finished lesson without minutes still counts as study.
        event("b", today.add_days(-1), 0, ActivityKind::LessonStep { unit_id: "u".into(), done: true }),
    ];
    let log = vec![
        review("r1", "c", at(today), Rating::Good, 60_000),
        review("r2", "c", at(today), Rating::Good, 30_000),
        review("r2", "c", at(today), Rating::Good, 30_000), // replayed
        // An interrupted session two days ago: only the review log has it.
        review("r3", "c", at(today.add_days(-2)), Rating::Again, 90_000),
    ];
    let t = day_totals(&events, &log, OFFSET);
    assert_eq!(t[&today], DayTotal { minutes: 5, reviews: 4, studied: true });
    assert_eq!(t[&today.add_days(-1)], DayTotal { minutes: 0, reviews: 0, studied: true });
    assert_eq!(t[&today.add_days(-2)], DayTotal { minutes: 2, reviews: 1, studied: true });
}

#[test]
fn review_days_follow_the_local_calendar() {
    // 23:30 in Cairo (+2) on the 26th is 21:30 UTC: still the 26th locally.
    let day = d(2026, 10, 26);
    let late = (i64::from(day.0) * DAY_MS as i64 + 21 * 3_600_000 + 30 * 60_000) as u64;
    let t = day_totals(&[], &[review("r", "c", late, Rating::Good, 1000)], OFFSET);
    assert!(t.contains_key(&day));
    // In Jakarta (+7) the same moment is 04:30 on the 27th.
    let t = day_totals(&[], &[review("r", "c", late, Rating::Good, 1000)], 420);
    assert!(t.contains_key(&day.add_days(1)));
}

#[test]
fn heatmap_spans_whole_weeks_and_shades_by_plan_size() {
    let today = d(2026, 10, 29); // a Thursday
    let mut totals = BTreeMap::new();
    for (back, minutes) in [(0, 3), (1, 5), (2, 15), (3, 45), (10, 0)] {
        totals.insert(today.add_days(-back), DayTotal { minutes, reviews: 0, studied: true });
    }
    let h = heatmap(&totals, today, HEATMAP_WEEKS);
    assert_eq!(h.start, d(2026, 10, 26).add_days(-7 * 19));
    assert_eq!(h.start.weekday(), kabar_protocol::coach::Weekday::Mon);
    assert_eq!(h.days.len(), 19 * 7 + 4);
    assert_eq!(h.weeks(), 20);
    assert_eq!(h.days.last().unwrap().date, today);
    let levels: Vec<u8> = h.days.iter().rev().take(4).map(|x| x.level).collect();
    assert_eq!(levels, [1, 2, 3, 4]);
    assert_eq!(h.active_days, 5, "a zero-minute study day still lights up");
    assert_eq!(h.minutes, 68);
    assert_eq!(heat_level(&DayTotal::default()), 0);
}

#[test]
fn time_studied_windows() {
    let today = d(2026, 10, 26);
    let mut totals = BTreeMap::new();
    for back in [0, 6, 7, 29, 30] {
        totals.insert(today.add_days(-back), DayTotal { minutes: 10, reviews: 0, studied: true });
    }
    let t = time_studied(&totals, today);
    assert_eq!(t.week, TimeSpan { minutes: 20, days: 2 });
    assert_eq!(t.month, TimeSpan { minutes: 40, days: 4 });
    assert_eq!(t.total_minutes, 50);
}

#[test]
fn known_words_count_each_word_once_by_its_best_card() {
    let sched = Scheduler::default();
    let cards = seed_bridge_deck();
    let (w1, w2) = (&cards[0], &cards[2]);
    assert_eq!(cards[1].word, w1.word, "recognition + bridge per word");
    let t0 = at(d(2026, 6, 1));
    // w1: reviewed "Good" with growing gaps until stability passes 21 days.
    let mut log = Vec::new();
    let mut s = CardState::new(w1.id.as_str());
    let mut t = t0;
    for i in 0..6 {
        log.push(review(&format!("a{i}"), &w1.id, t, Rating::Good, 3000));
        s = sched.apply(&s, Rating::Good, t);
        t = s.due_ms.unwrap();
    }
    assert!(s.memory.unwrap().stability >= KNOWN_STABILITY_DAYS);
    // w2: one review only.
    log.push(review("b", &w2.id, t0, Rating::Again, 3000));
    let states = sched.replay_all(&log);
    let counts = word_counts(&cards, |id| states.get(id).cloned().unwrap_or_else(|| CardState::new(id)));
    assert_eq!(counts.len(), 1);
    let c = counts[0];
    assert_eq!(c.language, StudyLanguage::Id);
    assert_eq!((c.known, c.learning), (1, 1));
    assert_eq!(c.total() as usize, cards.len() / 2);
}

#[test]
fn retention_splits_mature_and_young() {
    let sched = Scheduler::default();
    let now = at(d(2026, 10, 26));
    let mut log = Vec::new();
    // Card m: Good with growing gaps, so its last intervals are mature.
    let mut s = CardState::new("m");
    let mut t = now - 300 * DAY_MS;
    let mut i = 0;
    while t < now - 10 * DAY_MS {
        log.push(review(&format!("m{i}"), "m", t, Rating::Good, 1000));
        s = sched.apply(&s, Rating::Good, t);
        t = s.due_ms.unwrap();
        i += 1;
    }
    // Then a lapse on the next due date, within the window.
    let interval = (s.due_ms.unwrap() - s.last_review_ms.unwrap()) as f32 / DAY_MS as f32;
    assert!(interval >= MATURE_INTERVAL_DAYS);
    log.push(review("mlapse", "m", now - DAY_MS, Rating::Again, 1000));
    // Card y: first review (not counted), then a young pass.
    log.push(review("y0", "y", now - 3 * DAY_MS, Rating::Good, 1000));
    log.push(review("y1", "y", now - 2 * DAY_MS, Rating::Good, 1000));
    let r7 = retention(&sched, &log, now, 7);
    assert_eq!(r7.mature, Tally { passed: 0, total: 1 });
    assert_eq!(r7.young, Tally { passed: 1, total: 1 });
    assert_eq!(r7.all().share(), Some(0.5));
    let all = retention(&sched, &log, now, 400);
    assert!(all.mature.total >= 1 && all.young.total >= 2);
    assert_eq!(retention(&sched, &[], now, 30).all().share(), None);

    // One pass gives the same windows and the same card states.
    let (states, windows) = replay_with_retention(&sched, &log, now, &[7, 30, 400]);
    assert_eq!(windows, [r7, retention(&sched, &log, now, 30), all]);
    assert_eq!(states, sched.replay_all(&log));
}

#[test]
fn forecast_summary_finds_the_busy_day() {
    let s = forecast_summary(&[3, 0, 7, 7, 1]);
    assert_eq!(s, ForecastSummary { total: 18, peak_day: 2, peak: 7, per_day: 4 });
    assert_eq!(forecast_summary(&[]), ForecastSummary::default());
    assert_eq!(forecast_summary(&[2, 0, 0, 0, 0]).per_day, 1);
}
