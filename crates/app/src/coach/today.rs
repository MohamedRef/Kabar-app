//! *Rencana Hari Ini*: Kak Nur's greeting, "5, 15 or 30 minutes today?",
//! the plan with its progress, Pelita and Koin, snooze, and the end-of-day
//! recap.

use dioxus::prelude::*;
use kabar_core::coach::voice::Line;
use kabar_core::coach::{history, plan, recap, reminders, rewards};
use kabar_protocol::coach::{
    ActivityKind, CoachProfile, DailyPlan, LocalDate, PlanItem, PlanItemKind, ReminderStyle,
};

use super::widgets::{Nur, Progress, SizePicker, Stat};
use super::{Coach, build_plan, labels, logic, plan_for, say};
use crate::i18n::Msg;
use crate::{Db, Screen, Ui, icons};

/// Where a plan item's "start" button goes.
fn screen_for(kind: &PlanItemKind) -> Screen {
    match kind {
        PlanItemKind::Review { .. } => Screen::Review,
        PlanItemKind::Lesson { .. } | PlanItemKind::WeakSpot { .. } => Screen::Journey,
        PlanItemKind::Reading => Screen::Reader,
    }
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

    let greeting = if plan.comeback {
        Line::PlanComeback
    } else if progress.goal_met {
        Line::PlanAllDone
    } else {
        Line::PlanMorning
    };
    let minutes = ui.num(goal);
    let (count, topic) = labels::plan_voice_args(&ui, &plan.items);
    let args = [
        ("name", profile.name.as_str()),
        ("minutes", minutes.as_str()),
        ("count", count.as_str()),
        ("topic", topic.as_str()),
    ];
    let recap_ready =
        recap::recap_ready(progress.goal_met, stats.studied(), coach.time_of_day(), &profile.study_times);

    rsx! {
        RewardsBar { today }
        div { class: "coach-grid",
            div {
                div { class: "card",
                    h2 { lang: "id", bdi { "{ui.t(Msg::TodayTitle)}" } }
                    Nur { text: say(&ui, greeting, today, &args) }
                    p { class: "muted", "{say(&ui, Line::PlanAskSize, today, &args)}" }
                    SizePicker {
                        value: plan.size,
                        onpick: {
                            let db = db.clone();
                            let profile = profile.clone();
                            move |size| {
                                let events = coach.events.peek().clone();
                                let fresh = build_plan(&profile, &events, today, size);
                                if let Err(e) = db.0.borrow_mut().save_plan(&fresh) {
                                    eprintln!("kabar: saving today's plan failed: {e}");
                                }
                                let mut rev = coach.plan_rev;
                                rev += 1;
                            }
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
                            }
                        }
                    }
                    Snooze { profile: profile.clone(), goal_met: progress.goal_met }
                }
                crate::play::PagiTodayCard {}
                // On the phone, Kak Nur and Settings live here instead of the
                // bottom bar (it keeps five items).
                nav { class: "row compact-only more-links",
                    for screen in [Screen::Coach, Screen::Settings] {
                        button {
                            key: "{screen:?}",
                            onclick: move |_| {
                                let mut s = ui.screen;
                                s.set(screen);
                            },
                            span { class: "nav-icon", aria_hidden: "true", {icons::nav(screen)} }
                            " {ui.t(screen.label())}"
                        }
                    }
                }
            }
            if recap_ready {
                div {
                    Recap { profile: profile.clone(), plan: plan.clone(), today }
                }
            }
        }
    }
}

/// Pelita (the streak lamp) and Koin.
#[component]
fn RewardsBar(today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let events = coach.events.read();
    let pelita = rewards::pelita(&events, today);
    let koin = rewards::koin_balance(&events);
    rsx! {
        div { class: "rewards-bar",
            div { class: if pelita.lit_today { "reward lit" } else { "reward" },
                span { class: "reward-icon", aria_hidden: "true", {icons::lamp()} }
                span { class: "latin", bdi { "Pelita" } }
                strong { "{ui.num(pelita.streak)}" }
                if pelita.freezes > 0 {
                    span { class: "muted small", "· {ui.t(Msg::Freezes)} {ui.num(pelita.freezes)}" }
                }
            }
            div { class: "reward",
                span { class: "reward-icon", aria_hidden: "true", {icons::coin()} }
                span { class: "latin", bdi { "Koin" } }
                strong { "{ui.num(koin)}" }
            }
        }
    }
}

#[component]
fn PlanRow(item: PlanItem, done: bool, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let (title, detail) = labels::plan_item(&ui, &item.kind);
    let target = screen_for(&item.kind);
    let id = item.id.clone();
    let seconds = u32::from(item.minutes) * 60;
    rsx! {
        li { class: if done { "plan-item done" } else { "plan-item" },
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
            div { class: "plan-text",
                strong { "{ui.t(title)}" }
                if let Some(detail) = detail {
                    span { class: "muted", " · " bdi { lang: "id", "{detail}" } }
                }
                div { class: "muted small", {labels::minutes(&ui, u32::from(item.minutes))} }
            }
            if !done {
                button {
                    onclick: move |_| {
                        let mut screen = ui.screen;
                        screen.set(target);
                    },
                    "{ui.t(Msg::Start)}"
                }
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
    let events = coach.events.read().clone();
    let tomorrow = build_plan(&profile, &events, today.add_days(1), profile.daily_size);
    let tomorrow_first = tomorrow.items.first().map(|i| i.kind.clone());
    let r = recap::day_recap(&events, today, plan.size.minutes(), plan.items.len(), tomorrow_first);

    // Voice placeholders: text already localized, numbers in the UI's digits,
    // and empty when they don't apply (say then picks a variant without them).
    let minutes = ui.num(r.minutes);
    let words = if r.new_words.is_empty() { String::new() } else { ui.num(r.new_words.len() as u64) };
    let done = history::day(&events, today).items_done;
    let worked_on = plan
        .items
        .iter()
        .find(|i| done.contains(&i.id) && !matches!(i.kind, PlanItemKind::Review { .. }))
        .map(|i| labels::plan_item_text(&ui, &i.kind))
        .unwrap_or_default();
    let base = [
        ("name", profile.name.as_str()),
        ("minutes", minutes.as_str()),
        ("words", words.as_str()),
        ("topic", worked_on.as_str()),
    ];
    let headline = say(&ui, if r.goal_met { Line::RecapGoalMet } else { Line::RecapSomeStudy }, today, &base);
    let remember = r.remember.as_ref().map(|f| {
        let topic = format!("{} · {}", ui.t(labels::skill(f.skill)), f.topic);
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
            Nur { text: headline }
            div { class: "stats-row",
                Stat { value: labels::minutes(&ui, r.minutes), label: Msg::StatTime }
                Stat { value: ui.num(r.items_done), label: Msg::StatSteps }
                Stat { value: ui.num(r.reviews), label: Msg::StatReviews }
                Stat { value: ui.num(r.new_words.len() as u32), label: Msg::StatNewWords }
            }
            if !r.new_words.is_empty() {
                p { class: "words", lang: "id",
                    for w in r.new_words.iter() {
                        bdi { key: "{w}", class: "word", "{w}" }
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
