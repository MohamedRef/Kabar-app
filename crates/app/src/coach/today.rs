//! *Rencana Hari Ini*: Kak Nur's greeting, "5, 15 or 30 minutes today?",
//! the plan with its progress, Pelita and Koin, snooze, and the end-of-day
//! recap. After a break (spec §6) Today opens on a welcome-back card instead:
//! the light plan and one Start button, no streak talk, until the first bit
//! of study.

use dioxus::prelude::*;
use kabar_core::coach::voice::Line;
use kabar_core::coach::{history, plan, recap, reminders, rewards};
use kabar_protocol::coach::{
    ActivityKind, CoachProfile, DailyPlan, LanguageFocus, LocalDate, PlanItem, PlanItemKind, PlanSize,
    ReminderStyle,
};

use super::widgets::{Nur, Progress, SizePicker, Stat};
use super::text::{self, T, tt};
use super::{Coach, build_plan, labels, logic, plan_for, say};
use crate::i18n::Msg;
use crate::portraits::Mood;
use crate::{Db, Screen, Ui, icons};

pub(crate) const COACH_CSS: crate::embedded::Css = crate::embedded::css!("assets/coach.css");

/// The lesson a lesson step opens: the unit's next lesson, or the Journey's
/// next one once that unit is finished. `None` when neither is known.
fn lesson_for(db: &Db, coach: Coach, unit_id: &str) -> Option<String> {
    let completed = db.0.borrow().completed_lessons().unwrap_or_default();
    crate::journey::next_lesson_in(unit_id, &completed).or_else(|| journey_next(db, coach, &completed))
}

/// The Journey's next lesson on the learner's own route.
pub(super) fn journey_next(db: &Db, coach: Coach, completed: &std::collections::BTreeSet<String>) -> Option<String> {
    let profile = coach.profile.peek().clone()?;
    let start = crate::journey::start_level(db, Some(&profile));
    let javanese = profile.focus == LanguageFocus::Javanese;
    let unit = crate::journey::current_unit(completed, start.as_deref(), javanese)?;
    crate::journey::next_lesson_in(&unit, completed)
}

/// Opens what a plan item asks for: a lesson item goes straight into the
/// unit's next lesson, or the Journey's next one once that unit is finished
/// (the Journey itself only when neither is known).
pub(super) fn start_item(ui: Ui, db: &Db, coach: Coach, kind: &PlanItemKind) {
    let screen = match kind {
        PlanItemKind::Review { .. } => Screen::Review,
        PlanItemKind::Lesson { unit_id } => {
            if let Some(lesson) = lesson_for(db, coach, unit_id) {
                ui.open_lesson(lesson);
                return;
            }
            Screen::Journey
        }
        PlanItemKind::WeakSpot { .. } => Screen::Journey,
        PlanItemKind::Reading => Screen::Reader,
    };
    let mut current = ui.screen;
    current.set(screen);
}

/// The icon a plan step shows: where it goes.
pub(super) fn item_icon(kind: &PlanItemKind) -> Element {
    icons::nav(match kind {
        PlanItemKind::Review { .. } => Screen::Review,
        PlanItemKind::Lesson { .. } => Screen::Journey,
        PlanItemKind::WeakSpot { .. } => Screen::LevelCheck,
        PlanItemKind::Reading => Screen::Reader,
    })
}

/// Rebuilds and saves today's plan at the size the learner just picked.
fn pick_size(db: &Db, coach: Coach, profile: &CoachProfile, today: LocalDate, size: PlanSize) {
    let events = coach.events.peek().clone();
    let sched = crate::schedule::scheduler(&coach, &db.0.borrow());
    let due = crate::review::due_words(db, &sched, coach.now()).len() as u32;
    let fresh = build_plan(db, due, profile, &events, today, size, true);
    if let Err(e) = db.0.borrow_mut().save_plan(&fresh) {
        crate::diag::report!("saving today's plan failed: {e}");
    }
    let mut rev = coach.plan_rev;
    rev += 1;
}

