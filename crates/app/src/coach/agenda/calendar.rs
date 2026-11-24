use super::super::{Coach, labels};
use super::assignments;
use crate::{Db, Ui};
use dioxus::prelude::*;
use kabar_protocol::coach::LocalDate;

mod text;
use text::Message as M;

#[component]
pub(crate) fn LessonCalendar(
    totals: std::collections::BTreeMap<LocalDate, kabar_core::stats::DayTotal>,
) -> Element {
    use kabar_protocol::coach::Weekday;
    use std::collections::BTreeMap;
    use study_ui::{CalendarDay, StudyCalendar, calendar_months};

    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let today = coach.today();
    let selected = use_signal(|| today.0);
    let lang = ui.lang();
    let profile = coach.profile.read();
    let _plan_rev = coach.plan_rev.read();
    let target = profile.as_ref().and_then(|p| p.target.as_ref());
    let (lessons, plan) = profile.as_ref().map_or_else(Default::default, |profile| {
        let stored = crate::persistence::read_or("today's plan", db.0.borrow().plan_for(today));
        let minutes = stored.map_or(profile.daily_size.minutes(), |plan| plan.size.minutes());
        assignments(&db, &coach, profile, u32::from(minutes), lang)
    });

    let mut completed: BTreeMap<LocalDate, Vec<(String, String)>> = BTreeMap::new();
    let events = crate::persistence::read_or("lesson completions", db.0.borrow().lesson_completions());
    for event in events {
        if let Some(title) = crate::journey::lesson_title(&event.lesson_id) {
            let day = coach.zone().date(event.at_ms);
            let rows = completed.entry(day).or_default();
            if !rows.iter().any(|(id, _)| id == &event.lesson_id) {
                rows.push((event.lesson_id, title));
            }
        }
    }

    let earliest = totals.keys().chain(completed.keys()).copied().fold(today, LocalDate::min);
    let start = earliest.add_days(1 - earliest.ymd().2 as i32);
    let last = target.map_or(today, |t| t.by.max(today));
    let end = last.add_days(1 - last.ymd().2 as i32).add_days(32);
    let end = end.add_days(-(end.ymd().2 as i32));
    let requested = LocalDate(selected());
    let date = if (start..=end).contains(&requested) { requested } else { today };
    let agenda_on = |day: LocalDate| usize::try_from(day.days_since(today)).ok().and_then(|i| plan.get(i));
    let count = |day| completed.get(&day).map_or(0, Vec::len);
    let months = calendar_months(
        start.0,
        end.0,
        |date| {
            let date = LocalDate(date);
            let (year, month, day) = date.ymd();
            (
                day == 1,
                usize::from(date.weekday().index()),
                format!("{} {year}", labels::month_short(&ui, month)),
            )
        },
        |date| {
            let day = LocalDate(date);
            let deadline = target.is_some_and(|t| t.by == day);
            let total = totals.get(&day).copied().unwrap_or_default();
            let done = count(day);
            let assigned = agenda_on(day).map_or(0, |agenda| agenda.portions.len());
            let completed = kabar_core::stats::heat_level(&total).max(done.min(4) as u8);
            let planned = if agenda_on(day).is_some() { assigned.clamp(1, 4) as u8 } else { 0 };
            let long_date = labels::long_date(&ui, day);
            let phase = if deadline { ui.t(M::TargetDay) } else { "" };
            let progress = text::progress(lang, assigned, done);
            CalendarDay {
                date,
                number: ui.num(day.ymd().2),
                label: format!("{long_date} · {phase} {progress}"),
                planned,
                completed,
                deadline,
            }
        },
    );
    let long_date = labels::long_date(&ui, date);
    let total = totals.get(&date).copied().unwrap_or_default();
    let done = count(date);
    let duration = labels::minutes(&ui, total.minutes);
    let summary = text::summary(lang, done, &duration, total.reviews);
    let target_day = ui.t(M::TargetDay);
    let practice_label = ui.t(M::Practice);
    let beyond_plan = ui.t(M::BeyondPlan);
    rsx! {
        StudyCalendar {
            title: ui.t(M::Title),
            months, weekdays: Weekday::ALL.map(|day| labels::weekday_short(lang, day).to_owned()),
            today: today.0, selected,
            previous: ui.t(M::Previous),
            next: ui.t(M::Next),
            today_label: ui.t(M::Today),
            planned_label: ui.t(M::Planned),
            completed_label: ui.t(M::Completed),
            deadline_label: ui.t(M::Deadline),
            h3 { {long_date}
                if let Some(target) = target.filter(|t| t.by == date) {
                    " · {target_day} "
                    bdi { "{target.level}" }
                }
            }
            if let Some(agenda) = agenda_on(date) {
                ul { class: "study-agenda-preview",
                    for part in &agenda.portions {
                        {
                            let (group, lesson, done) = &lessons[part.lesson];
                            let id = lesson.id.as_str();
                            let time = labels::minutes(&ui, part.minutes);
                            rsx! { li { key: "{id}",
                                button { class: "text-btn", onclick: move |_| ui.open_lesson(id.to_string()),
                                    bdi { "{lesson.title}" } }
                                small { "{group} · {time}" if *done { " ✓" } }
                            } }
                        }
                    }
                }
                if agenda.practice_minutes > 0 {
                    p { "{practice_label} · {labels::minutes(&ui, agenda.practice_minutes)}" }
                }
            }
            if date <= today {
                p { {summary} }
                if let Some(rows) = completed.get(&date) {
                    ul { class: "study-agenda-preview",
                        for (id, title) in rows {
                            li { key: "{id}", bdi { "{title}" } }
                        }
                    }
                }
            }
            if let Some(minutes) = plan.first().map(|agenda| agenda.unassigned_minutes).filter(|m| *m > 0) {
                p { class: "muted", "{beyond_plan} · {labels::minutes(&ui, minutes)}" }
            }
        }
    }
}
