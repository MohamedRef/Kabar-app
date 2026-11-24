//! Lessons leading up to the learner's target level, alongside the coach's daily activities.
mod calendar;
use super::{Coach, labels};
use crate::{Db, Ui};
pub(crate) use calendar::LessonCalendar;
use dioxus::prelude::*;
use kabar_core::assignments::{DayBudget, LessonWork, agendas};
use kabar_core::locale::UiLanguage;
use kabar_protocol::coach::{CoachProfile, LocalDate};
use study_ui::{AgendaOption, AgendaSelect, CompactLessonCards, LessonAgendaCard, LessonCardItem};

type LessonEntry = (String, &'static kabar_core::journey::Lesson, bool);

fn assignments(
    db: &Db,
    coach: &Coach,
    profile: &CoachProfile,
    today_minutes: u32,
    lang: UiLanguage,
) -> (Vec<LessonEntry>, Vec<kabar_core::assignments::Agenda>) {
    let Some(target) = &profile.target else { return Default::default() };
    let today = coach.today();
    let span = target.by.days_since(today);
    if span <= 0 {
        return Default::default();
    }
    let completed = crate::persistence::read_or("lesson completions", db.0.borrow().lesson_completions());
    let mut first = std::collections::HashMap::<String, u64>::new();
    for event in completed {
        first.entry(event.lesson_id).and_modify(|t| *t = (*t).min(event.at_ms)).or_insert(event.at_ms);
    }
    let start = crate::journey::start_level(db, Some(profile));
    let catalog = crate::journey::target_lessons(&target.level, start.as_deref(), lang);
    let mut work = Vec::new();
    let mut rows = Vec::new();
    for (group, l) in catalog {
        let done = first.get(&l.id).map(|t| coach.zone().date(*t));
        if done.is_some_and(|d| d < today) {
            continue;
        }
        // The catalog already rotates units within each curriculum level.
        work.push(LessonWork { group: String::new(), minutes: l.minutes.max(1) });
        rows.push((group, l, done.is_some()));
    }
    let span = span as usize;
    let regular = u32::from(profile.daily_size.minutes());
    let reserved_days = if span > 1 { span.div_ceil(5).min(7) } else { 0 };
    // Keep the calendar allocation bounded for imported distant dates.
    let horizon = span.min(3660);
    let study_days = span - reserved_days;
    let budgets: Vec<_> = (0..horizon)
        .map(|i| DayBudget {
            minutes: if i == 0 { today_minutes } else { regular },
            new_lessons: i < study_days,
            practice: true,
        })
        .collect();
    let assignments = agendas(&work, &budgets, |index| rows[index].2);
    (rows, assignments)
}

#[component]
pub(super) fn Agenda(profile: CoachProfile, today_minutes: u32) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut selected = use_signal(|| 0usize);
    let mut quiz = use_signal(|| None::<kabar_protocol::lesson::LessonContent>);
    let mut unavailable = use_signal(|| false);
    let today = coach.today();
    let lang = ui.lang();
    let words = |eg, ar, en| match lang {
        UiLanguage::ArEg => eg,
        UiLanguage::Ar => ar,
        UiLanguage::En => en,
    };
    if let Some(lesson) = quiz() {
        return rsx! {crate::lesson::PracticeQuiz { lesson, on_close: move |_| quiz.set(None) }};
    }
    let _events = coach.events.read();
    let (rows, calendar) = assignments(&db, &coach, &profile, today_minutes, lang);
    let Some(agenda) = calendar.get(selected().min(calendar.len().saturating_sub(1))) else { return rsx! {} };
    let horizon = calendar.len();
    let day_index = agenda.index;
    let practice = agenda.practice_minutes;
    let title = words("واجب الدروس", "دروس اليوم", "Lesson assignments");
    let test = words("اختبار الدرس", "اختبار الدرس", "Lesson test");
    let revise = words("مراجعة وتمارين", "مراجعة وتدريب", "Review and practice");
    let day_label = words("يوم الخطة", "يوم الخطة", "Study day");
    let excess_label = words(
        "وقت الدروس اللي لسه محتاج مكان في الخطة",
        "وقت الدروس الذي يتجاوز السعة",
        "Lesson time beyond your budget",
    );
    let empty_label = words(
        "مفيش تمارين متاحة للدرس ده",
        "لا تتوفر تمارين لهذا الدرس",
        "No exercises available for this lesson",
    );
    let left = agenda.unassigned_minutes;
    let left_label = words("دروس متبقية", "دروس متبقية", "lessons left");
    let day_options = (0..horizon)
        .map(|value| {
            let date = labels::long_date(&ui, LocalDate(today.0 + value as i32));
            let label = if value == 0 {
                format!("{} · {date}", words("النهارده", "اليوم", "Today"))
            } else {
                date
            };
            AgendaOption { value, label }
        })
        .collect();
    rsx! {
        LessonAgendaCard { class: "coach-agenda", title,
            overview: rsx! {
                p { class: "muted",
                    "{ui.num(agenda.lessons_left)} {left_label} · {labels::minutes(&ui, agenda.minutes_left)}"
                }
            },
            controls: rsx! {
                if horizon > 1 {
                    AgendaSelect { label: day_label, value: day_index, options: day_options,
                        on_select: move |i| {
                            selected.set(i);
                            unavailable.set(false);
                        },
                    }
                }
            },
            if !agenda.portions.is_empty() {
                CompactLessonCards {
                    items: agenda.portions.iter().map(|part| {
                        let (group, lesson, done) = &rows[part.lesson];
                        let (unit, reference) = lesson.id.rsplit_once('.').unwrap_or((&lesson.id, ""));
                        LessonCardItem {
                            id: lesson.id.clone(), group: unit.to_owned(), group_title: group.clone(),
                            title: lesson.title.clone(), minutes: ui.num(part.minutes),
                            minute_unit: if ui.lang() == UiLanguage::En { "min" } else { "د" }.into(),
                            duration: labels::minutes(&ui, part.minutes),
                            reference: reference.to_owned(), note: String::new(), done: *done, order: part.lesson,
                        }
                    }).collect::<Vec<_>>(),
                    more_label: words("باقي الدروس", "باقي الدروس", "More lessons"),
                    done_label: words("خلصته", "مكتمل", "Completed"),
                    test_label: test.to_owned(),
                    on_open: move |id: String| ui.open_lesson(id),
                    on_test: move |id: String| {
                        let parsed = crate::lesson::lesson_content(&id)
                            .and_then(|s| kabar_core::lesson::parse_lesson(&s).ok())
                            .filter(|lesson| !lesson.exercises.is_empty());
                        unavailable.set(parsed.is_none());
                        quiz.set(parsed);
                    },
                }
            }
            if practice > 0 { p { "{revise} · {labels::minutes(&ui,practice)}" } }
            if left > 0 { p { class: "muted", "{excess_label} · {labels::minutes(&ui,left)}" } }
            if unavailable() { p { role: "status", "{empty_label}" } }
        }
    }
}
