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
use super::{Coach, build_plan, labels, logic, plan_for, say};
use crate::i18n::Msg;
use crate::portraits::Mood;
use crate::{Db, Screen, Ui, icons};

/// Where a plan item's "start" button goes.
/// Opens what a plan item asks for: a lesson item goes straight into the
/// unit's next lesson, or the Journey's next one once that unit is finished
/// (the Journey itself only when neither is known).
fn start_item(ui: Ui, db: &Db, coach: Coach, kind: &PlanItemKind) {
    let screen = match kind {
        PlanItemKind::Review { .. } => Screen::Review,
        PlanItemKind::Lesson { unit_id } => {
            let completed = db.0.borrow().completed_lessons().unwrap_or_default();
            let next = crate::journey::next_lesson_in(unit_id, &completed).or_else(|| {
                let profile = coach.profile.peek().clone()?;
                let start = crate::journey::start_level(db, Some(&profile));
                let javanese = profile.focus == LanguageFocus::Javanese;
                let unit = crate::journey::current_unit(&completed, start.as_deref(), javanese)?;
                crate::journey::next_lesson_in(&unit, &completed)
            });
            if let Some(lesson) = next {
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
        RewardsBar { today, welcome }
        crate::update::view::UpdateBanner {}
        // Wide screens: the plan, with the side cards (recap, placement,
        // Kabar Pagi) in a second column when there are any.
        div { class: "coach-grid today-grid",
            div {
                if welcome {
                    WelcomeBack { profile: profile.clone(), plan: plan.clone(), today }
                } else {
                div { class: "card",
                    h2 { lang: "id", bdi { dir: "ltr", "{ui.t(Msg::TodayTitle)}" } }
                    Nur { text: say(&ui, greeting, today, &args) }
                    p { class: "muted", "{say(&ui, Line::PlanAskSize, today, &args)}" }
                    SizePicker {
                        value: plan.size,
                        onpick: {
                            let db = db.clone();
                            let profile = profile.clone();
                            move |size| pick_size(&db, coach, &profile, today, size)
                        },
                    }
                    Progress {
                        done: progress.minutes_done,
                        goal,
                        label: ui.t(Msg::TodayProgress).to_string(),
                    }
                    p { class: "muted small",
                        "{labels::minutes(&ui, progress.minutes_done)} / {labels::minutes(&ui, goal)}"
                        if progress.goal_met {
                            " · "
                            span { class: "ok", "{ui.t(Msg::GoalMet)}" }
                        }
                    }
                    ul { class: "plan",
                        for (i , item) in plan.items.iter().enumerate() {
                            PlanRow {
                                key: "{item.id}",
                                item: item.clone(),
                                done: progress.done.get(i).copied().unwrap_or(false),
                                today,
                                show_start: true,
                                next: next == Some(i),
                                break_text: (break_at == Some(i))
                                    .then(|| say(&ui, Line::PlanBreak, today, &args)),
                            }
                        }
                    }
                    Snooze { profile: profile.clone(), goal_met: progress.goal_met }
                }
                }
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
        section { class: "card welcome-back", aria_labelledby: "welcome-back-title",
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
                        show_start: false,
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

/// One plan item. `show_start`: its own Start button (the welcome-back card
/// has a single one instead). `break_text`: Kak Nur's break offer, shown
/// after this item when a 30-minute plan breaks here.
#[component]
fn PlanRow(
    item: PlanItem,
    done: bool,
    today: LocalDate,
    show_start: bool,
    #[props(default)] next: bool,
    break_text: Option<String>,
) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let (title, detail) = labels::plan_item(&ui, &item.kind);
    let kind = item.kind.clone();
    let id = item.id.clone();
    let seconds = u32::from(item.minutes) * 60;
    let (text_db, text_kind) = (db.clone(), kind.clone());
    rsx! {
        li { class: if done { "plan-item done" } else if next { "plan-item next" } else { "plan-item" },
            button {
                class: "check",
                role: "checkbox",
                aria_checked: if done { "true" } else { "false" },
                aria_label: "{ui.t(Msg::MarkDone)}",
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
            // The row's text starts it too, not only the Start button.
            button {
                class: "plan-text tap tap-plain",
                onclick: move |_| start_item(ui, &text_db, coach, &text_kind),
                strong { "{ui.t(title)}" }
                if let Some(detail) = detail {
                    span { class: "muted", " · " bdi { lang: "id", dir: "ltr", "{detail}" } }
                }
                div { class: "muted small", {labels::minutes(&ui, u32::from(item.minutes))} }
            }
            if !done && show_start {
                button {
                    class: if next { "primary" } else { "" },
                    onclick: {
                        let db = db.clone();
                        move |_| start_item(ui, &db, coach, &kind)
                    },
                    "{ui.t(Msg::Start)}"
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
