//! The learning journal (spec §6): an automatic timeline of everything the
//! learner did, day by day, plus their own reflections on each day.
//!
//! The timeline is derived (activity log, finished lessons, taught words);
//! only reflections are stored, in the synced `journal_entries` table.
//! Reflections are personal and are never sent to any AI engine.
//!
//! Handwriting: Kabar has no reusable ink canvas yet (Pen Lab is a
//! diagnostic probe, not an editor), so reflections are typed for now. The
//! seam is `JournalEntry::ink` (`kabar_protocol::journal::InkRef`): once
//! Tablet Studio (M9) ships a canvas that saves to `ink_documents` /
//! `ink_strokes`, the reflection editor offers it on medium/expanded widths
//! and stores the document id there; `Reflection` renders it.

use std::rc::Rc;

use dioxus::prelude::*;
use kabar_core::coach::rewards;
use kabar_core::layout::WidthClass;
use kabar_protocol::coach::{ActivityKind, LocalDate, Skill};
use kabar_protocol::journal::{JournalEntry, MAX_REFLECTION_CHARS};

use super::logic::{self, EntryKind, JournalDay, JournalFilter, JournalSources};
use super::{Coach, clock, labels};
use crate::account::Cloud;
use crate::cloud::Command;
use crate::i18n::Msg;
use crate::store::now_ms;
use crate::{Db, Screen, Ui};

/// Days rendered per page; "show older days" adds another page.
const PAGE_DAYS: usize = 14;

/// The reflection being written or edited (one at a time).
#[derive(Clone, Debug, PartialEq)]
struct Draft {
    date: LocalDate,
    /// The reflection being edited; `None` for a new one.
    editing: Option<JournalEntry>,
    text: String,
}

fn filter_label(f: JournalFilter) -> Msg {
    match f {
        JournalFilter::All => Msg::JournalFilterAll,
        JournalFilter::Learning => Msg::JournalFilterLearning,
        JournalFilter::Words => Msg::JournalFilterWords,
        JournalFilter::Reflections => Msg::JournalFilterReflections,
    }
}

/// Pushes local writes to Kabar Cloud right away (when sync is on).
fn kick_sync(cloud: Option<Cloud>) {
    if let Some(cloud) = cloud {
        cloud.send(Command::Kick);
    }
}

#[component]
pub fn JournalScreen() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let coach = use_context::<Coach>();
    // Remote changes (another device's reflection, lesson or taught word).
    let remote_rev = try_use_context::<Cloud>().map(|c| *c.revision.read());
    // Local reflection writes.
    let local_rev = use_signal(|| 0u32);
    let mut filter = use_signal(JournalFilter::default);
    let mut shown = use_signal(|| PAGE_DAYS);
    let draft = use_signal(|| None::<Draft>);
    let catalog = use_hook(|| Rc::new(crate::journey::journal_catalog()));

    let _ = (remote_rev, local_rev());
    let events = coach.events.read().clone();
    let today = coach.today();
    let offset = coach.offset();
    let (lessons, taught, reflections) = {
        let store = db.0.borrow();
        let log = |what: &str, e: &dyn std::fmt::Display| crate::diag::report!("loading {what} failed: {e}");
        (
            store.lesson_completions().unwrap_or_else(|e| {
                log("finished lessons", &e);
                Vec::new()
            }),
            store.taught_with_time().unwrap_or_else(|e| {
                log("taught words", &e);
                Vec::new()
            }),
            store.reflections().unwrap_or_else(|e| {
                log("reflections", &e);
                Vec::new()
            }),
        )
    };
    let src = JournalSources { lessons: &lessons, taught: &taught, reflections: &reflections };
    let days = logic::timeline(&events, &src, &catalog, offset, today, filter());
    let nothing = days.iter().all(|d| d.entries.is_empty() && d.reflections.is_empty());
    let inked = *ui.width.read() != WidthClass::Compact;

    rsx! {
        div { class: "journal",
            header { class: "journal-head",
                div { class: "page-head",
                    div {
                        h1 { "{ui.t(Msg::JournalTitle)}" }
                    }
                }
                div { class: "segmented journal-filters", role: "radiogroup", aria_label: "{ui.t(Msg::JournalTitle)}",
                    for f in JournalFilter::ALL {
                        button {
                            key: "{f:?}",
                            role: "radio",
                            aria_checked: if filter() == f { "true" } else { "false" },
                            onclick: move |_| {
                                filter.set(f);
                                shown.set(PAGE_DAYS);
                            },
                            "{ui.t(filter_label(f))}"
                        }
                    }
                }
                if inked {
                }
            }
            if nothing && filter() != JournalFilter::All {
                // A filter that hides everything offers the way back to all of it.
                div { class: "empty-state", role: "status",
                    crate::ui::Card { element: crate::ui::CardElement::Paragraph, class: "muted", "{ui.t(Msg::JournalNothingHere)}" }
                    button {
                        class: "tonal",
                        onclick: move |_| {
                            filter.set(JournalFilter::All);
                            shown.set(PAGE_DAYS);
                        },
                        "{ui.t(Msg::JournalShowAll)}"
                    }
                }
            }
            for day in days.iter().take(shown()) {
                Day {
                    key: "{day.date}",
                    day: day.clone(),
                    today,
                    offset,
                    reflections_on: matches!(filter(), JournalFilter::All | JournalFilter::Reflections),
                    draft,
                    local_rev,
                }
            }
            if days.len() > shown() {
                button { class: "journal-older", onclick: move |_| shown += PAGE_DAYS, "{ui.t(Msg::JournalShowOlder)}" }
            }
        }
    }
}

