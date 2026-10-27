//! Cases: key-feature clinical cases, the `calc` number vignettes, and the
//! standalone calculators, as three tabs of one screen.
//!
//! A case shows its stem, then each step reveals new data and asks one
//! decision, with feedback (`why` and the book page) right after. The data
//! revealed so far stays on screen; from 840dp it sits beside the question.
//! A finished case is one review: its grade comes from the steps right.

use std::sync::Mutex;

use batna_core::content::{Body, Item, Status};
use batna_core::review::{Grade, MemorySink, Origin, ReviewEvent, ReviewSink};
use dioxus::prelude::*;

use crate::calc::{Calculators, item::CalcItem};
use crate::content::Library;
use crate::i18n::case as tx;

const CSS: &str = include_str!("case.css");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Cases,
    CalcItems,
    Calculators,
}

#[component]
pub fn CasesScreen() -> Element {
    let mut tab = use_signal(|| Tab::Cases);
    let mut open = use_signal(|| None::<String>);
    let back = move |_| open.set(None);
    rsx! {
        style { {CSS} }
        if let Some(id) = open() {
            if tab() == Tab::Cases {
                CasePlayer { key: "{id}", item_id: id, on_back: back }
            } else {
                CalcItem { key: "{id}", item_id: id, on_back: back }
            }
        } else {
            div { class: "tabs", role: "tablist",
                for (t, label) in [(Tab::Cases, tx::TAB_CASES), (Tab::CalcItems, tx::TAB_CALC_ITEMS), (Tab::Calculators, crate::i18n::case::TAB_CALCULATORS)] {
                    button {
                        key: "{t:?}",
                        role: "tab",
                        "aria-selected": if tab() == t { "true" } else { "false" },
                        onclick: move |_| tab.set(t),
                        {label}
                    }
                }
            }
            match tab() {
                Tab::Cases => rsx! { ItemList { kind: "case", on_open: move |id| open.set(Some(id)) } },
                Tab::CalcItems => rsx! { ItemList { kind: "calc", on_open: move |id| open.set(Some(id)) } },
                Tab::Calculators => rsx! { Calculators {} },
            }
        }
    }
}

/// Items of one kind across every written lecture, in book order.
fn items_of(kind: &str) -> Vec<(&'static str, &'static Item)> {
    let lib = Library::get();
    lib.index
        .lectures
        .iter()
        .filter_map(|e| lib.lectures.get(&e.id))
        .flat_map(|l| l.items.iter().map(move |i| (l.title.as_str(), i)))
        .filter(|(_, i)| i.body.kind() == kind && i.status != Status::Rejected)
        .collect()
}

/// The lecture title and the item, by item id.
pub fn find_item(id: &str) -> Option<(&'static str, &'static Item)> {
    let lib = Library::get();
    let lecture = id.rsplit_once('.').map(|(l, _)| l)?;
    let lec = lib.lectures.get(lecture)?;
    lec.items.iter().find(|i| i.id == id).map(|i| (lec.title.as_str(), i))
}

#[component]
fn ItemList(kind: &'static str, on_open: EventHandler<String>) -> Element {
    let items = items_of(kind);
    let empty = if kind == "case" { tx::EMPTY } else { tx::EMPTY_CALC };
    rsx! {
        if kind == "case" { p { class: "muted", {tx::INTRO} } }
        if items.is_empty() {
            p { class: "card muted", {empty} }
        }
        ul { class: "case-list",
            for (lecture, item) in items {
                li { key: "{item.id}",
                    button { class: "case-row", dir: "auto", onclick: { let id = item.id.clone(); move |_| on_open.call(id.clone()) },
                        bdi { class: "case-row-stem", {first_line(item)} }
                        span { class: "muted small", dir: "rtl",
                            match &item.body {
                                Body::Case { steps, .. } => rsx! { "{steps.len()} {tx::STEPS} · " },
                                Body::Calc { calculator, .. } => rsx! { bdi { {batna_core::calc::spec(calculator).map_or("", |s| s.name)} } " · " },
                                _ => rsx! {},
                            }
                            bdi { "{lecture}" }
                        }
                    }
                }
            }
        }
    }
}

fn first_line(item: &Item) -> String {
    let text = match &item.body {
        Body::Case { stem, .. } | Body::Calc { stem, .. } => stem.as_str(),
        _ => "",
    };
    let mut out: String = text.chars().take(120).collect();
    if text.chars().count() > 120 {
        out.push('…');
    }
    out
}

