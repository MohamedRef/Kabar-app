//! The weekly report (Thursday evening by default): time, new words,
//! retention, Pelita, strongest and weakest skills, Liga Pribadi and next
//! week's focus. Kak Nur's note is picked by [`super::voice`].

use std::collections::BTreeSet;

use kabar_protocol::coach::{
    ActivityEvent, CoachProfile, LeagueResult, LocalDate, Skill, WeeklyReport, to_unix_ms,
};

use super::history;
use super::plan::weak_spots_between;

/// Answers in a skill over the week before it can be called strongest or weakest.
pub const MIN_SKILL_ATTEMPTS: u32 = 5;
/// How many topics "next week's focus" names at most.
pub const MAX_FOCUS: usize = 3;

/// The report for the 7 days starting `week_start` (a Monday).
pub fn weekly_report(
    events: &[ActivityEvent],
    week_start: LocalDate,
    streak: u32,
    league: LeagueResult,
) -> WeeklyReport {
    let week_end = week_start.add_days(7);
    let days = history::by_day(events);
    let week = days.range(week_start..week_end).map(|(_, s)| s);

    let (mut minutes, mut days_studied, mut reviews, mut reviews_correct) = (0, 0, 0u32, 0u32);
    let mut words = BTreeSet::new();
    // (correct, total) per skill, in Skill::ALL order.
    let mut skills = [(0u32, 0u32); 5];
    for s in week {
        minutes += s.minutes();
        days_studied += u32::from(s.studied());
        reviews += s.reviews;
        reviews_correct += s.reviews_correct;
        words.extend(s.new_words.iter().map(String::as_str));
        for p in &s.practice {
            let t = &mut skills[skill_index(p.skill)];
            t.0 += p.correct;
            t.1 += p.total;
        }
    }
    // Reviews are vocabulary practice too.
    let vocab = &mut skills[skill_index(Skill::Vocabulary)];
    vocab.0 += reviews_correct;
    vocab.1 += reviews;

    let (strongest, weakest) = strongest_and_weakest(&skills);
    WeeklyReport {
        week_start,
        minutes,
        days_studied,
        new_words: words.len() as u32,
        retention: (reviews > 0).then(|| reviews_correct as f32 / reviews as f32),
        streak,
        strongest,
        weakest,
        league,
        next_focus: weak_spots_between(events, week_start, week_end).into_iter().take(MAX_FOCUS).collect(),
    }
}

fn skill_index(skill: Skill) -> usize {
    Skill::ALL.iter().position(|&s| s == skill).unwrap_or(0)
}

/// Best and worst accuracy among skills with enough answers. Ties go to the
/// earlier skill in [`Skill::ALL`]. `weakest` is `None` unless it is truly
/// below the strongest, so one practised skill is only ever "strongest".
fn strongest_and_weakest(skills: &[(u32, u32); 5]) -> (Option<Skill>, Option<Skill>) {
    let qualified: Vec<(Skill, u32, u32)> = Skill::ALL
        .iter()
        .zip(skills)
        .filter(|(_, (_, total))| *total >= MIN_SKILL_ATTEMPTS)
        .map(|(&s, &(c, t))| (s, c, t))
        .collect();
    // a/b vs c/d without floats.
    let better = |a: &(Skill, u32, u32), b: &(Skill, u32, u32)| {
        u64::from(a.1) * u64::from(b.2) > u64::from(b.1) * u64::from(a.2)
    };
    let mut strong: Option<&(Skill, u32, u32)> = None;
    let mut weak: Option<&(Skill, u32, u32)> = None;
    for q in &qualified {
        if strong.is_none_or(|s| better(q, s)) {
            strong = Some(q);
        }
        if weak.is_none_or(|w| better(w, q)) {
            weak = Some(q);
        }
    }
    match (strong, weak) {
        (Some(s), Some(w)) if better(s, w) => (Some(s.0), Some(w.0)),
        (s, _) => (s.map(|s| s.0), None),
    }
}