#[component]
fn Day(
    day: JournalDay,
    today: LocalDate,
    offset: i16,
    reflections_on: bool,
    draft: Signal<Option<Draft>>,
    local_rev: Signal<u32>,
) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let date = day.date;
    let stats = &day.stats;
    let editing_here = draft.read().as_ref().is_some_and(|d| d.date == date && d.editing.is_none());
    let summary = (stats.seconds > 0 || stats.xp > 0).then(|| {
        let minutes = labels::minutes(&ui, stats.minutes());
        let xp = ui.num(stats.xp);
        (minutes, xp)
    });
    rsx! {
        crate::ui::Card { class: "journal-day", aria_label: "{labels::day_heading(&ui, date, today)}",
            header { class: "journal-day-head",
                h3 { "{labels::day_heading(&ui, date, today)}" }
                if let Some((minutes, xp)) = summary {
                    p { class: "muted small journal-day-sum",
                        "{minutes} · {xp} "
                        bdi { class: "latin", "XP" }
                    }
                }
            }
            if reflections_on {
                for r in day.reflections.iter() {
                    Reflection { key: "{r.id}", entry: r.clone(), draft, local_rev }
                }
                if editing_here {
                    Editor { draft, local_rev }
                } else if draft.read().is_none() {
                    button {
                        class: "journal-add",
                        onclick: move |_| {
                            let mut draft = draft;
                            draft.set(Some(Draft { date, editing: None, text: String::new() }));
                        },
                        "{ui.t(Msg::JournalReflectionAdd)}"
                    }
                }
            }
            if !day.entries.is_empty() {
                ul { class: "journal-entries",
                    for e in day.entries.iter() {
                        // Each line opens what it was: the lesson, the reviews,
                        // the reading, the souvenir, the taught word.
                        li { key: "{e.key}",
                            button {
                                class: "journal-entry tap tap-plain",
                                onclick: {
                                    let kind = e.kind.clone();
                                    let db = db.clone();
                                    move |_| open_entry(ui, &db, &kind)
                                },
                                span { class: "muted small journal-time",
                                    {labels::time(&ui, clock::time_of_day(e.at_ms, offset))}
                                }
                                span { class: "journal-what", EntryLine { kind: e.kind.clone() } }
                                span { class: "chev", aria_hidden: "true" }
                            }
                        }
                    }
                }
            } else if day.reflections.is_empty() && !editing_here && date == today {
                p { class: "muted", "{ui.t(Msg::JournalEmpty)}" }
            }
        }
    }
}

