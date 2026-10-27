//! `table` items: a comparison grid from the book with some cells blanked.
//!
//! Each blank is tap-to-reveal, then self-marked ✓ / ✗; or, with typing on,
//! the learner types every blank and "Check" marks them (and can be overruled
//! per cell). When every blank is marked, the share recalled suggests a grade.
//! On phones the grid scrolls sideways in its own box with the row labels
//! (column 0) pinned.

pub mod grade;
mod grades;

pub use grades::GradeRow;

use batna_core::content::{Body, Item};
use batna_core::review::Grade;
use dioxus::prelude::*;

use crate::i18n::table as tx;

const CSS: &str = include_str!("table.css");

/// Cells longer than this are tap-to-reveal even with typing on.
const TYPABLE_CHARS: usize = 40;

#[derive(Clone, Debug, Default, PartialEq)]
struct Blank {
    typed: String,
    shown: bool,
    /// Recalled or not: set by "Check" for typed cells, by the learner otherwise.
    right: Option<bool>,
}

// Live once crate::study dispatches `table` items here (chunk 2).
#[allow(dead_code)]
#[component]
pub fn TableItem(item: Item, on_grade: EventHandler<Grade>) -> Element {
    let Body::Table { title, columns, rows, blanks } = &item.body else {
        return rsx! {};
    };
    let cells: Vec<[usize; 2]> = blanks
        .iter()
        .copied()
        .filter(|&[r, c]| rows.get(r).is_some_and(|row| c < row.len() && c < columns.len()))
        .collect();
    let n = cells.len();
    let mut state = use_signal(|| vec![Blank::default(); n]);
    let mut typing = use_signal(|| false);

    let s = state.read().clone();
    let hidden = s.iter().filter(|b| !b.shown).count();
    let marked: Vec<bool> = s.iter().filter_map(|b| b.right).collect();
    let done = marked.len() == n;
    let right = marked.iter().filter(|&&r| r).count();

    let answers: Vec<String> = cells.iter().map(|&[r, c]| rows[r][c].clone()).collect();
    let check_all = move |_| {
        let mut st = state.write();
        for (b, answer) in st.iter_mut().zip(&answers) {
            if !b.shown {
                b.shown = true;
                if !b.typed.trim().is_empty() {
                    b.right = Some(grade::matches(&b.typed, answer));
                }
            }
        }
    };

    rsx! {
        style { {CSS} }
        section { class: "tb-item",
            h2 { class: "tb-title", dir: "ltr", lang: "en", "{title}" }
            if hidden > 0 {
                p { class: "muted small", if *typing.read() { {tx::HINT_TYPE} } else { {tx::HINT_TAP} } }
            }
            div {
                class: "tb-scroll",
                dir: "ltr",
                lang: "en",
                tabindex: "0",
                role: "region",
                aria_label: "{title}",
                table { class: "tb-grid",
                    thead {
                        tr {
                            for (c, head) in columns.iter().enumerate() {
                                th { key: "h{c}", scope: "col", class: if c == 0 { "tb-pin" } else { "" }, "{head}" }
                            }
                        }
                    }
                    tbody {
                        for (r, row) in rows.iter().enumerate() {
                            tr { key: "r{r}",
                                for (c, text) in row.iter().enumerate().take(columns.len()) {
                                    match cells.iter().position(|&cell| cell == [r, c]) {
                                        Some(i) => rsx! {
                                            BlankCell {
                                                key: "c{r}-{c}",
                                                index: i,
                                                answer: text.clone(),
                                                pinned: c == 0,
                                                typing: *typing.read(),
                                                state,
                                            }
                                        },
                                        None if c == 0 => rsx! { th { key: "c{r}-{c}", scope: "row", class: "tb-pin", "{text}" } },
                                        None => rsx! { td { key: "c{r}-{c}", "{text}" } },
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if hidden > 0 {
                div { class: "tb-actions",
                    if *typing.read() {
                        button { class: "tb-button primary", onclick: check_all, {tx::CHECK_TYPED} }
                    } else {
                        button { class: "tb-button primary", onclick: check_all, {tx::REVEAL_ALL} }
                    }
                    button {
                        class: "tb-button",
                        aria_pressed: if *typing.read() { "true" } else { "false" },
                        onclick: move |_| typing.toggle(),
                        if *typing.read() { {tx::TYPING_OFF} } else { {tx::TYPING_ON} }
                    }
                }
            } else if !done {
                p { class: "muted small", {tx::mark_each(n - marked.len())} }
            }
            if done {
                p { class: if right == n { "tb-verdict good" } else { "tb-verdict" }, {tx::recalled(right, n)} }
                GradeRow { suggested: grade::suggest(right, n), on_grade }
            }
        }
    }
}

#[component]
fn BlankCell(index: usize, answer: String, pinned: bool, typing: bool, state: Signal<Vec<Blank>>) -> Element {
    let b = state.read()[index].clone();
    let class = match (pinned, b.right) {
        (_, Some(true)) => "tb-blank ok",
        (_, Some(false)) => "tb-blank wrong",
        _ => "tb-blank",
    };
    let class = if pinned { format!("{class} tb-pin") } else { class.to_string() };
    let mut set_right = move |value: bool| state.write()[index].right = Some(value);
    rsx! {
        td { class,
            if !b.shown {
                // Long cells are revealed and self-marked even in typing mode.
                if typing && answer.chars().count() <= TYPABLE_CHARS {
                    input {
                        class: "tb-input",
                        r#type: "text",
                        aria_label: tx::TYPE_CELL,
                        autocomplete: "off",
                        spellcheck: "false",
                        value: "{b.typed}",
                        oninput: move |e| state.write()[index].typed = e.value(),
                    }
                } else {
                    button {
                        class: "tb-reveal",
                        aria_label: tx::REVEAL_CELL,
                        onclick: move |_| state.write()[index].shown = true,
                        "?"
                    }
                }
            } else {
                span { class: "tb-answer", "{answer}" }
                if !b.typed.trim().is_empty() && b.right == Some(false) {
                    span { class: "tb-typed", s { "{b.typed}" } }
                }
                match b.right {
                    None => rsx! {
                        span { class: "tb-marks", dir: "rtl", lang: "ar",
                            button { class: "tb-mark yes", aria_label: tx::KNEW_IT, title: tx::KNEW_IT, onclick: move |_| set_right(true), "✓" }
                            button { class: "tb-mark no", aria_label: tx::MISSED_IT, title: tx::MISSED_IT, onclick: move |_| set_right(false), "✗" }
                        }
                    },
                    Some(r) => rsx! {
                        button {
                            class: "tb-flip",
                            dir: "rtl",
                            lang: "ar",
                            aria_label: if r { tx::MARK_WRONG } else { tx::MARK_RIGHT },
                            title: if r { tx::MARK_WRONG } else { tx::MARK_RIGHT },
                            onclick: move |_| set_right(!r),
                            if r { "✓" } else { "✗" }
                        }
                    },
                }
            }
        }
    }
}
