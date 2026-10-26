//! Small pure helpers behind the coach screens (kept out of the components so
//! they can be tested without a WebView).

use std::collections::{BTreeMap, HashMap, HashSet};

use kabar_core::coach::history::{self, DayStats};
use kabar_core::coach::plan::COMEBACK_AFTER_DAYS;
use kabar_core::coach::voice::Line;
use kabar_protocol::coach::{ActivityEvent, ActivityKind, LocalDate, LocalTime, WeeklyReport};
use kabar_protocol::journal::{JournalEntry, MAX_REFLECTION_CHARS};

use crate::store::{LessonDone, TaughtAt};

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

// --- the learning journal timeline -------------------------------------------

/// Lesson titles and unit souvenirs from the bundled curricula, so the
/// timeline can name what was finished.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    /// Lesson id → title; level test id → level code ("BIPA 1").
    pub titles: HashMap<String, String>,
    pub tests: HashSet<String>,
    pub units: Vec<UnitInfo>,
}

/// A unit that earns a souvenir once all of its lessons are finished.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitInfo {
    pub id: String,
    pub souvenir: String,
    pub lessons: Vec<String>,
}

/// What one line of the timeline says.
#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    Activity(ActivityKind),
    LessonDone { lesson_id: String, title: Option<String>, test: bool, score: Option<u32> },
    Souvenir { unit_id: String, name: String },
    Taught { word: String, draft: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimelineEntry {
    /// Stable within the timeline (RSX key).
    pub key: String,
    pub at_ms: u64,
    pub kind: EntryKind,
}

/// One day of the journal: what happened, what it added up to, and the
/// learner's own reflections.
#[derive(Clone, Debug, PartialEq)]
pub struct JournalDay {
    pub date: LocalDate,
    /// Newest first.
    pub entries: Vec<TimelineEntry>,
    pub stats: DayStats,
    /// In writing order.
    pub reflections: Vec<JournalEntry>,
}

/// The timeline's filter chips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JournalFilter {
    #[default]
    All,
    /// Reviews, lessons, practice, plan items and souvenirs.
    Learning,
    /// New words and words taught to Kabar.
    Words,
    /// Only days with a reflection, and only the reflections.
    Reflections,
}

impl JournalFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Learning, Self::Words, Self::Reflections];

    fn keeps(self, kind: &EntryKind) -> bool {
        use ActivityKind as A;
        match self {
            Self::All => true,
            Self::Reflections => false,
            Self::Words => matches!(kind, EntryKind::Taught { .. } | EntryKind::Activity(A::NewWords { .. })),
            Self::Learning => match kind {
                EntryKind::LessonDone { .. } | EntryKind::Souvenir { .. } => true,
                EntryKind::Taught { .. } => false,
                EntryKind::Activity(a) => !matches!(a, A::NewWords { .. } | A::Purchase { .. }),
            },
        }
    }

    fn shows_reflections(self) -> bool {
        matches!(self, Self::All | Self::Reflections)
    }
}

/// Everything the timeline is built from, besides the activity log.
#[derive(Clone, Debug, Default)]
pub struct JournalSources<'a> {
    pub lessons: &'a [LessonDone],
    pub taught: &'a [TaughtAt],
    pub reflections: &'a [JournalEntry],
}

/// A finished-lesson line already says what the generic "lesson step done"
/// event says, so that event is folded into it when they match.
const SAME_LESSON_WINDOW_MS: u64 = 10 * 60 * 1000;

/// Souvenirs earned: a unit's souvenir arrives when its last lesson is first
/// finished.
pub fn souvenirs_earned(units: &[UnitInfo], lessons: &[LessonDone]) -> Vec<(String, String, u64)> {
    let first_done = |id: &str| lessons.iter().filter(|l| l.lesson_id == id).map(|l| l.at_ms).min();
    units
        .iter()
        .filter(|u| !u.lessons.is_empty())
        .filter_map(|u| {
            let times: Option<Vec<u64>> = u.lessons.iter().map(|l| first_done(l)).collect();
            Some((u.id.clone(), u.souvenir.clone(), times?.into_iter().max()?))
        })
        .collect()
}