#[component]
fn Reflection(entry: JournalEntry, draft: Signal<Option<Draft>>, local_rev: Signal<u32>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let cloud = try_use_context::<Cloud>();
    let mut confirm = use_signal(|| false);
    let editing = draft.read().as_ref().and_then(|d| d.editing.as_ref()).is_some_and(|e| e.id == entry.id);
    if editing {
        return rsx! { Editor { draft, local_rev } };
    }
    let edited = entry.updated_ms > entry.created_ms;
    let id = entry.id.clone();
    rsx! {
        article { class: "journal-note", aria_label: "{ui.t(Msg::JournalReflectionLabel)}",
            // The learner may write in any script; `dir: auto` follows it.
            p { class: "journal-note-text", dir: "auto", "{entry.text}" }
            div { class: "row journal-note-actions",
                if edited {
                    span { class: "muted small", "{ui.t(Msg::JournalReflectionEdited)}" }
                }
                if draft.read().is_none() {
                    button {
                        onclick: {
                            let entry = entry.clone();
                            move |_| {
                                let mut draft = draft;
                                let text = entry.text.clone();
                                draft.set(Some(Draft { date: entry.date, editing: Some(entry.clone()), text }));
                            }
                        },
                        "{ui.t(Msg::JournalReflectionEdit)}"
                    }
                    if confirm() {
                        button {
                            class: "journal-delete",
                            onclick: move |_| {
                                if let Err(e) = db.0.borrow_mut().delete_reflection(&id) {
                                    crate::diag::report!("deleting the reflection failed: {e}");
                                }
                                kick_sync(cloud);
                                let mut rev = local_rev;
                                rev += 1;
                            },
                            "{ui.t(Msg::JournalReflectionDeleteSure)}"
                        }
                        button { onclick: move |_| confirm.set(false), "{ui.t(Msg::JournalReflectionCancel)}" }
                    } else {
                        button { onclick: move |_| confirm.set(true), "{ui.t(Msg::JournalReflectionDelete)}" }
                    }
                }
            }
        }
    }
}

#[component]
fn Editor(draft: Signal<Option<Draft>>, local_rev: Signal<u32>) -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let cloud = try_use_context::<Cloud>();
    let Some(current) = draft.read().clone() else { return rsx! {} };
    let can_save = logic::clean_reflection(&current.text).is_some();
    let save = move |_| {
        let Some(d) = draft.peek().clone() else { return };
        let Some(text) = logic::clean_reflection(&d.text) else { return };
        let now = now_ms();
        let entry = match d.editing {
            Some(old) => JournalEntry { text, updated_ms: now, ..old },
            None => JournalEntry {
                id: uuid::Uuid::now_v7().to_string(),
                date: d.date,
                text,
                ink: None,
                created_ms: now,
                updated_ms: now,
            },
        };
        if let Err(e) = db.0.borrow_mut().save_reflection(&entry) {
            crate::diag::report!("saving the reflection failed: {e}");
            return;
        }
        kick_sync(cloud);
        let (mut draft, mut rev) = (draft, local_rev);
        draft.set(None);
        rev += 1;
    };
    rsx! {
        div { class: "journal-editor",
            textarea {
                dir: "auto",
                rows: "4",
                maxlength: "{MAX_REFLECTION_CHARS}",
                aria_label: "{ui.t(Msg::JournalReflectionLabel)}",
                placeholder: "{ui.t(Msg::JournalReflectionPlaceholder)}",
                value: "{current.text}",
                autofocus: true,
                oninput: move |e| {
                    let mut draft = draft;
                    if let Some(d) = draft.write().as_mut() {
                        d.text = e.value();
                    }
                },
            }
            div { class: "row",
                button { class: "primary", disabled: !can_save, onclick: save, "{ui.t(Msg::JournalReflectionSave)}" }
                button {
                    onclick: move |_| {
                        let mut draft = draft;
                        draft.set(None);
                    },
                    "{ui.t(Msg::JournalReflectionCancel)}"
                }
            }
        }
    }
}

