//! Meeting Kak Nur: why, focus, minutes a day, a target, study times,
//! reminders and where the journey starts (spec §6).

use dioxus::prelude::*;
use kabar_core::coach::onboarding::{self, ProfileError, Step};
use kabar_core::coach::voice::Line;
use kabar_protocol::coach::{CoachProfile, LanguageFocus, LocalDate, ReminderStyle, Target};

use super::text::{T, tt};
use super::widgets::{Nur, QuietHoursEditor, SizePicker, StylePicker, TimesEditor};
use super::{Coach, labels, say};
use crate::i18n::Msg;
use crate::{Db, Ui};

fn line(step: Step) -> Line {
    match step {
        Step::Name => Line::OnboardHello,
        Step::Why => Line::OnboardWhy,
        Step::Focus => Line::OnboardFocus,
        Step::Minutes => Line::OnboardMinutes,
        Step::Target => Line::OnboardTarget,
        Step::Times => Line::OnboardTimes,
        Step::Reminders => Line::OnboardReminders,
        Step::Start => Line::OnboardStart,
        Step::Done => Line::OnboardDone,
    }
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
    // Voice placeholders: text already localized, numbers in the UI's digits.
    let minutes = ui.num(p.daily_size.minutes());
    let (level, by) = p
        .target
        .as_ref()
        .map(|t| (labels::level_name(&t.level), labels::long_date(&ui, t.by)))
        .unwrap_or_default();
    let args = [("name", name.as_str()), ("minutes", minutes.as_str()), ("level", &level), ("date", &by)];
    let text = say(&ui, line(current), today, &args);
    let can_next = match current {
        Step::Name => !name.is_empty(),
        Step::Times => !p.study_times.is_empty(),
        _ => true,
    };
    let index = Step::ALL.iter().position(|s| *s == current).unwrap_or(0);

    let suggest = move |p: &CoachProfile| onboarding::suggest_target(p.start, p.daily_size, today);
    // Saves the profile through the core's validation; on an error, goes
    // back to the step that needs fixing. `early`: the fast start, taken
    // before the reminders step, so reminders stay off until the learner
    // turns them on (Kak Nur's page and Settings › Kak Nur).
    let finish = move |db: &Db, early: bool| {
        let mut profile = draft();
        profile.name = profile.name.trim().to_string();
        profile.utc_offset_min = coach.offset();
        if !target_touched() && profile.target.is_some() {
            profile.target = Some(suggest(&profile));
        }
        if early {
            profile.reminder_style = ReminderStyle::Off;
        }
        match onboarding::finish(profile, today) {
            Ok(profile) => coach.save_profile(db, profile),
            Err(e) => {
                let (s, msg) = error_step(&e);
                let (mut error, mut step) = (error, step);
                error.set(Some(msg));
                step.set(s);
            }
        }
    };
    // The fast start: once the language and the starting point are chosen,
    // the rest (minutes, target, times, reminders) can wait.
    let can_start_early = matches!(current, Step::Minutes | Step::Target | Step::Times);

    rsx! {
        style { {super::today::COACH_CSS.text()} }
        div { class: "onboarding",
            crate::ui::DivCard {
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
                    p { class: "field-error", role: "alert", "{ui.t(msg)}" }
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
                        button { class: "primary", onclick: {
                                let db = db.clone();
                                move |_| finish(&db, false)
                            }, "{ui.t(Msg::OnbFinish)}" }
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
                if can_start_early {
                    button { class: "text-btn onb-early", onclick: {
                            let db = db.clone();
                            move |_| finish(&db, true)
                        }, "{tt(ui.lang(), T::StartNow)}" }
                    p { class: "muted small onb-early-hint", "{tt(ui.lang(), T::StartNowHint)}" }
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
        div { class: "answers", role: "group",
            for f in labels::FOCUSES {
                button {
                    key: "{f:?}",
                    class: "answer",
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
    // Both languages: the goal is an Indonesian (or Javanese) level, and
    // Javanese opens after BIPA 1, as the Journey says.
    let both = draft.read().focus == LanguageFocus::Both;
    let set = move |t: Option<Target>| {
        let (mut touched, mut draft) = (touched, draft);
        touched.set(true);
        draft.write().target = t;
    };
    rsx! {
        if both {
            p { class: "note", "{ui.t(Msg::OnbBothOrder)}" }
        }
        if target.is_some() {
            p { class: "muted small", "{tt(ui.lang(), T::TargetEstimateHint)}" }
        }
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
                    TargetMonth {
                        value: t.by,
                        today,
                        onchange: {
                            let t = t.clone();
                            move |by| set(Some(Target { by, ..t.clone() }))
                        },
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
        div { class: "answers", role: "group",
            for s in labels::STARTS {
                button {
                    key: "{s:?}",
                    class: "answer",
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
        // The date leads so an Arabic line stays right-to-left; the level is isolated.
        .map(|t| {
            format!("{} · \u{2068}{}\u{2069}", labels::long_date(&ui, t.by), labels::level_name(&t.level))
        });
    rsx! {
        dl { class: "summary",
            dt { "{ui.t(Msg::OnbNameLabel)}" }
            dd { bdi { "{profile.name}" } }
            if !why.is_empty() {
                dt { "{ui.t(Msg::OnbWhy)}" }
                dd { "{why}" }
            }
            dt { "{ui.t(Msg::OnbFocus)}" }
            // The Both note rides in the focus value: a lone `dd` would
            // shift every later label into the value column (#303).
            dd {
                bdi { "{focus}" }
                if profile.focus == LanguageFocus::Both {
                    span { class: "summary-note muted", "{ui.t(Msg::OnbBothOrder)}" }
                }
            }
            dt { "{ui.t(Msg::DailySize)}" }
            dd { {labels::minutes(&ui, u32::from(profile.daily_size.minutes()))} }
            if let Some(target) = target {
                dt { "{ui.t(Msg::OnbTarget)}" }
                dd { "{target}" }
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

/// The target's month and year as two selects (targets fall on the 1st):
/// month names instead of a native date field, which shows the device
/// locale's "04/01/2028" and reads as January in some places.
#[component]
fn TargetMonth(value: LocalDate, today: LocalDate, onchange: EventHandler<LocalDate>) -> Element {
    let ui = use_context::<Ui>();
    let (year, month, _) = value.ymd();
    let (this_year, this_month, _) = today.ymd();
    // Targets run up to ten years out; keep an older pick selectable.
    let years: Vec<i32> = (this_year.min(year)..=this_year.max(year).max(this_year + 10)).collect();
    // Never earlier than next month's 1st (a target can't be in the past).
    let pick = move |y: i32, m: u32| {
        let (y, m) = if (y, m) <= (this_year, this_month) {
            if this_month == 12 { (this_year + 1, 1) } else { (this_year, this_month + 1) }
        } else {
            (y, m)
        };
        if let Some(d) = LocalDate::from_ymd(y, m, 1) {
            onchange.call(d);
        }
    };
    rsx! {
        label {
            span { class: "label", "{ui.t(Msg::OnbTargetDate)}" }
            span { class: "row target-month",
                select {
                    aria_label: "{ui.t(Msg::OnbTargetDate)}",
                    onchange: move |e: FormEvent| {
                        if let Ok(m) = e.value().parse() {
                            pick(year, m);
                        }
                    },
                    for m in 1..=12u32 {
                        option { key: "{m}", value: "{m}", selected: m == month, "{labels::month(&ui, m)}" }
                    }
                }
                select {
                    aria_label: "{ui.t(Msg::OnbTargetDate)}",
                    onchange: move |e: FormEvent| {
                        if let Ok(y) = e.value().parse() {
                            pick(y, month);
                        }
                    },
                    for y in years {
                        option { key: "{y}", value: "{y}", selected: y == year, {ui.num(y as u32)} }
                    }
                }
            }
        }
    }
}