/// The line above every player: back, lecture, and the review status.
#[component]
pub fn ItemMeta(lecture: String, item_id: String, on_back: EventHandler) -> Element {
    let unverified = find_item(&item_id).is_some_and(|(_, i)| i.status == Status::Unverified);
    rsx! {
        div { class: "player-meta",
            button { class: "link-button", onclick: move |_| on_back.call(()), "‹ " {tx::BACK} }
            bdi { class: "muted small", "{lecture}" }
            if unverified { span { class: "chip", {tx::UNVERIFIED} } }
        }
    }
}

#[component]
pub fn CasePlayer(item_id: String, on_back: EventHandler) -> Element {
    // `step` runs past the last index once the case is finished.
    let mut step = use_signal(|| 0usize);
    let mut picked = use_signal(|| None::<usize>);
    let mut shown = use_signal(|| false);
    let mut results = use_signal(Vec::<bool>::new);
    let mut started = use_signal(now_ms);

    let Some((lecture, item)) = find_item(&item_id) else { return rsx! {} };
    let Body::Case { stem, steps } = &item.body else { return rsx! {} };
    let total = steps.len();
    let k = step();
    let done = k >= total;
    let answered = results.read().len() > k;
    let page = item.src.page;
    let record_id = item.id.clone();
    let step_answer = steps.get(k).and_then(|s| s.answer);

    let mut answer = move |right: bool| {
        if results.read().len() == step() {
            results.write().push(right);
        }
    };

    rsx! {
        article { class: "player case",
            ItemMeta { lecture: lecture.to_owned(), item_id: item.id.clone(), on_back }
            div { class: "case-grid",
                // What is known so far: the stem and every reveal up to this step.
                section { class: "card case-data",
                    bdi { class: "case-stem", "{stem}" }
                    for (i, s) in steps.iter().enumerate().take(k + 1) {
                        if let Some(reveal) = &s.reveal {
                            div { key: "{i}", class: if i == k { "case-reveal new" } else { "case-reveal" },
                                span { class: "small muted", "{tx::STEP} {i + 1}" if i == k { " · {tx::NEW_DATA}" } }
                                bdi { "{reveal}" }
                            }
                        }
                    }
                }
                if !done {
                    section { class: "card case-step", key: "{k}",
                        p { class: "small muted", "{tx::STEP} {k + 1} {tx::OF} {total}" }
                        bdi { class: "case-question", "{steps[k].question}" }
                        if !steps[k].options.is_empty() {
                            div { class: "options", role: "list",
                                for (i, opt) in steps[k].options.iter().enumerate() {
                                    button {
                                        key: "{i}",
                                        role: "listitem",
                                        dir: "auto",
                                        class: option_class(i, picked(), steps[k].answer),
                                        disabled: picked().is_some(),
                                        onclick: move |_| {
                                            picked.set(Some(i));
                                            answer(step_answer == Some(i));
                                        },
                                        bdi { "{opt}" }
                                    }
                                }
                            }
                        } else if !shown() {
                            button { class: "btn-primary", onclick: move |_| shown.set(true), {tx::SHOW_ANSWER} }
                        }
                        if picked().is_some() || shown() {
                            if let Some(p) = picked() {
                                p { class: if steps[k].answer == Some(p) { "verdict good" } else { "verdict bad" },
                                    if steps[k].answer == Some(p) { {tx::RIGHT} } else { {tx::WRONG} }
                                }
                            }
                            div { class: "case-why",
                                bdi { "{steps[k].why}" }
                                p { class: "small muted", "{tx::PAGE} " bdi { "{page}" } }
                            }
                            if shown() && !answered {
                                div { class: "row",
                                    button { class: "btn-good", onclick: move |_| answer(true), {tx::GOT_IT} }
                                    button { class: "btn-secondary", onclick: move |_| answer(false), {tx::MISSED} }
                                }
                            }
                            if answered {
                                button {
                                    class: "btn-primary",
                                    onclick: {
                                        let record_id = record_id.clone();
                                        move |_| {
                                            if k + 1 == total {
                                                let right = results.read().iter().filter(|r| **r).count() as u32;
                                                record(&record_id, right, total as u32, started());
                                            }
                                            step.set(k + 1);
                                            picked.set(None);
                                            shown.set(false);
                                        }
                                    },
                                    if k + 1 == total { {tx::FINISH} } else { {tx::NEXT} }
                                }
                            }
                        }
                    }
                } else {
                    Summary {
                        results: results(),
                        whys: steps.iter().map(|s| s.why.clone()).collect::<Vec<_>>(),
                        update: item.update.as_ref().map(|u| format!("{} ({})", u.text, u.reference)),
                        on_again: move |_| {
                            step.set(0);
                            picked.set(None);
                            shown.set(false);
                            results.set(Vec::new());
                            started.set(now_ms());
                        },
                        on_back,
                    }
                }
            }
        }
    }
}

