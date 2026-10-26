//! Meeting Kak Nur: why, focus, minutes a day, a target, study times,
//! reminders and where the journey starts (spec §6).

use dioxus::prelude::*;
use kabar_core::coach::onboarding::{self, ProfileError, Step};
use kabar_core::coach::voice::Line;
use kabar_protocol::coach::{CoachProfile, LocalDate, Target};

use super::widgets::{Nur, QuietHoursEditor, SizePicker, StylePicker, TimesEditor};
use super::{Coach, labels, say};
use crate::i18n::Msg;
use crate::{Db, Ui};

fn line(step: Step) -> Option<Line> {
    Some(match step {
        Step::Name => Line::OnboardHello,
        Step::Why => Line::OnboardWhy,
        // No voice line for the focus question yet; the UI asks it.
        Step::Focus => return None,
        Step::Minutes => Line::OnboardMinutes,
        Step::Target => Line::OnboardTarget,
        Step::Times => Line::OnboardTimes,
        Step::Reminders => Line::OnboardReminders,
        Step::Start => Line::OnboardStart,
        Step::Done => Line::OnboardDone,
    })
}

fn error_step(e: &ProfileError) -> (Step, Msg) {
    match e {
        ProfileError::NoStudyTime => (Step::Times, Msg::ErrNoStudyTime),
        ProfileError::TargetInPast => (Step::Target, Msg::ErrTargetInPast),
        ProfileError::UnknownLevel(_) => (Step::Target, Msg::ErrUnknownLevel),
    }
}

