//! The Coach section of Settings: reminders, study times, quiet hours, the
//! weekly report's day and time, and the usual daily size.

use dioxus::prelude::*;
use kabar_protocol::coach::{CoachProfile, Weekday};

use super::widgets::{QuietHoursEditor, SizePicker, StylePicker, TimeInput, TimesEditor};
use super::{Coach, labels};
use crate::i18n::Msg;
use crate::{Db, Ui};

#[component]
pub fn CoachSettings() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    let save = use_callback(move |p: CoachProfile| coach.save_profile(&db, p));
    let Some(p) = coach.profile.read().clone() else {
        return rsx! {};
    };
    // Each editor changes one field of the current profile and saves it.
    let edit = move |f: Box<dyn FnOnce(&mut CoachProfile)>| {
        if let Some(mut p) = coach.profile.peek().clone() {
            f(&mut p);
            save.call(p);
        }
    };

    rsx! {
        div { class: "card",
            h2 { bdi { class: "latin", "Kak Nur" } }
            div { class: "field",
                span { class: "label", "{ui.t(Msg::DailySize)}" }
                SizePicker { value: p.daily_size, onpick: move |s| edit(Box::new(move |p| p.daily_size = s)) }
            }
            div { class: "field",
                span { class: "label", "{ui.t(Msg::StudyTimes)}" }
                TimesEditor {
                    times: p.study_times.clone(),
                    onchange: move |t| edit(Box::new(move |p| p.study_times = t)),
                }
            }
            div { class: "field",
                span { class: "label", "{ui.t(Msg::ReminderStyle)}" }
                StylePicker {
                    value: p.reminder_style,
                    onpick: move |s| edit(Box::new(move |p| p.reminder_style = s)),
                }
            }
            div { class: "field",
                span { class: "label", "{ui.t(Msg::QuietHours)}" }
                QuietHoursEditor {
                    value: p.quiet_hours,
                    onchange: move |q| edit(Box::new(move |p| p.quiet_hours = q)),
                }
            }
            div { class: "field",
                span { class: "label", "{ui.t(Msg::ReportWhen)}" }
                div { class: "row",
                    select {
                        aria_label: "{ui.t(Msg::ReportWhen)}",
                        onchange: move |e: FormEvent| {
                            if let Some(day) = Weekday::ALL.iter().copied().find(|d| format!("{d:?}") == e.value()) {
                                edit(Box::new(move |p| p.report_day = day));
                            }
                        },
                        for day in Weekday::ALL {
                            option {
                                key: "{day:?}",
                                value: "{day:?}",
                                selected: day == p.report_day,
                                "{ui.t(labels::weekday(day))}"
                            }
                        }
                    }
                    TimeInput {
                        value: p.report_time,
                        label: ui.t(Msg::ReportWhen).to_string(),
                        onchange: move |t| edit(Box::new(move |p| p.report_time = t)),
                    }
                }
            }
        }
    }
}
