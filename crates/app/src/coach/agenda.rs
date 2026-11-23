//! Lessons leading up to the learner's target level, alongside the coach's daily activities.
use super::{Coach, labels};
use crate::{Db, Ui};
use dioxus::prelude::*;
use kabar_core::assignments::{LessonWork, schedule};
use kabar_core::locale::UiLanguage;
use kabar_protocol::coach::{CoachProfile, LocalDate};

#[component]
pub(super) fn Agenda(profile: CoachProfile, today_minutes: u32) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut selected = use_signal(|| 0usize);
    let mut quiz = use_signal(|| None::<kabar_protocol::lesson::LessonContent>);
    let mut unavailable = use_signal(|| false);
    let Some(target) = &profile.target else { return rsx! {} };
    let today = coach.today();
    let span = target.by.0.saturating_sub(today.0);
    if span <= 0 {
        return rsx! {};
    }
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
    let completed = crate::persistence::read_or("lesson completions", db.0.borrow().lesson_completions());
    let mut first = std::collections::HashMap::<String, u64>::new();
    for event in completed {
        first.entry(event.lesson_id).and_modify(|t| *t = (*t).min(event.at_ms)).or_insert(event.at_ms);
    }
    let position = db.0.borrow().position().ok().flatten();
    let start = crate::journey::start_level(&db, Some(&profile));
    let catalog = crate::journey::target_lessons(&target.level, start.as_deref(), lang);
    let mut work = Vec::new();
    let mut rows = Vec::new();
    for (group, l) in catalog {
        let done = first.get(&l.id).map(|t| coach.zone().date(*t));
        if done.is_some_and(|d| d < today) {
            continue;
        }
        let spent = position
            .as_ref()
            .filter(|p| p.lesson_id == l.id)
            .and_then(|p| p.progress.as_ref())
            .map_or(0, |p| (p.elapsed_ms / 60_000).min(u64::from(l.minutes)) as u32);
        // The catalog already rotates units within each curriculum level.
        work.push(LessonWork { group: String::new(), minutes: l.minutes.saturating_sub(spent).max(1) });
        rows.push((group, l, done.is_some()));
    }
    let span = span as usize;
    let day_index = selected().min(span - 1);
    let regular = u32::from(profile.daily_size.minutes());
    let reserved_days = if span > 1 { span.div_ceil(5).min(7) } else { 0 };
    // Keep the calendar allocation bounded for imported distant dates.
    let horizon = span.min(3660);
    let day_index = day_index.min(horizon - 1);
    let study_days = span - reserved_days;
    let capacity: Vec<_> = (0..horizon)
        .map(|i| if i >= study_days { 0 } else { (if i == 0 { today_minutes } else { regular }) * 2 / 3 })
        .collect();
    let schedule = schedule(&work, &capacity);
    let card = &schedule.days[day_index];
    let total = if day_index == 0 { today_minutes } else { regular };
    let practice = total.saturating_sub(card.iter().map(|p| p.minutes).sum::<u32>());
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
    let left = schedule.unassigned_minutes;
    rsx! {
        crate::ui::Card { class: "coach-agenda",
            h2 { "{title}" }
            label { "{day_label} "
                select { value: "{day_index}", onchange: move |e| if let Ok(i) = e.value().parse() { selected.set(i); unavailable.set(false); },
                    for i in 0..horizon { option { value: "{i}", selected: day_index == i, "{labels::long_date(&ui,LocalDate(today.0+i as i32))}" } }
                }
            }
            for part in card {
                {
                    let (group,l,done) = &rows[part.lesson];
                    let id = l.id.as_str();
                    let time = labels::minutes(&ui,part.minutes);
                    rsx! { div { key: "{id}", class: "agenda-row",
                        button { class: "text-btn", onclick: move |_| ui.open_lesson(id.to_string()), "{l.title}" }
                        span { "{group} · {time}" if *done { " ✓" } }
                        button { class: "secondary", onclick: move |_| {
                            let parsed = crate::lesson::lesson_content(id).and_then(|s| kabar_core::lesson::parse_lesson(&s).ok()).filter(|l| !l.exercises.is_empty());
                            unavailable.set(parsed.is_none());
                            quiz.set(parsed);
                        }, "{test}" }
                    }}
                }
            }
            if practice > 0 { p { "{revise} · {labels::minutes(&ui,practice)}" } }
            if left > 0 { p { class: "muted", "{excess_label} · {labels::minutes(&ui,left)}" } }
            if unavailable() { p { role: "status", "{empty_label}" } }
        }
    }
}
