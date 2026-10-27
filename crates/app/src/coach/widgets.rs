//! Pieces shared by onboarding, Today, the Coach screen and Settings.

use dioxus::prelude::*;
use kabar_protocol::coach::{LocalTime, PlanSize, QuietHours, ReminderStyle};

use super::{labels, logic};
use crate::i18n::Msg;
use crate::icons;
use crate::portraits::{self, Face, Mood};
use crate::settings::device::DevicePrefs;
use crate::{Db, Ui, platform};

/// Kak Nur speaking, with her portrait once Codex has drawn it.
#[component]
pub fn Nur(text: String, #[props(default)] mood: Mood) -> Element {
    rsx! {
        div { class: "nur",
            if let Some(src) = portraits::url("kak-nur", mood) {
                Face { src, class: "nur-avatar" }
            } else {
                div { class: "nur-avatar", aria_hidden: "true", {icons::coach()} }
            }
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

/// A 24-hour time picker bound to a [`LocalTime`]: hour and minute selects,
/// so it reads "20:00" like the rest of the app whatever the device locale
/// (a native `<input type=time>` shows "08:00 PM" on some systems).
#[component]
pub fn TimeInput(value: LocalTime, label: String, onchange: EventHandler<LocalTime>) -> Element {
    let ui = use_context::<Ui>();
    let (hour, minute) = (value.hour(), value.minute());
    // Five-minute steps, plus the current minute if it is off the grid.
    let mut minutes: Vec<u16> = (0..60).step_by(5).collect();
    if !minutes.contains(&minute) {
        minutes.push(minute);
        minutes.sort_unstable();
    }
    let set = move |h: u16, m: u16| onchange.call(LocalTime::hm(h, m));
    rsx! {
        span { class: "time-input", dir: "ltr", role: "group", aria_label: "{label}",
            select {
                aria_label: "{label}",
                onchange: move |e: FormEvent| {
                    if let Ok(h) = e.value().parse() {
                        set(h, minute);
                    }
                },
                for h in 0..24u16 {
                    option { key: "{h}", value: "{h}", selected: h == hour, {labels::two_digits(&ui, h)} }
                }
            }
            span { aria_hidden: "true", ":" }
            select {
                aria_label: "{label}",
                onchange: move |e: FormEvent| {
                    if let Ok(m) = e.value().parse() {
                        set(hour, m);
                    }
                },
                for m in minutes {
                    option { key: "{m}", value: "{m}", selected: m == minute, {labels::two_digits(&ui, m)} }
                }
            }
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
    // Re-read after the system prompt, which answers outside the app.
    let mut asked = use_signal(|| 0u32);
    let _ = asked();
    let allowed = platform::notifications_enabled() != Some(false);
    let db = use_context::<Db>();
    let prefs = use_context::<DevicePrefs>();
    let here = prefs.reminds_here(*ui.tablet.read());
    let asleep = here && platform::reminders::battery_unrestricted() == Some(false);
    rsx! {
        div { class: "choices", role: "group",
            for style in labels::STYLES {
                button {
                    key: "{style:?}",
                    class: "choice",
                    aria_pressed: if style == value { "true" } else { "false" },
                    onclick: move |_| {
                        // Android 13+ asks once, right when reminders are chosen.
                        if style != ReminderStyle::Off {
                            platform::reminders::request();
                            asked += 1;
                        }
                        onpick.call(style);
                    },
                    strong { "{ui.t(labels::reminder_style(style).0)}" }
                    span { class: "hint", "{ui.t(labels::reminder_style(style).1)}" }
                }
            }
        }
        if value != ReminderStyle::Off {
            if !platform::reminders::available() {
                p { class: "hint", "{ui.t(Msg::RemindersElsewhere)}" }
            } else if !allowed {
                p { class: "hint", "{ui.t(Msg::RemindersBlocked)}" }
                button {
                    class: "secondary",
                    onclick: move |_| {
                        if let Err(e) = platform::open_app_page(platform::AppPage::Notifications) {
                            crate::diag::report!("{e}");
                        }
                        asked += 1;
                    },
                    "{ui.t(Msg::RemindersAllow)}"
                }
            }
            if platform::reminders::available() {
                label { class: "row",
                    input {
                        r#type: "checkbox",
                        checked: here,
                        onchange: move |e: FormEvent| prefs.set_remind_here(&db, e.checked()),
                    }
                    " {ui.t(Msg::RemindHere)}"
                }
                p { class: "hint", "{ui.t(Msg::RemindHereHint)}" }
            }
            if asleep {
                p { class: "hint", "{ui.t(Msg::RemindersAsleep)}" }
                button {
                    class: "secondary",
                    onclick: move |_| {
                        if let Err(e) = platform::open_app_page(platform::AppPage::Details) {
                            crate::diag::report!("opening the app settings page failed: {e}");
                        }
                        asked += 1;
                    },
                    "{ui.t(Msg::RemindersBattery)}"
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