#[component]
pub fn Today() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let Some(profile) = coach.profile.read().clone() else {
        return rsx! {};
    };
    let _rev = coach.plan_rev.read();
    let today = coach.today();
    let events = coach.events.read().clone();
    let plan = plan_for(&db, &profile, &events, today);
    let stats = history::day(&events, today);
    let progress = plan::progress(&plan, &stats);
    let goal = u32::from(plan.size.minutes());
    // Back after a break and nothing done yet today: the welcome-back card.
    let welcome = plan.comeback && !stats.studied();
    let lit_today = rewards::pelita(&events, today).lit_today;

    let greeting = if progress.goal_met {
        Line::PlanAllDone
    } else if plan.comeback && lit_today {
        Line::PelitaRelit
    } else if plan.comeback {
        Line::PlanComeback
    } else {
        Line::PlanMorning
    };
    let break_at = plan::break_after(&plan);
    let minutes = ui.num(goal);
    let (count, topic) = labels::plan_voice_args(&ui, &plan.items);
    let args = [
        ("name", profile.name.as_str()),
        ("minutes", minutes.as_str()),
        ("count", count.as_str()),
        ("topic", topic.as_str()),
    ];
    // The one clear next step: the first thing on the plan not done yet.
    let next = (0..plan.items.len()).find(|&i| !progress.done.get(i).copied().unwrap_or(false));
    let recap_ready =
        recap::recap_ready(progress.goal_met, stats.studied(), coach.time_of_day(), &profile.study_times);

    rsx! {
        style { {COACH_CSS.text()} }
        crate::update::view::UpdateBanner {}
        // Wide screens: the plan, with the side cards (recap, placement,
        // Kabar Pagi) in a second column when there are any.
        div { class: "coach-grid today-grid",
            div { class: "today-main",
                if welcome {
                    WelcomeBack { profile: profile.clone(), plan: plan.clone(), today }
                } else {
                    div { class: "today-size",
                        span { class: "muted small", "{tt(ui.lang(), T::HowLong)}" }
                        SizePicker {
                            value: plan.size,
                            onpick: {
                                let db = db.clone();
                                let profile = profile.clone();
                                move |size| pick_size(&db, coach, &profile, today, size)
                            },
                        }
                    }
                    Hero {
                        plan: plan.clone(),
                        next,
                        minutes_done: progress.minutes_done,
                        // What's left is the steps not done yet, whatever the clock says.
                        left: plan
                            .items
                            .iter()
                            .zip(&progress.done)
                            .filter(|(_, d)| !**d)
                            .map(|(i, _)| u32::from(i.minutes))
                            .sum::<u32>(),
                    }
                    if plan.items.len() > 1 {
                        section { class: "today-rest", aria_labelledby: "today-rest-title",
                            h2 { id: "today-rest-title", class: "section-title", "{tt(ui.lang(), T::ThenRows)}" }
                            ul { class: "plan",
                                for (i , item) in plan.items.iter().enumerate() {
                                    if Some(i) != next {
                                        PlanRow {
                                            key: "{item.id}",
                                            item: item.clone(),
                                            done: progress.done.get(i).copied().unwrap_or(false),
                                            today,
                                            break_text: (break_at == Some(i))
                                                .then(|| say(&ui, Line::PlanBreak, today, &args)),
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Kak Nur's one line opens her page (report, achievements, Warung).
                    button {
                        class: "nur-card tap",
                        aria_label: "{tt(ui.lang(), T::NurOpens)}",
                        onclick: move |_| {
                            let mut screen = ui.screen;
                            screen.set(Screen::Coach);
                        },
                        Nur { text: say(&ui, greeting, today, &args) }
                        span { class: "chev", aria_hidden: "true" }
                    }
                    Snooze { profile: profile.clone(), goal_met: progress.goal_met }
                }
                RewardsBar { today, welcome }
            }
            div { class: "today-side",
                if recap_ready {
                    Recap { profile: profile.clone(), plan: plan.clone(), today }
                }
                crate::lesson::PlacementInvite {}
                crate::play::PagiTodayCard {}
                crate::stats::TodayCards {}
            }
        }
    }
}

/// The hero: the plan's next step, today's minutes, and one button that
/// opens that step. A step with no record of its own (a reading, a weak
/// spot) can be ticked here too, with a quiet text button that looks
/// nothing like Start. Once the plan is done the hero offers one more
/// lesson from the Journey.
#[component]
fn Hero(plan: DailyPlan, next: Option<usize>, minutes_done: u32, left: u32) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let lang = ui.lang();
    let goal = u32::from(plan.size.minutes());
    let step = next.and_then(|i| plan.items.get(i)).cloned();
    let time = rsx! {
        Progress { done: minutes_done, goal, label: ui.t(Msg::TodayProgress).to_string() }
        p { class: "hero-count small",
            bdi { "{labels::minutes(&ui, minutes_done)} / {labels::minutes(&ui, goal)}" }
        }
    };
    let Some(step) = step else {
        let completed = db.0.borrow().completed_lessons().unwrap_or_default();
        let more = journey_next(&db, coach, &completed);
        let title = more.as_deref().and_then(crate::journey::lesson_title);
        return rsx! {
            section { class: "card today-hero done", aria_labelledby: "hero-title",
                p { class: "hero-eyebrow", "{tt(lang, T::PlanEyebrow)}" }
                h2 { id: "hero-title", class: "hero-title", "{tt(lang, T::AllDoneTitle)}" }
                {time}
                button {
                    class: "primary hero-start",
                    onclick: move |_| match &more {
                        Some(id) => ui.open_lesson(id.clone()),
                        None => {
                            let mut screen = ui.screen;
                            screen.set(Screen::Journey);
                        }
                    },
                    "{tt(lang, T::OneMoreLesson)}"
                    if let Some(t) = title {
                        " · "
                        bdi { lang: "id", dir: "ltr", "{t}" }
                    }
                }
            }
        };
    };
    let (kind_msg, detail) = labels::plan_item(&ui, &step.kind);
    // A lesson step names the lesson it opens; its unit is the subtitle.
    let lesson = match &step.kind {
        PlanItemKind::Lesson { unit_id } => {
            lesson_for(&db, coach, unit_id).and_then(|id| crate::journey::lesson_title(&id))
        }
        _ => None,
    };
    let started = minutes_done > 0;
    let label = if started {
        format!("{} · {}", tt(lang, T::ContinuePlan), text::left(lang, &labels::minutes(&ui, left)))
    } else {
        format!("{} · {}", tt(lang, T::StartPlan), labels::minutes(&ui, goal))
    };
    // Lessons and reviews tick themselves; the rest can be ticked by hand.
    let manual = matches!(step.kind, PlanItemKind::Reading | PlanItemKind::WeakSpot { .. });
    let seconds = u32::from(step.minutes) * 60;
    let kind = step.kind.clone();
    let id = step.id.clone();
    rsx! {
        section { class: "card today-hero", aria_labelledby: "hero-title",
            p { class: "hero-eyebrow",
                span { class: "hero-icon", aria_hidden: "true", {item_icon(&step.kind)} }
                "{tt(lang, T::PlanEyebrow)}"
            }
            if let Some(title) = lesson {
                h2 { id: "hero-title", class: "hero-title",
                    bdi { lang: "id", dir: "ltr", "{title}" }
                }
                p { class: "hero-sub",
                    "{tt(lang, T::LessonIn)} "
                    if let Some(d) = &detail { bdi { "{d}" } }
                    " · {labels::minutes(&ui, u32::from(step.minutes))}"
                }
            } else {
                h2 { id: "hero-title", class: "hero-title", "{ui.t(kind_msg)}" }
                p { class: "hero-sub",
                    if let Some(d) = &detail {
                        bdi { lang: "id", "{d}" }
                        " · "
                    }
                    "{labels::minutes(&ui, u32::from(step.minutes))}"
                }
            }
            {time}
            button {
                class: "primary hero-start",
                onclick: {
                    let db = db.clone();
                    move |_| start_item(ui, &db, coach, &kind)
                },
                "{label}"
                span { class: "chev hero-arrow", aria_hidden: "true" }
            }
            if manual {
                button {
                    class: "quiet hero-mark",
                    onclick: {
                        let db = db.clone();
                        move |_| {
                            coach.record(&db, ActivityKind::PlanItemDone { item_id: id.clone() }, seconds);
                        }
                    },
                    "{tt(lang, T::MarkStepDone)}"
                }
            }
        }
    }
}

/// The welcome-back card: Kak Nur's warm hello, the light plan, one Start
/// button for its first item, and a quiet way to pick a bigger plan.
/// Never a streak count or a number of days away.
#[component]
fn WelcomeBack(profile: CoachProfile, plan: DailyPlan, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let minutes = ui.num(plan.size.minutes());
    let args = [("name", profile.name.as_str()), ("minutes", minutes.as_str())];
    let first = plan.items.first().map(|i| i.kind.clone());
    rsx! {
        section { class: "card today-hero welcome-back", aria_labelledby: "welcome-back-title",
            h2 { id: "welcome-back-title", "{ui.t(Msg::WelcomeBackTitle)}" }
            Nur { text: say(&ui, Line::PlanComeback, today, &args) }
            h3 { class: "welcome-back-plan-title",
                "{ui.t(Msg::WelcomeBackPlan)} · {labels::minutes(&ui, u32::from(plan.size.minutes()))}"
            }
            ul { class: "plan",
                for item in plan.items.iter() {
                    PlanRow {
                        key: "{item.id}",
                        item: item.clone(),
                        done: false,
                        today,
                        tick: false,
                        break_text: None,
                    }
                }
            }
            if let Some(kind) = first {
                button {
                    class: "primary welcome-back-start",
                    onclick: {
                        let db = db.clone();
                        move |_| start_item(ui, &db, coach, &kind)
                    },
                    "{ui.t(Msg::WelcomeBackStart)}"
                }
            }
            details { class: "welcome-back-more",
                summary { "{ui.t(Msg::WelcomeBackBigger)}" }
                div { class: "row", role: "group",
                    for size in [PlanSize::Steady, PlanSize::Deep] {
                        button {
                            key: "{size:?}",
                            class: "quiet-solid",
                            onclick: {
                                let db = db.clone();
                                let profile = profile.clone();
                                move |_| pick_size(&db, coach, &profile, today, size)
                            },
                            {labels::minutes(&ui, u32::from(size.minutes()))}
                        }
                    }
                }
            }
        }
    }
}

/// Pelita (the streak lamp) and Koin. On a welcome-back morning the lamp is
/// shown ready to glow, without a count.
#[component]
fn RewardsBar(today: LocalDate, welcome: bool) -> Element {
    let ui = use_context::<Ui>();
    let mut screen = ui.screen;
    let coach = use_context::<Coach>();
    let events = coach.events.read();
    let pelita = rewards::pelita(&events, today);
    let koin = rewards::koin_balance(&events);
    let lamp_class = match (pelita.lit_today, welcome) {
        (true, _) => "reward lit",
        (false, true) => "reward ready",
        (false, false) => "reward",
    };
    rsx! {
        // Pelita opens Stats (the study days behind it); Koin opens Kak Nur's
        // screen, where the Warung spends it.
        div { class: "rewards-bar",
            button { class: "{lamp_class} tap", onclick: move |_| crate::stats::open(ui, Screen::Stats),
                span { class: "reward-icon", aria_hidden: "true", {icons::lamp()} }
                span { class: "latin", bdi { "Pelita" } }
                if welcome {
                    span { class: "small", "{ui.t(Msg::PelitaReady)}" }
                } else {
                    strong { "{ui.num(pelita.streak)}" }
                }
                if !welcome && pelita.freezes > 0 {
                    span { class: "muted small", "· {ui.t(Msg::Freezes)} {ui.num(pelita.freezes)}" }
                }
            }
            button { class: "reward tap", onclick: move |_| screen.set(Screen::Coach),
                span { class: "reward-icon", aria_hidden: "true", {icons::coin()} }
                span { class: "latin", bdi { "Koin" } }
                strong { "{ui.num(koin)}" }
            }
        }
    }
}

/// One plan step below the hero. The row itself opens the step; the small
/// round check beside it ticks it off by hand (or unticks it), so starting
/// and marking done never look like the same button. `tick: false` drops
/// the check (the welcome-back card). `break_text`: Kak Nur's break offer,
/// shown after this step when a 30-minute plan breaks here.
#[component]
fn PlanRow(
    item: PlanItem,
    done: bool,
    today: LocalDate,
    #[props(default = true)] tick: bool,
    break_text: Option<String>,
) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let (title, detail) = labels::plan_item(&ui, &item.kind);
    let kind = item.kind.clone();
    let id = item.id.clone();
    let seconds = u32::from(item.minutes) * 60;
    let text_db = db.clone();
    rsx! {
        li { class: if done { "plan-item done" } else { "plan-item" },
            button {
                class: "plan-text tap tap-plain",
                onclick: move |_| start_item(ui, &text_db, coach, &kind),
                span { class: "plan-icon", aria_hidden: "true", {item_icon(&item.kind)} }
                span { class: "plan-words",
                    strong { "{ui.t(title)}" }
                    if let Some(detail) = detail {
                        span { class: "muted small plan-detail", bdi { lang: "id", dir: "ltr", "{detail}" } }
                    }
                }
                span { class: "muted small plan-min", {labels::minutes(&ui, u32::from(item.minutes))} }
                span { class: "chev", aria_hidden: "true" }
            }
            if tick {
                button {
                    class: "check",
                    role: "checkbox",
                    aria_checked: if done { "true" } else { "false" },
                    aria_label: "{ui.t(Msg::MarkDone)}: {ui.t(title)}",
                    title: "{ui.t(Msg::MarkDone)}",
                    onclick: move |_| {
                        if done {
                            let ids: Vec<String> = logic::done_events(&coach.events.peek(), today, &id)
                                .into_iter()
                                .map(str::to_string)
                                .collect();
                            coach.retract(&db, &ids);
                        } else {
                            coach.record(&db, ActivityKind::PlanItemDone { item_id: id.clone() }, seconds);
                        }
                    },
                    if done { "✓" }
                }
            }
        }
        if let Some(text) = break_text {
            li { class: "plan-break", role: "note",
                strong { "{ui.t(Msg::PlanBreakTitle)}" }
                p { class: "muted small", "{text}" }
            }
        }
    }
}

/// One tap to quiet reminders for an hour, when one could still come today.
#[component]
fn Snooze(profile: CoachProfile, goal_met: bool) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    if profile.reminder_style == ReminderStyle::Off || goal_met {
        return rsx! {};
    }
    let now = coach.now();
    let state = coach.reminders.read().clone();
    if let Some(until) = state.snoozed_until_ms.filter(|u| *u > now) {
        let at = labels::time(&ui, super::clock::time_of_day(until, profile.utc_offset_min));
        return rsx! {
            p { class: "muted small", "{ui.t(Msg::SnoozedUntil)} " bdi { "{at}" } }
        };
    }
    rsx! {
        button {
            class: "quiet",
            onclick: move |_| coach.save_reminders(&db, reminders::snooze(&state, coach.now(), 60)),
            "{ui.t(Msg::Snooze)}"
        }
    }
}

/// The 30-second end-of-day recap.
#[component]
fn Recap(profile: CoachProfile, plan: DailyPlan, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let db = use_context::<Db>();
    let events = coach.events.read().clone();
    // Cards due by tomorrow morning size tomorrow's first step.
    let sched = crate::schedule::scheduler(&coach, &db.0.borrow());
    let due = crate::review::due_words(&db, &sched, coach.now() + 86_400_000).len() as u32;
    let tomorrow = build_plan(&db, due, &profile, &events, today.add_days(1), profile.daily_size, false);
    let tomorrow_first = tomorrow.items.first().map(|i| i.kind.clone());
    let r = recap::day_recap(&events, today, plan.size.minutes(), plan.items.len(), tomorrow_first);

    // Voice placeholders: text already localized, numbers in the UI's digits,
    // and empty when they don't apply (say then picks a variant without them).
    let minutes = ui.num(r.minutes);
    let words = if r.new_words.is_empty() { String::new() } else { ui.num(r.new_words.len() as u64) };
    // Steps count as the plan shows them: ticked off, or visibly done (a
    // finished lesson ticks its row without a tap).
    let done = plan::progress(&plan, &history::day(&events, today)).done;
    let steps = done.iter().filter(|d| **d).count() as u32;
    let worked_on = plan
        .items
        .iter()
        .zip(&done)
        .find(|(i, d)| **d && !matches!(i.kind, PlanItemKind::Review { .. }))
        .map(|(i, _)| labels::plan_item_text(&ui, &i.kind))
        .unwrap_or_default();
    let base = [
        ("name", profile.name.as_str()),
        ("minutes", minutes.as_str()),
        ("words", words.as_str()),
        ("topic", worked_on.as_str()),
    ];
    let headline = say(&ui, if r.goal_met { Line::RecapGoalMet } else { Line::RecapSomeStudy }, today, &base);
    let remember = r.remember.as_ref().map(|f| {
        let topic = format!("{} · \u{2068}{}\u{2069}", ui.t(labels::skill(f.skill)), f.topic);
        say(&ui, Line::RecapRemember, today, &[("topic", topic.as_str())])
    });
    let next = r.tomorrow.as_ref().map(|first| {
        let (count, topic) = labels::plan_voice_args(&ui, &tomorrow.items);
        let topic = if topic.is_empty() { labels::plan_item_text(&ui, first) } else { topic };
        say(&ui, Line::RecapTomorrow, today, &[("topic", topic.as_str()), ("count", count.as_str())])
    });

    rsx! {
        div { class: "card recap",
            h2 { "{ui.t(Msg::RecapTitle)}" }
            Nur { mood: Mood::Laughing, text: headline }
            div { class: "stats-row",
                Stat { value: labels::minutes(&ui, r.minutes), label: Msg::StatTime }
                Stat { value: ui.num(steps.max(r.items_done)), label: Msg::StatSteps }
                Stat { value: ui.num(r.reviews), label: Msg::StatReviews, to: Screen::Review }
                Stat { value: ui.num(r.new_words.len() as u32), label: Msg::StatNewWords }
            }
            if !r.new_words.is_empty() {
                p { class: "words", lang: "id",
                    for w in r.new_words.iter() {
                        bdi { key: "{w}", class: "word", dir: "ltr", "{w}" }
                    }
                }
            }
            if let Some(text) = remember {
                h3 { "{ui.t(Msg::RecapRemember)}" }
                p { "{text}" }
            }
            if let Some(text) = next {
                h3 { "{ui.t(Msg::RecapTomorrow)}" }
                p { "{text}" }
            }
        }
    }
}