#[component]
pub fn Onboarding() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let mut step = use_signal(|| Step::Name);
    let mut draft = use_signal(CoachProfile::default);
    // Until the learner edits the target, it follows the suggestion.
    let target_touched = use_signal(|| false);
    let mut error = use_signal(|| None::<Msg>);

    let today = coach.today();
    let current = step();
    let p = draft.read().clone();
    let name = p.name.trim().to_string();
    let text = match line(current) {
        Some(l) => say(&ui, l, today, &[("name", name.as_str())]),
        None => ui.t(Msg::OnbFocusAsk).to_string(),
    };
    let can_next = match current {
        Step::Name => !name.is_empty(),
        Step::Times => !p.study_times.is_empty(),
        _ => true,
    };
    let index = Step::ALL.iter().position(|s| *s == current).unwrap_or(0);

    let suggest = move |p: &CoachProfile| onboarding::suggest_target(p.start, p.daily_size, today);

    rsx! {
        div { class: "onboarding",
            div { class: "card",
                ol { class: "dots", aria_label: "{ui.t(Msg::OnbProgress)}",
                    for (i , s) in Step::ALL.iter().enumerate() {
                        li {
                            key: "{s:?}",
                            class: if i < index { "dot done" } else if i == index { "dot current" } else { "dot" },
                            aria_current: if i == index { "step" } else { "false" },
                        }
                    }
                }
                Nur { text }
                div { class: "step",
                    match current {
                        Step::Name => rsx! { NameStep { draft } },
                        Step::Why => rsx! { WhyStep { draft } },
                        Step::Focus => rsx! { FocusStep { draft } },
                        Step::Minutes => rsx! {
                            SizePicker { value: p.daily_size, onpick: move |s| draft.write().daily_size = s }
                        },
                        Step::Target => rsx! { TargetStep { draft, touched: target_touched, today } },
                        Step::Times => rsx! {
                            TimesEditor { times: p.study_times.clone(), onchange: move |t| draft.write().study_times = t }
                        },
                        Step::Reminders => rsx! { RemindersStep { draft } },
                        Step::Start => rsx! { StartStep { draft } },
                        Step::Done => rsx! { Summary { profile: p.clone(), today } },
                    }
                }
                if let Some(msg) = error() {
                    p { class: "error", role: "alert", "{ui.t(msg)}" }
                }
                div { class: "row step-nav",
                    if current != Step::Name {
                        button {
                            onclick: move |_| {
                                error.set(None);
                                step.set(current.prev());
                            },
                            "{ui.t(Msg::Back)}"
                        }
                    }
                    if current == Step::Done {
                        button {
                            class: "primary",
                            onclick: move |_| {
                                let mut profile = draft();
                                profile.name = profile.name.trim().to_string();
                                profile.utc_offset_min = coach.offset();
                                if !target_touched() && profile.target.is_some() {
                                    profile.target = Some(suggest(&profile));
                                }
                                match onboarding::finish(profile, today) {
                                    Ok(profile) => coach.save_profile(&db, profile),
                                    Err(e) => {
                                        let (s, msg) = error_step(&e);
                                        error.set(Some(msg));
                                        step.set(s);
                                    }
                                }
                            },
                            "{ui.t(Msg::OnbFinish)}"
                        }
                    } else {
                        button {
                            class: "primary",
                            disabled: !can_next,
                            onclick: move |_| {
                                error.set(None);
                                let next = current.next();
                                if next == Step::Target && !target_touched() {
                                    let t = suggest(&draft.read());
                                    draft.write().target = Some(t);
                                }
                                step.set(next);
                            },
                            "{ui.t(Msg::Next)}"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn NameStep(draft: Signal<CoachProfile>) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        label { class: "field",
            span { class: "label", "{ui.t(Msg::OnbNameLabel)}" }
            input {
                r#type: "text",
                class: "text-input",
                autocomplete: "given-name",
                value: "{draft.read().name}",
                oninput: move |e: FormEvent| draft.write().name = e.value(),
            }
        }
    }
}

#[component]
fn WhyStep(draft: Signal<CoachProfile>) -> Element {
    let ui = use_context::<Ui>();
    let chosen = draft.read().motivations.clone();
    let note = draft.read().why_note.clone().unwrap_or_default();
    rsx! {
        div { class: "row field", role: "group",
            for m in labels::MOTIVATIONS {
                button {
                    key: "{m:?}",
                    class: "chip",
                    aria_pressed: if chosen.contains(&m) { "true" } else { "false" },
                    onclick: move |_| {
                        let mut d = draft.write();
                        match d.motivations.iter().position(|x| *x == m) {
                            Some(i) => {
                                d.motivations.remove(i);
                            }
                            None => {
                                d.motivations.push(m);
                                d.motivations.sort();
                            }
                        }
                    },
                    "{ui.t(labels::motivation(m))}"
                }
            }
        }
        label { class: "field",
            span { class: "label", "{ui.t(Msg::OnbWhyNote)}" }
            textarea {
                class: "text-input",
                rows: "3",
                value: "{note}",
                oninput: move |e: FormEvent| {
                    let v = e.value();
                    draft.write().why_note = (!v.trim().is_empty()).then_some(v);
                },
            }
        }
    }
}

#[component]
fn FocusStep(draft: Signal<CoachProfile>) -> Element {
    let ui = use_context::<Ui>();
    let current = draft.read().focus;
    rsx! {
        div { class: "choices", role: "group",
            for f in labels::FOCUSES {
                button {
                    key: "{f:?}",
                    class: "choice",
                    aria_pressed: if f == current { "true" } else { "false" },
                    onclick: move |_| draft.write().focus = f,
                    match labels::focus(f) {
                        Some(name) => rsx! { strong { lang: "id", bdi { "{name}" } } },
                        None => rsx! { strong { "{ui.t(Msg::FocusBoth)}" } },
                    }
                }
            }
        }
    }
}

#[component]
fn TargetStep(draft: Signal<CoachProfile>, touched: Signal<bool>, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let target = draft.read().target.clone();
    let set = move |t: Option<Target>| {
        let (mut touched, mut draft) = (touched, draft);
        touched.set(true);
        draft.write().target = t;
    };
    rsx! {
        match target.clone() {
            Some(t) => rsx! {
                div { class: "row field",
                    label {
                        span { class: "label", "{ui.t(Msg::OnbTargetLevel)}" }
                        select {
                            dir: "ltr",
                            value: "{t.level}",
                            onchange: {
                                let t = t.clone();
                                move |e: FormEvent| set(Some(Target { level: e.value(), ..t.clone() }))
                            },
                            for id in labels::LEVELS {
                                option { key: "{id}", value: "{id}", selected: id == t.level, "{labels::level_name(id)}" }
                            }
                        }
                    }
                    label {
                        span { class: "label", "{ui.t(Msg::OnbTargetDate)}" }
                        input {
                            r#type: "date",
                            dir: "ltr",
                            class: "time-input",
                            min: "{today}",
                            value: "{t.by}",
                            onchange: {
                                let t = t.clone();
                                move |e: FormEvent| {
                                    if let Ok(by) = e.value().parse::<LocalDate>() {
                                        set(Some(Target { by, ..t.clone() }));
                                    }
                                }
                            },
                        }
                    }
                }
                button { class: "quiet", onclick: move |_| set(None), "{ui.t(Msg::OnbNoTarget)}" }
            },
            None => rsx! {
                p { class: "muted", "{ui.t(Msg::OnbNoTargetChosen)}" }
                button {
                    onclick: move |_| {
                        let p = draft.read().clone();
                        set(Some(onboarding::suggest_target(p.start, p.daily_size, today)));
                    },
                    "{ui.t(Msg::OnbSetTarget)}"
                }
            },
        }
    }
}

#[component]
fn RemindersStep(draft: Signal<CoachProfile>) -> Element {
    let ui = use_context::<Ui>();
    let p = draft.read().clone();
    rsx! {
        StylePicker { value: p.reminder_style, onpick: move |s| draft.write().reminder_style = s }
        div { class: "field",
            span { class: "label", "{ui.t(Msg::QuietHours)}" }
            QuietHoursEditor { value: p.quiet_hours, onchange: move |q| draft.write().quiet_hours = q }
        }
    }
}

#[component]
fn StartStep(draft: Signal<CoachProfile>) -> Element {
    let ui = use_context::<Ui>();
    let current = draft.read().start;
    rsx! {
        div { class: "choices", role: "group",
            for s in labels::STARTS {
                button {
                    key: "{s:?}",
                    class: "choice",
                    aria_pressed: if s == current { "true" } else { "false" },
                    onclick: move |_| draft.write().start = s,
                    strong { "{ui.t(labels::starting_point(s).0)}" }
                    span { class: "hint", "{ui.t(labels::starting_point(s).1)}" }
                }
            }
        }
        p { class: "muted", "{ui.t(Msg::OnbPlacementLater)}" }
    }
}

#[component]
fn Summary(profile: CoachProfile, today: LocalDate) -> Element {
    let ui = use_context::<Ui>();
    let why = profile.motivations.iter().map(|m| ui.t(labels::motivation(*m))).collect::<Vec<_>>().join("، ");
    let focus = labels::focus(profile.focus).unwrap_or(ui.t(Msg::FocusBoth));
    let times = profile.study_times.iter().map(|t| labels::time(&ui, *t)).collect::<Vec<_>>().join(" · ");
    let target = profile
        .target
        .as_ref()
        .map(|t| format!("{} · {}", labels::level_name(&t.level), labels::date(&ui, t.by, today)));
    rsx! {
        dl { class: "summary",
            dt { "{ui.t(Msg::OnbNameLabel)}" }
            dd { bdi { "{profile.name}" } }
            if !why.is_empty() {
                dt { "{ui.t(Msg::OnbWhy)}" }
                dd { "{why}" }
            }
            dt { "{ui.t(Msg::OnbFocus)}" }
            dd { bdi { "{focus}" } }
            dt { "{ui.t(Msg::DailySize)}" }
            dd { {labels::minutes(&ui, u32::from(profile.daily_size.minutes()))} }
            if let Some(target) = target {
                dt { "{ui.t(Msg::OnbTarget)}" }
                dd { bdi { "{target}" } }
            }
            dt { "{ui.t(Msg::StudyTimes)}" }
            dd { "{times}" }
            dt { "{ui.t(Msg::ReminderStyle)}" }
            dd { "{ui.t(labels::reminder_style(profile.reminder_style).0)}" }
            dt { "{ui.t(Msg::OnbStart)}" }
            dd { "{ui.t(labels::starting_point(profile.start).0)}" }
        }
    }
}