fn option_class(i: usize, picked: Option<usize>, answer: Option<usize>) -> &'static str {
    match picked {
        None => "option",
        Some(_) if answer == Some(i) => "option right",
        Some(p) if p == i => "option wrong",
        Some(_) => "option dim",
    }
}

#[component]
fn Summary(
    results: Vec<bool>,
    whys: Vec<String>,
    update: Option<String>,
    on_again: EventHandler,
    on_back: EventHandler,
) -> Element {
    let right = results.iter().filter(|r| **r).count();
    let total = results.len();
    let message = match grade_for(right as u32, total as u32) {
        Grade::Good | Grade::Easy => tx::ALL_RIGHT,
        Grade::Hard => tx::SOME_RIGHT,
        Grade::Again => tx::FEW_RIGHT,
    };
    rsx! {
        section { class: "card case-summary",
            p { class: "case-score", bdi { dir: "ltr", "{right} / {total}" } " " {tx::SCORE} }
            p { {message} }
            ol { class: "case-steps-done",
                for (i, (ok, why)) in results.iter().zip(whys.iter()).enumerate() {
                    li { key: "{i}", class: if *ok { "ok" } else { "miss" },
                        span { aria_hidden: "true", if *ok { "✓ " } else { "✗ " } }
                        bdi { "{why}" }
                    }
                }
            }
            if let Some(u) = update { p { class: "small update", "{tx::UPDATE}: " bdi { "{u}" } } }
            div { class: "row",
                button { class: "btn-secondary", onclick: move |_| on_again.call(()), {tx::AGAIN} }
                button { class: "btn-primary", onclick: move |_| on_back.call(()), {tx::BACK} }
            }
        }
    }
}

/// One review per case: every step right is Good, at least half is Hard,
/// fewer is Again. Easy is left to the self-rating screens.
pub fn grade_for(right: u32, total: u32) -> Grade {
    if total == 0 || right * 2 < total {
        Grade::Again
    } else if right < total {
        Grade::Hard
    } else {
        Grade::Good
    }
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Answers wait here until chunk 3's store takes `ReviewSink`; then this
/// records straight into it.
static EVENTS: Mutex<MemorySink> = Mutex::new(MemorySink { events: Vec::new() });

pub fn record(item_id: &str, right: u32, total: u32, started_ms: i64) {
    let at_ms = now_ms();
    let event = ReviewEvent {
        item_id: item_id.to_owned(),
        grade: grade_for(right, total),
        origin: Origin::Case,
        at_ms,
        took_ms: u32::try_from(at_ms.saturating_sub(started_ms)).unwrap_or(u32::MAX),
    };
    if let Ok(mut sink) = EVENTS.lock() {
        sink.record(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades_from_steps_right() {
        assert_eq!(grade_for(3, 3), Grade::Good);
        assert_eq!(grade_for(2, 3), Grade::Hard);
        assert_eq!(grade_for(1, 2), Grade::Hard);
        assert_eq!(grade_for(1, 3), Grade::Again);
        assert_eq!(grade_for(0, 0), Grade::Again);
    }

    #[test]
    fn options_are_marked_after_a_pick() {
        assert_eq!(option_class(0, None, Some(1)), "option");
        assert_eq!(option_class(1, Some(0), Some(1)), "option right");
        assert_eq!(option_class(0, Some(0), Some(1)), "option wrong");
        assert_eq!(option_class(2, Some(0), Some(1)), "option dim");
    }

    #[test]
    fn record_keeps_the_case_as_one_review() {
        record("hb.L03.050", 2, 3, now_ms() - 1500);
        let sink = EVENTS.lock().unwrap();
        let e = sink.events.iter().find(|e| e.item_id == "hb.L03.050").unwrap();
        assert_eq!(e.grade, Grade::Hard);
        assert_eq!(e.origin, Origin::Case);
        assert!(e.took_ms >= 1500);
    }
}