/// One timeline line, in words.
/// Opens what a journal line refers to.
fn open_entry(ui: Ui, db: &Db, kind: &EntryKind) {
    let mut screen = ui.screen;
    let to = match kind {
        EntryKind::LessonDone { lesson_id, .. } => {
            ui.open_lesson(lesson_id.clone());
            return;
        }
        EntryKind::Activity(ActivityKind::LessonStep { unit_id, .. }) => {
            let completed = db.0.borrow().completed_lessons().unwrap_or_default();
            match crate::journey::next_lesson_in(unit_id, &completed) {
                Some(id) => {
                    ui.open_lesson(id);
                    return;
                }
                None => Screen::Journey,
            }
        }
        EntryKind::Activity(ActivityKind::Reviews { .. } | ActivityKind::NewWords { .. }) => Screen::Review,
        EntryKind::Activity(ActivityKind::Practice { skill, .. }) => match skill {
            Skill::Reading => Screen::Reader,
            Skill::Listening => Screen::Listen,
            _ => Screen::Play,
        },
        EntryKind::Activity(ActivityKind::PlanItemDone { .. }) => Screen::Today,
        EntryKind::Activity(ActivityKind::Purchase { .. }) => Screen::Coach,
        EntryKind::Souvenir { unit_id, .. } => {
            crate::journey::open_suitcase(ui, unit_id);
            return;
        }
        EntryKind::Taught { .. } => Screen::Teach,
    };
    screen.set(to);
}

#[component]
fn EntryLine(kind: EntryKind) -> Element {
    let ui = use_context::<Ui>();
    match kind {
        EntryKind::Activity(a) => rsx! { ActivityLine { kind: a } },
        EntryKind::LessonDone { lesson_id, title, test, score } => {
            let what = ui.t(if test { Msg::JournalTestPassed } else { Msg::EvLessonDone });
            let name = title.unwrap_or(lesson_id);
            rsx! {
                "{what} · "
                bdi { lang: "id", "{name}" }
                if let Some(s) = score {
                    " · {ui.whole_percent(s)}"
                }
            }
        }
        EntryKind::Souvenir { name, .. } => rsx! {
            "{ui.t(Msg::JournalSouvenir)}: "
            bdi { "{name}" }
        },
        EntryKind::Taught { word, draft } => rsx! {
            "{ui.t(Msg::JournalTaught)}: "
            bdi { lang: "id", "{word}" }
            if draft {
                span { class: "muted small", " · {ui.t(Msg::JournalDraft)}" }
            }
        },
    }
}

#[component]
fn ActivityLine(kind: ActivityKind) -> Element {
    let ui = use_context::<Ui>();
    match kind {
        ActivityKind::Reviews { cards, correct } => rsx! {
            "{ui.t(Msg::EvReviews)} · {ui.num(correct.min(cards))} / {ui.num(cards)}"
        },
        ActivityKind::NewWords { words } => rsx! {
            "{ui.t(Msg::EvNewWords)}: "
            bdi { lang: "id", "{words.join(\", \")}" }
        },
        ActivityKind::LessonStep { unit_id, done } => {
            let what = ui.t(if done { Msg::EvLessonDone } else { Msg::EvLessonStep });
            rsx! {
                "{what} · "
                bdi { lang: "id", "{unit_id}" }
            }
        }
        ActivityKind::Practice { skill, topic, correct, total } => rsx! {
            "{ui.t(labels::skill(skill))} · "
            bdi { lang: "id", "{topic}" }
            " · {ui.num(correct.min(total))} / {ui.num(total)}"
        },
        ActivityKind::PlanItemDone { .. } => rsx! { "{ui.t(Msg::EvPlanItem)}" },
        ActivityKind::Purchase { item_id, koin } => {
            let name =
                rewards::warung().iter().find(|i| i.id == item_id).map_or(item_id.clone(), |i| i.name.into());
            rsx! {
                "{ui.t(Msg::EvPurchase)} "
                bdi { lang: "id", "{name}" }
                " · {ui.num(koin)} "
                bdi { class: "latin", "Koin" }
            }
        }
    }
}