/// The learning journal: days newest first, each day's lines newest first,
/// with the day's totals and reflections. Replayed events show once. In the
/// unfiltered view today is always there, so a reflection can be written
/// before anything else happens.
pub fn timeline(
    events: &[ActivityEvent],
    src: &JournalSources<'_>,
    catalog: &Catalog,
    utc_offset_min: i16,
    today: LocalDate,
    filter: JournalFilter,
) -> Vec<JournalDay> {
    let stats = history::by_day(events);
    let day_of = |ms: u64| LocalDate::from_unix_ms(ms, utc_offset_min);
    let mut lines: Vec<(LocalDate, TimelineEntry)> = Vec::new();

    let mut seen = HashSet::new();
    for e in events {
        if !seen.insert(e.id.as_str()) {
            continue; // a replayed event
        }
        if let ActivityKind::LessonStep { unit_id, done: true } = &e.kind {
            let prefix = format!("{unit_id}.");
            let folded = src.lessons.iter().any(|l| {
                l.lesson_id.starts_with(&prefix) && l.at_ms.abs_diff(e.at_ms) <= SAME_LESSON_WINDOW_MS
            });
            if folded {
                continue;
            }
        }
        let entry =
            TimelineEntry { key: e.id.clone(), at_ms: e.at_ms, kind: EntryKind::Activity(e.kind.clone()) };
        lines.push((e.date, entry));
    }
    for (i, l) in src.lessons.iter().enumerate() {
        let kind = EntryKind::LessonDone {
            lesson_id: l.lesson_id.clone(),
            title: catalog.titles.get(&l.lesson_id).cloned(),
            test: catalog.tests.contains(&l.lesson_id),
            score: l.score,
        };
        lines.push((
            day_of(l.at_ms),
            TimelineEntry { key: format!("lesson:{i}:{}", l.lesson_id), at_ms: l.at_ms, kind },
        ));
    }
    for (unit_id, name, at_ms) in souvenirs_earned(&catalog.units, src.lessons) {
        let key = format!("souvenir:{unit_id}");
        lines
            .push((day_of(at_ms), TimelineEntry { key, at_ms, kind: EntryKind::Souvenir { unit_id, name } }));
    }
    for t in src.taught {
        let kind = EntryKind::Taught { word: t.entry.entry.word.clone(), draft: t.entry.is_draft() };
        lines.push((
            day_of(t.at_ms),
            TimelineEntry { key: format!("taught:{}", t.entry.row_id()), at_ms: t.at_ms, kind },
        ));
    }
    lines.retain(|(_, e)| filter.keeps(&e.kind));

    fn day<'m>(
        days: &'m mut BTreeMap<LocalDate, JournalDay>,
        stats: &BTreeMap<LocalDate, DayStats>,
        date: LocalDate,
    ) -> &'m mut JournalDay {
        days.entry(date).or_insert_with(|| JournalDay {
            date,
            entries: Vec::new(),
            stats: stats.get(&date).cloned().unwrap_or_default(),
            reflections: Vec::new(),
        })
    }
    let mut days: BTreeMap<LocalDate, JournalDay> = BTreeMap::new();
    for (date, entry) in lines {
        day(&mut days, &stats, date).entries.push(entry);
    }
    if filter.shows_reflections() {
        for r in src.reflections {
            day(&mut days, &stats, r.date).reflections.push(r.clone());
        }
    }
    if filter == JournalFilter::All {
        day(&mut days, &stats, today);
    }

    let mut out: Vec<JournalDay> = days.into_values().rev().collect();
    for d in &mut out {
        d.entries.sort_by(|a, b| b.at_ms.cmp(&a.at_ms).then_with(|| a.key.cmp(&b.key)));
        d.reflections.sort_by(|a, b| a.created_ms.cmp(&b.created_ms).then_with(|| a.id.cmp(&b.id)));
    }
    out
}

