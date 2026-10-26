//! Pieces shared by onboarding, Today, the Coach screen and Settings.

use dioxus::prelude::*;
use kabar_protocol::coach::{LocalTime, PlanSize, QuietHours, ReminderStyle};

use super::{labels, logic};
use crate::Ui;
use crate::i18n::Msg;
use crate::icons;

/// Kak Nur speaking.
#[component]
pub fn Nur(text: String) -> Element {
    rsx! {
        div { class: "nur",
            div { class: "nur-avatar", aria_hidden: "true", {icons::coach()} }
            div { class: "nur-bubble",
                span { class: "nur-name latin", bdi { "Kak Nur" } }
                p { "{text}" }
            }
        }
    }
}

/// "5, 15 or 30 minutes today?"
#[component]
pub fn SizePicker(value: PlanSize, onpick: EventHandler<PlanSize>) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "row", role: "group",
            for size in PlanSize::ALL {
                button {
                    key: "{size:?}",
                    aria_pressed: if size == value { "true" } else { "false" },
                    onclick: move |_| onpick.call(size),
                    {labels::minutes(&ui, u32::from(size.minutes()))}
                }
            }
        }
    }
}

/// A native time picker bound to a [`LocalTime`].
#[component]
pub fn TimeInput(value: LocalTime, label: String, onchange: EventHandler<LocalTime>) -> Element {
    rsx! {
        input {
            r#type: "time",
            class: "time-input",
            dir: "ltr",
            aria_label: "{label}",
            value: "{value}",
            onchange: move |e: FormEvent| {
                let v = e.value();
                if let Ok(t) = v.get(..5).unwrap_or(&v).parse::<LocalTime>() {
                    onchange.call(t);
                }
            },
        }
    }
}

/// Preferred study times: edit, add, remove (at least one stays).
#[component]
pub fn TimesEditor(times: Vec<LocalTime>, onchange: EventHandler<Vec<LocalTime>>) -> Element {
    let ui = use_context::<Ui>();
    let only_one = times.len() <= 1;
    rsx! {
        ul { class: "times",
            for (i , t) in times.iter().copied().enumerate() {
                li { key: "{i}-{t}", class: "row",
                    TimeInput {
                        value: t,
                        label: ui.t(Msg::StudyTimes).to_string(),
                        onchange: {
                            let times = times.clone();
                            move |new: LocalTime| {
                                let mut times = times.clone();
                                times[i] = new;
                                onchange.call(logic::normalize_times(times));
                            }
                        },
                    }
                    if !only_one {
                        button {
                            class: "quiet",
                            onclick: {
                                let times = times.clone();
                                move |_| {
                                    let mut times = times.clone();
                                    times.remove(i);
                                    onchange.call(times);
                                }
                            },
                            "{ui.t(Msg::RemoveTime)}"
                        }
                    }
                }
            }
        }
        button {
            onclick: {
                let times = times.clone();
                move |_| {
                    let mut times = times.clone();
                    times.push(logic::next_time(&times));
                    onchange.call(logic::normalize_times(times));
                }
            },
            "+ {ui.t(Msg::AddTime)}"
        }
    }
}

#[component]
pub fn StylePicker(value: ReminderStyle, onpick: EventHandler<ReminderStyle>) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "choices", role: "group",
            for style in labels::STYLES {
                button {
                    key: "{style:?}",
                    class: "choice",
                    aria_pressed: if style == value { "true" } else { "false" },
                    onclick: move |_| onpick.call(style),
                    strong { "{ui.t(labels::reminder_style(style).0)}" }
                    span { class: "hint", "{ui.t(labels::reminder_style(style).1)}" }
                }
            }
        }
    }
}

#[component]
pub fn QuietHoursEditor(value: QuietHours, onchange: EventHandler<QuietHours>) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "row",
            span { "{ui.t(Msg::QuietFrom)}" }
            TimeInput {
                value: value.start,
                label: ui.t(Msg::QuietFrom).to_string(),
                onchange: move |start| onchange.call(QuietHours { start, ..value }),
            }
            span { "{ui.t(Msg::QuietTo)}" }
            TimeInput {
                value: value.end,
                label: ui.t(Msg::QuietTo).to_string(),
                onchange: move |end| onchange.call(QuietHours { end, ..value }),
            }
        }
    }
}

/// One number with its label.
#[component]
pub fn Stat(value: String, label: Msg) -> Element {
    let ui = use_context::<Ui>();
    rsx! {
        div { class: "stat",
            span { class: "stat-value", "{value}" }
            span { class: "stat-label", "{ui.t(label)}" }
        }
    }
}

/// A bar that fills toward a goal.
#[component]
pub fn Progress(done: u32, goal: u32, label: String) -> Element {
    let pct = (done.min(goal) * 100).checked_div(goal).unwrap_or(100);
    rsx! {
        div {
            class: "progress",
            role: "progressbar",
            aria_label: "{label}",
            aria_valuemin: "0",
            aria_valuemax: "{goal}",
            aria_valuenow: "{done.min(goal)}",
            div { class: "progress-fill", style: "inline-size: {pct}%" }
        }
    }
}
