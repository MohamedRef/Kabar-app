//! Small pure helpers behind the coach screens (kept out of the components so
//! they can be tested without a WebView).

use kabar_core::coach::history;
use kabar_core::coach::plan::COMEBACK_AFTER_DAYS;
use kabar_core::coach::voice::Line;
use kabar_protocol::coach::{ActivityEvent, ActivityKind, LocalDate, LocalTime, WeeklyReport};

/// Study times as the profile keeps them: sorted, no duplicates.
pub fn normalize_times(mut times: Vec<LocalTime>) -> Vec<LocalTime> {
    times.sort();
    times.dedup();
    times
}

/// A sensible time to offer when "add a time" is tapped: an hour after the
/// last one, or 20:00 for the first.
pub fn next_time(times: &[LocalTime]) -> LocalTime {
    let Some(last) = times.iter().max() else {
        return LocalTime::hm(20, 0);
    };
    let mut t = LocalTime((last.0 + 60) % 1440);
    while times.contains(&t) {
        t = LocalTime((t.0 + 15) % 1440);
    }
    t
}

/// The learning journal: days newest first, each day's events newest first.
pub fn journal(events: &[ActivityEvent]) -> Vec<(LocalDate, Vec<&ActivityEvent>)> {
    let mut days: Vec<(LocalDate, Vec<&ActivityEvent>)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut sorted: Vec<&ActivityEvent> = events.iter().collect();
    sorted.sort_by(|a, b| b.date.cmp(&a.date).then(b.at_ms.cmp(&a.at_ms)).then(b.hlc.cmp(&a.hlc)));
    for e in sorted {
        if !seen.insert(e.id.as_str()) {
            continue; // a replayed event
        }
        match days.last_mut() {
            Some((d, es)) if *d == e.date => es.push(e),
            _ => days.push((e.date, vec![e])),
        }
    }
    days
}

/// Today's `PlanItemDone` events for one item (what "untick" retracts).
pub fn done_events<'a>(events: &'a [ActivityEvent], date: LocalDate, item_id: &str) -> Vec<&'a str> {
    events
        .iter()
        .filter(|e| e.date == date)
        .filter(|e| matches!(&e.kind, ActivityKind::PlanItemDone { item_id: id } if id == item_id))
        .map(|e| e.id.as_str())
        .collect()
}

/// Kak Nur's note on a weekly report, picked from the data: celebrate a
/// comeback, praise a strong week, and keep a light week light.
pub fn report_note(report: &WeeklyReport, events: &[ActivityEvent]) -> Line {
    if report.days_studied == 0 {
        return Line::ReportLightWeek;
    }
    let week = report.week_start..report.week_start.add_days(7);
    let days = history::by_day(events);
    let first_this_week = days.iter().find(|(d, s)| week.contains(d) && s.studied()).map(|(d, _)| *d);
    if let Some(first) = first_this_week {
        let before = history::last_study_day(events, first);
        if before.is_some_and(|b| first.days_since(b) > COMEBACK_AFTER_DAYS) {
            return Line::ReportComeback;
        }
    }
    let best_of_several = report.league.of > 1 && report.league.rank == 1;
    if report.days_studied >= 5 || (best_of_several && report.days_studied >= 3) {
        Line::ReportGreatWeek
    } else if report.days_studied >= 3 {
        Line::ReportSteadyWeek
    } else {
        Line::ReportLightWeek
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::LeagueResult;
    use kabar_protocol::{DeviceId, Hlc};

    use super::*;

    fn d(y: i32, m: u32, day: u32) -> LocalDate {
        LocalDate::from_ymd(y, m, day).unwrap()
    }

    fn ev(id: &str, date: LocalDate, at: u64, kind: ActivityKind) -> ActivityEvent {
        ActivityEvent {
            id: id.into(),
            hlc: Hlc { wall_ms: at, counter: 0, device: DeviceId("t".into()) },
            date,
            at_ms: at,
            kind,
            seconds: 300,
        }
    }

    fn reviews() -> ActivityKind {
        ActivityKind::Reviews { cards: 5, correct: 4 }
    }

    fn report(week_start: LocalDate, days: u32, rank: u32, of: u32) -> WeeklyReport {
        WeeklyReport {
            week_start,
            minutes: 60,
            days_studied: days,
            new_words: 10,
            retention: None,
            streak: days,
            strongest: None,
            weakest: None,
            league: LeagueResult { xp: 100, rank, of },
            next_focus: vec![],
        }
    }

    #[test]
    fn times_sort_dedup_and_suggest() {
        let t = |h, m| LocalTime::hm(h, m);
        assert_eq!(normalize_times(vec![t(20, 0), t(7, 30), t(20, 0)]), vec![t(7, 30), t(20, 0)]);
        assert_eq!(next_time(&[]), t(20, 0));
        assert_eq!(next_time(&[t(7, 30), t(20, 0)]), t(21, 0));
        assert_eq!(next_time(&[t(23, 30)]), t(0, 30));
        assert_eq!(next_time(&[t(20, 0), t(21, 0)]), t(22, 0));
    }

    #[test]
    fn journal_groups_newest_first_without_replays() {
        let mon = d(2026, 10, 26);
        let events = vec![
            ev("a", mon.add_days(-1), 10, reviews()),
            ev("b", mon, 20, reviews()),
            ev("c", mon, 30, ActivityKind::PlanItemDone { item_id: "review".into() }),
            ev("c", mon, 30, ActivityKind::PlanItemDone { item_id: "review".into() }),
        ];
        let j = journal(&events);
        assert_eq!(j.len(), 2);
        assert_eq!(j[0].0, mon);
        assert_eq!(j[0].1.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["c", "b"]);
        assert_eq!(j[1].0, mon.add_days(-1));
        assert_eq!(done_events(&events, mon, "review"), ["c", "c"]);
        assert!(done_events(&events, mon, "reading").is_empty());
    }

    #[test]
    fn report_notes_follow_the_data() {
        let week = d(2026, 10, 19);
        let mut events: Vec<ActivityEvent> =
            (0..5).map(|i| ev(&format!("w{i}"), week.add_days(i), 1, reviews())).collect();
        events.push(ev("old", week.add_days(-1), 0, reviews()));
        assert_eq!(report_note(&report(week, 5, 2, 4), &events), Line::ReportGreatWeek);
        assert_eq!(report_note(&report(week, 3, 1, 4), &events), Line::ReportGreatWeek);
        assert_eq!(report_note(&report(week, 3, 2, 4), &events), Line::ReportSteadyWeek);
        assert_eq!(report_note(&report(week, 1, 1, 1), &events), Line::ReportLightWeek);
        assert_eq!(report_note(&report(week, 0, 1, 1), &[]), Line::ReportLightWeek);

        // Back after two weeks away: a comeback, whatever the numbers.
        let back = vec![ev("x", week.add_days(-15), 0, reviews()), ev("y", week.add_days(2), 1, reviews())];
        assert_eq!(report_note(&report(week, 1, 1, 3), &back), Line::ReportComeback);
    }
}