/// A reflection as typed, ready to save: trimmed at the ends (inner line
/// breaks kept) and capped at [`MAX_REFLECTION_CHARS`]. `None` when there's
/// nothing to keep.
pub fn clean_reflection(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    Some(t.chars().take(MAX_REFLECTION_CHARS).collect())
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
        let j =
            timeline(&events, &JournalSources::default(), &Catalog::default(), 0, mon, JournalFilter::All);
        assert_eq!(j.len(), 2);
        assert_eq!(j[0].date, mon);
        assert_eq!(j[0].entries.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["c", "b"]);
        assert_eq!(j[0].stats.reviews, 5, "the replay counts once");
        assert_eq!(j[1].date, mon.add_days(-1));
        assert_eq!(done_events(&events, mon, "review"), ["c", "c"]);
        assert!(done_events(&events, mon, "reading").is_empty());
    }

    /// Unix ms at `h:00` UTC on `date`.
    fn at(date: LocalDate, h: u64) -> u64 {
        date.0 as u64 * 86_400_000 + h * 3_600_000
    }

    fn catalog() -> Catalog {
        Catalog {
            titles: [("bipa1.u01.l01", "Halo!"), ("bipa1.u01.l02", "Nama saya…"), ("bipa1.test", "BIPA 1")]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            tests: ["bipa1.test".to_string()].into(),
            units: vec![UnitInfo {
                id: "bipa1.u01".into(),
                souvenir: "Batik".into(),
                lessons: vec!["bipa1.u01.l01".into(), "bipa1.u01.l02".into()],
            }],
        }
    }

    fn note(id: &str, date: LocalDate, created: u64) -> JournalEntry {
        JournalEntry {
            id: id.into(),
            date,
            text: "Saya sedang belajar bahasa Indonesia.".into(),
            ink: None,
            created_ms: created,
            updated_ms: created,
        }
    }

    #[test]
    fn timeline_merges_every_source_by_day() {
        let mon = d(2026, 10, 26);
        let sun = mon.add_days(-1);
        let events = vec![
            ev("rev", sun, at(sun, 8), reviews()),
            ev("step", mon, at(mon, 9), ActivityKind::LessonStep { unit_id: "bipa1.u01".into(), done: true }),
            ev(
                "words",
                mon,
                at(mon, 9) + 1,
                ActivityKind::NewWords { words: vec!["pasar".into(), "murah".into()] },
            ),
            ev("buy", mon, at(mon, 11), ActivityKind::Purchase { item_id: "batik-parang".into(), koin: 50 }),
        ];
        let lessons = vec![
            LessonDone { lesson_id: "bipa1.u01.l01".into(), at_ms: at(sun, 7), score: Some(90) },
            LessonDone { lesson_id: "bipa1.u01.l02".into(), at_ms: at(mon, 9) - 1000, score: Some(81) },
        ];
        let taught: TaughtAt = TaughtAt {
            entry: serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json"))
                .unwrap(),
            at_ms: at(mon, 10),
        };
        let taught = [taught];
        let reflections = [note("r2", mon, 5), note("r1", mon, 3)];
        let src = JournalSources { lessons: &lessons, taught: &taught, reflections: &reflections };
        let j = timeline(&events, &src, &catalog(), 0, mon, JournalFilter::All);

        assert_eq!(j.iter().map(|d| d.date).collect::<Vec<_>>(), [mon, sun]);
        let keys: Vec<&str> = j[0].entries.iter().map(|e| e.key.as_str()).collect();
        // The generic "lesson step" is folded into the finished lesson it matches;
        // finishing the unit's last lesson earns its souvenir.
        assert_eq!(
            keys,
            ["buy", "taught:id:kalimat", "words", "lesson:1:bipa1.u01.l02", "souvenir:bipa1.u01"]
        );
        assert_eq!(j[0].entries[1].kind, EntryKind::Taught { word: "kalimat".into(), draft: true });
        let lesson = &j[0].entries[3].kind;
        assert_eq!(
            lesson,
            &EntryKind::LessonDone {
                lesson_id: "bipa1.u01.l02".into(),
                title: Some("Nama saya…".into()),
                test: false,
                score: Some(81)
            }
        );
        assert!(matches!(&j[0].entries[4].kind, EntryKind::Souvenir { name, .. } if name == "Batik"));
        assert_eq!(j[0].reflections.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["r1", "r2"]);
        assert_eq!(j[0].stats.new_words, ["pasar", "murah"]);
        assert_eq!(
            j[1].entries.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(),
            ["rev", "lesson:0:bipa1.u01.l01"]
        );
    }

    #[test]
    fn timeline_days_follow_the_learners_clock() {
        // 22:30 UTC on Sunday is already Monday in Jakarta, still Sunday in London.
        let mon = d(2026, 10, 26);
        let sun = mon.add_days(-1);
        let lessons =
            [LessonDone { lesson_id: "bipa1.test".into(), at_ms: at(sun, 22) + 1_800_000, score: None }];
        let src = JournalSources { lessons: &lessons, ..Default::default() };
        let j = timeline(&[], &src, &catalog(), 420, mon, JournalFilter::Learning);
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].date, mon);
        assert!(matches!(j[0].entries[0].kind, EntryKind::LessonDone { test: true, .. }));
        let j = timeline(&[], &src, &catalog(), 0, mon, JournalFilter::Learning);
        assert_eq!(j[0].date, sun);
    }

    #[test]
    fn timeline_filters_and_keeps_today_open() {
        let mon = d(2026, 10, 26);
        let fri = mon.add_days(-3);
        let events = vec![
            ev("rev", fri, at(fri, 8), reviews()),
            ev("words", fri, at(fri, 9), ActivityKind::NewWords { words: vec!["kabar".into()] }),
            ev("buy", fri, at(fri, 10), ActivityKind::Purchase { item_id: "x".into(), koin: 5 }),
        ];
        let reflections = [note("r", fri.add_days(-1), 1)];
        let src = JournalSources { reflections: &reflections, ..Default::default() };
        let run = |f| timeline(&events, &src, &Catalog::default(), 0, mon, f);

        let all = run(JournalFilter::All);
        // Today (empty, ready for a reflection), Friday, Thursday's reflection.
        assert_eq!(all.iter().map(|d| d.date).collect::<Vec<_>>(), [mon, fri, fri.add_days(-1)]);
        assert!(all[0].entries.is_empty());
        assert_eq!(all[1].entries.len(), 3);

        let keys = |days: &[JournalDay]| -> Vec<String> {
            days.iter().flat_map(|d| d.entries.iter().map(|e| e.key.clone())).collect()
        };
        assert_eq!(keys(&run(JournalFilter::Learning)), ["rev"]);
        assert_eq!(keys(&run(JournalFilter::Words)), ["words"]);
        let only_notes = run(JournalFilter::Reflections);
        assert_eq!(only_notes.len(), 1);
        assert!(only_notes[0].entries.is_empty());
        assert_eq!(only_notes[0].reflections.len(), 1);
        assert!(run(JournalFilter::Words).iter().all(|d| d.reflections.is_empty()));
    }

    #[test]
    fn souvenirs_need_every_lesson_of_the_unit() {
        let units = catalog().units;
        let one = [LessonDone { lesson_id: "bipa1.u01.l01".into(), at_ms: 5, score: None }];
        assert!(souvenirs_earned(&units, &one).is_empty());
        let both = [
            LessonDone { lesson_id: "bipa1.u01.l02".into(), at_ms: 9, score: None },
            one[0].clone(),
            // Redoing a lesson later doesn't move the souvenir.
            LessonDone { lesson_id: "bipa1.u01.l02".into(), at_ms: 20, score: None },
        ];
        assert_eq!(souvenirs_earned(&units, &both), [("bipa1.u01".into(), "Batik".into(), 9)]);
    }

    #[test]
    fn reflections_are_trimmed_and_capped() {
        assert_eq!(clean_reflection("  \n "), None);
        assert_eq!(
            clean_reflection("  Saya mau ke pasar.\nMurah sekali!\n").as_deref(),
            Some("Saya mau ke pasar.\nMurah sekali!")
        );
        let long = "ا".repeat(MAX_REFLECTION_CHARS + 10);
        assert_eq!(clean_reflection(&long).unwrap().chars().count(), MAX_REFLECTION_CHARS);
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