/// If a report is due at `now_ms` and hasn't been made yet, the Monday of the
/// week it covers. `last_report` is the `week_start` of the newest report.
///
/// Each week's report falls on `profile.report_day` at `profile.report_time`
/// (local time) and covers that Monday-to-Sunday week. Before this week's
/// moment, last week's report is still due if it was missed, but only for a
/// learner who has had a report before: a brand new learner never gets a
/// report for a week before they joined.
pub fn report_due(profile: &CoachProfile, now_ms: u64, last_report: Option<LocalDate>) -> Option<LocalDate> {
    let offset = profile.utc_offset_min;
    let this_week = LocalDate::from_unix_ms(now_ms, offset).week_start();
    let day = this_week.add_days(i32::from(profile.report_day.index()));
    let week = if now_ms >= to_unix_ms(day, profile.report_time, offset) {
        this_week
    } else if last_report.is_some() {
        this_week.add_days(-7)
    } else {
        return None;
    };
    match last_report {
        Some(last) if last >= week => None,
        _ => Some(week),
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::{ActivityKind, Focus, LocalTime, Weekday};

    use super::*;
    use crate::coach::history::tests::{d, event};

    fn league() -> LeagueResult {
        LeagueResult { xp: 420, rank: 1, of: 4 }
    }

    fn practice(skill: Skill, topic: &str, correct: u32, total: u32) -> ActivityKind {
        ActivityKind::Practice { skill, topic: topic.into(), correct, total }
    }

    fn words(w: &[&str]) -> ActivityKind {
        ActivityKind::NewWords { words: w.iter().map(|s| (*s).to_owned()).collect() }
    }

    #[test]
    fn report_sums_the_week() {
        let mon = d(2026, 10, 26);
        let events = vec![
            event("a", mon, 600, ActivityKind::Reviews { cards: 20, correct: 15 }),
            event("b", mon, 120, words(&["membaca", "buku"])),
            event("c", mon.add_days(2), 300, ActivityKind::Reviews { cards: 20, correct: 17 }),
            event("d", mon.add_days(2), 60, words(&["buku", "taman"])),
            event("e", mon.add_days(3), 300, practice(Skill::Writing, "-kan verbs", 2, 6)),
            event("f", mon.add_days(3), 300, practice(Skill::Reading, "di- passive", 5, 6)),
            event("g", mon.add_days(6), 30, practice(Skill::Listening, "apa kabar", 1, 2)),
            // Outside the week.
            event("x", mon.add_days(-1), 900, ActivityKind::Reviews { cards: 99, correct: 0 }),
            event("y", mon.add_days(7), 900, words(&["kesehatan"])),
            // Spending isn't studying.
            event("p", mon.add_days(5), 0, ActivityKind::Purchase { item_id: "batik".into(), koin: 50 }),
        ];
        let r = weekly_report(&events, mon, 9, league());
        assert_eq!(r.week_start, mon);
        assert_eq!(r.minutes, 12 + 6 + 10 + 1);
        assert_eq!(r.days_studied, 4);
        assert_eq!(r.new_words, 3);
        assert_eq!(r.retention, Some(32.0 / 40.0));
        assert_eq!(r.streak, 9);
        assert_eq!(r.league, league());
        // Reading 5/6 beats vocabulary (reviews) 32/40; writing 2/6 is weakest;
        // listening has too few answers to count.
        assert_eq!(r.strongest, Some(Skill::Reading));
        assert_eq!(r.weakest, Some(Skill::Writing));
        assert_eq!(r.next_focus, [Focus { skill: Skill::Writing, topic: "-kan verbs".into() }]);
    }

    #[test]
    fn an_empty_week_reports_nothing_made_up() {
        let r = weekly_report(&[], d(2026, 10, 26), 0, league());
        assert_eq!((r.minutes, r.days_studied, r.new_words), (0, 0, 0));
        assert_eq!((r.retention, r.strongest, r.weakest), (None, None, None));
        assert!(r.next_focus.is_empty());
    }

    #[test]
    fn one_skill_is_only_ever_strongest() {
        let mon = d(2026, 10, 26);
        let events = [event("a", mon, 300, ActivityKind::Reviews { cards: 10, correct: 6 })];
        let r = weekly_report(&events, mon, 1, league());
        assert_eq!((r.strongest, r.weakest), (Some(Skill::Vocabulary), None));
        // Two skills at the same accuracy: no weakest either.
        let events = [
            event("a", mon, 300, practice(Skill::Reading, "di- passive", 3, 6)),
            event("b", mon, 300, practice(Skill::Speaking, "apa kabar", 3, 6)),
        ];
        let r = weekly_report(&events, mon, 1, league());
        assert_eq!((r.strongest, r.weakest), (Some(Skill::Reading), None));
    }

    #[test]
    fn next_focus_names_at_most_three() {
        let mon = d(2026, 10, 26);
        let events: Vec<_> = ["-kan verbs", "di- passive", "ber- verbs", "ke-an nouns"]
            .iter()
            .enumerate()
            .map(|(i, t)| event(&format!("e{i}"), mon, 60, practice(Skill::Writing, t, i as u32, 5)))
            .collect();
        let r = weekly_report(&events, mon, 1, league());
        let topics: Vec<&str> = r.next_focus.iter().map(|f| f.topic.as_str()).collect();
        assert_eq!(topics, ["-kan verbs", "di- passive", "ber- verbs"]);
    }

    fn profile() -> CoachProfile {
        // Thursday 19:00 in Cairo (+2), the default.
        CoachProfile::default()
    }

    fn at(date: LocalDate, h: u16, m: u16) -> u64 {
        to_unix_ms(date, LocalTime::hm(h, m), profile().utc_offset_min)
    }

    #[test]
    fn report_fires_on_the_report_day() {
        let p = profile();
        let mon = d(2026, 10, 26);
        let thu = mon.add_days(3);
        let last_week = mon.add_days(-7);
        assert_eq!(report_due(&p, at(thu, 18, 59), Some(last_week)), None);
        assert_eq!(report_due(&p, at(thu, 19, 0), Some(last_week)), Some(mon));
        assert_eq!(report_due(&p, at(mon.add_days(6), 23, 59), Some(last_week)), Some(mon));
        // Already made.
        assert_eq!(report_due(&p, at(thu, 20, 0), Some(mon)), None);
        // First ever report: this week's, at its moment.
        assert_eq!(report_due(&p, at(thu, 19, 0), None), Some(mon));
        assert_eq!(report_due(&p, at(thu, 9, 0), None), None);
    }

    #[test]
    fn a_missed_report_is_still_made() {
        let p = profile();
        let mon = d(2026, 10, 26);
        // Last week's report never made; now it's Tuesday of this week.
        assert_eq!(
            report_due(&p, at(mon.add_days(1), 10, 0), Some(mon.add_days(-14))),
            Some(mon.add_days(-7))
        );
        assert_eq!(report_due(&p, at(mon.add_days(1), 10, 0), Some(mon.add_days(-7))), None);
    }

    #[test]
    fn report_moment_follows_profile_and_offset() {
        let mon = d(2026, 10, 26);
        let p = CoachProfile {
            report_day: Weekday::Sun,
            report_time: LocalTime::hm(9, 0),
            utc_offset_min: 420,
            ..CoachProfile::default()
        };
        let sun = mon.add_days(6);
        let before = to_unix_ms(sun, LocalTime::hm(8, 59), 420);
        let after = to_unix_ms(sun, LocalTime::hm(9, 0), 420);
        assert_eq!(report_due(&p, before, Some(mon.add_days(-7))), None);
        assert_eq!(report_due(&p, after, Some(mon.add_days(-7))), Some(mon));
    }
}
