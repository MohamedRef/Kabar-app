//! `order` items: put an algorithm or management sequence in order.
//!
//! Tap a step in the pool to place it next; tap a placed step to send it
//! back, nudge it with ↑ ↓, or drag it with a mouse or pen. "Check" shows the
//! correct order beside the learner's, with only the out-of-place steps
//! marked (see [`grade::misplaced`]). The tile pattern is adapted from
//! kabar-app main: crates/app/src/lesson.rs (`Tiles`).

pub mod grade;

use batna_core::content::{Body, Item};
use batna_core::review::Grade;
use dioxus::prelude::*;

use crate::i18n::order as tx;
use crate::table::GradeRow;

const CSS: &str = include_str!("order.css");

#[derive(Clone, Default, PartialEq)]
struct Draft {
    /// Placed steps, by their index in the correct order.
    placed: Vec<usize>,
    /// Position in `placed` being dragged.
    dragging: Option<usize>,
    checked: bool,
}

// Live once crate::study dispatches `order` items here (chunk 2).
#[allow(dead_code)]
#[component]
pub fn OrderItem(item: Item, on_grade: EventHandler<Grade>) -> Element {
    let Body::Order { prompt, steps } = &item.body else {
        return rsx! {};
    };
    let n = steps.len();
    let pool_order = use_hook(|| grade::shuffled(n, &item.id));
    let mut draft = use_signal(Draft::default);
    let d = draft.read().clone();
    let wrong = if d.checked { grade::misplaced(&d.placed) } else { Vec::new() };
    let full = d.placed.len() == n;

    rsx! {
        style { {CSS} }
        section { class: "or-item",
            h2 { class: "or-prompt", dir: "ltr", lang: "en", "{prompt}" }
            if !d.checked {
                p { class: "muted small", {tx::HINT} }
            }
            div { class: "or-board", "data-checked": if d.checked { "yes" } else { "no" },
                div { class: "or-col",
                    h3 { class: "or-head", if d.checked { {tx::YOUR_ORDER} } else { {tx::PLACED} } }
                    ol { class: "or-list", dir: "ltr", lang: "en",
                        for (pos, &step) in d.placed.iter().enumerate() {
                            PlacedStep {
                                key: "p{step}",
                                pos,
                                step,
                                text: steps[step].clone(),
                                last: pos + 1 == d.placed.len(),
                                wrong: wrong.contains(&pos),
                                draft,
                            }
                        }
                        if !full {
                            li { class: "or-slot", aria_hidden: "true",
                                span { class: "or-num", "{d.placed.len() + 1}" }
                                span { class: "muted small", dir: "rtl", lang: "ar", {tx::NEXT_SLOT} }
                            }
                        }
                    }
                }
                if d.checked {
                    div { class: "or-col",
                        h3 { class: "or-head", {tx::CORRECT_ORDER} }
                        ol { class: "or-list", dir: "ltr", lang: "en",
                            for (i, text) in steps.iter().enumerate() {
                                li { key: "c{i}", class: "or-step or-key",
                                    span { class: "or-num", "{i + 1}" }
                                    bdi { class: "or-text", dir: "ltr", "{text}" }
                                }
                            }
                        }
                    }
                } else if !full {
                    div { class: "or-col",
                        h3 { class: "or-head", {tx::POOL} }
                        div { class: "or-pool", dir: "ltr", lang: "en",
                            for &step in pool_order.iter().filter(|s| !d.placed.contains(s)) {
                                button {
                                    key: "b{step}",
                                    class: "or-tile",
                                    onclick: move |_| draft.write().placed.push(step),
                                    bdi { dir: "ltr", "{steps[step]}" }
                                }
                            }
                        }
                    }
                }
            }
            if d.checked {
                p { class: if wrong.is_empty() { "or-verdict good" } else { "or-verdict" },
                    if wrong.is_empty() {
                        {tx::ALL_IN_PLACE}
                    } else {
                        {tx::in_place(n - wrong.len(), n)}
                    }
                }
                GradeRow { suggested: grade::suggest(wrong.len()), on_grade }
            } else {
                div { class: "or-actions",
                    button {
                        class: "or-button primary",
                        disabled: !full,
                        onclick: move |_| draft.write().checked = true,
                        {tx::CHECK}
                    }
                    if !d.placed.is_empty() {
                        button {
                            class: "or-button",
                            onclick: move |_| draft.write().placed.clear(),
                            {tx::CLEAR}
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PlacedStep(
    pos: usize,
    step: usize,
    text: String,
    last: bool,
    wrong: bool,
    draft: Signal<Draft>,
) -> Element {
    let checked = draft.read().checked;
    let dragging = draft.read().dragging == Some(pos);
    let class = match (checked, wrong) {
        (false, _) => "or-step",
        (true, false) => "or-step ok",
        (true, true) => "or-step wrong",
    };
    let mut nudge = move |to: usize| {
        let mut d = draft.write();
        let s = d.placed.remove(pos);
        d.placed.insert(to, s);
    };
    rsx! {
        li {
            class,
            "data-dragging": if dragging { "yes" } else { "no" },
            draggable: if checked { "false" } else { "true" },
            ondragstart: move |_| draft.write().dragging = Some(pos),
            ondragend: move |_| draft.write().dragging = None,
            ondragover: move |e| e.prevent_default(),
            ondrop: move |e| {
                e.prevent_default();
                let mut d = draft.write();
                if let Some(from) = d.dragging.take() {
                    let s = d.placed.remove(from);
                    d.placed.insert(pos, s);
                }
            },
            span { class: "or-num", "{pos + 1}" }
            if checked {
                bdi { class: "or-text", dir: "ltr", "{text}" }
                if wrong {
                    span { class: "or-should", dir: "rtl", lang: "ar", {tx::should_be(step + 1)} }
                } else {
                    span { class: "or-tick", aria_label: tx::IN_PLACE, "✓" }
                }
            } else {
                button {
                    class: "or-text or-back",
                    aria_label: tx::SEND_BACK,
                    title: tx::SEND_BACK,
                    onclick: move |_| {
                        draft.write().placed.remove(pos);
                    },
                    bdi { dir: "ltr", "{text}" }
                }
                span { class: "or-nudge",
                    button {
                        class: "or-mini",
                        aria_label: tx::MOVE_UP,
                        disabled: pos == 0,
                        onclick: move |_| nudge(pos - 1),
                        "↑"
                    }
                    button {
                        class: "or-mini",
                        aria_label: tx::MOVE_DOWN,
                        disabled: last,
                        onclick: move |_| nudge(pos + 1),
                        "↓"
                    }
                }
            }
        }
    }
}
