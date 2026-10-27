//! The `calc` study item: a vignette with numbers. Mohamed computes the score
//! himself (or opens the calculator, pre-filled from the vignette), sees the
//! worked result, then says what it means before the interpretation opens.

use batna_core::calc::{self, Inputs};
use batna_core::content::Body;
use dioxus::prelude::*;

use super::{CalcForm, OutcomeCard};
use crate::case::{ItemMeta, find_item, record};
use crate::i18n::{calc as tx, case as cx};

#[derive(Clone, Copy, PartialEq)]
enum Computed {
    /// Not yet checked.
    Open,
    /// He typed a result: right or not.
    Typed(bool),
    /// He used the calculator instead of typing.
    Calculator,
}

#[component]
pub fn CalcItem(item_id: String, on_back: EventHandler) -> Element {
    let mut typed = use_signal(String::new);
    let mut computed = use_signal(|| Computed::Open);
    let mut meaning_open = use_signal(|| false);
    let mut verdict = use_signal(|| None::<bool>);
    let mut started = use_signal(crate::case::now_ms);
    let finish = use_callback({
        let item_id = item_id.clone();
        move |right: bool| {
            verdict.set(Some(right));
            // Two decisions when he typed a result, one when he used the calculator.
            let (got, total) = match computed() {
                Computed::Typed(ok) => (u32::from(ok) + u32::from(right), 2),
                _ => (u32::from(right), 1),
            };
            record(&item_id, got, total, started());
        }
    });

    let Some((lecture, item)) = find_item(&item_id) else { return rsx! {} };
    let Body::Calc { calculator, stem, question, inputs, expect, interpret } = &item.body else {
        return rsx! {};
    };
    let Some(spec) = calc::spec(calculator) else { return rsx! {} };
    let initial = Inputs::for_spec(spec, inputs);
    let outcome = (spec.compute)(&initial).ok();
    let id = spec.id;
    let expect_text = expect.clone();

    let check = {
        let outcome = outcome.clone();
        move |_| {
            let t = typed.read().trim().to_owned();
            let right = outcome.as_ref().is_some_and(|o| calc::expect_matches(o, &t))
                || t.eq_ignore_ascii_case(expect_text.trim());
            computed.set(Computed::Typed(right));
        }
    };
    let state = computed();

    rsx! {
        article { class: "player calc-item",
            ItemMeta { lecture: lecture.to_owned(), item_id: item.id.clone(), on_back }
            section { class: "card",
                bdi { class: "case-stem", "{stem}" }
                p { class: "case-question", dir: "auto",
                    {question.clone().unwrap_or_else(|| format!("Calculate {}.", spec.name))}
                }
            }
            if state == Computed::Open {
                section { class: "card",
                    label { r#for: "calc-answer", {tx::YOUR_RESULT} }
                    input {
                        id: "calc-answer",
                        dir: "ltr",
                        value: "{typed}",
                        oninput: move |e: FormEvent| typed.set(e.value()),
                    }
                    p { class: "small muted", {tx::YOUR_RESULT_HINT} }
                    div { class: "row",
                        button { class: "btn-primary", disabled: typed.read().trim().is_empty(), onclick: check, {tx::CHECK} }
                        button { class: "btn-secondary", onclick: move |_| computed.set(Computed::Calculator), {tx::USE_CALC} }
                    }
                }
            } else {
                if let Computed::Typed(ok) = state {
                    p { class: if ok { "verdict good" } else { "verdict bad" }, if ok { {tx::MATCH} } else { {tx::NO_MATCH} } }
                }
                if state == Computed::Calculator {
                    CalcForm { id, initial: initial.clone() }
                } else if let Some(o) = outcome.clone() {
                    OutcomeCard { id, outcome: o }
                }
                section { class: "card",
                    h2 { {tx::MEANING} }
                    if !meaning_open() {
                        p { class: "small muted", {tx::MEANING_HINT} }
                        button { class: "btn-primary", onclick: move |_| meaning_open.set(true), {tx::SHOW_MEANING} }
                    } else {
                        bdi { class: "case-why", "{interpret}" }
                        p { class: "small muted", "{cx::PAGE} " bdi { "{item.src.page}" } }
                        if let Some(u) = &item.update {
                            p { class: "small update", "{cx::UPDATE}: " bdi { "{u.text} ({u.reference})" } }
                        }
                        match verdict() {
                            None => rsx! {
                                div { class: "row",
                                    button { class: "btn-good", onclick: move |_| finish.call(true), {cx::GOT_IT} }
                                    button { class: "btn-secondary", onclick: move |_| finish.call(false), {cx::MISSED} }
                                }
                            },
                            Some(_) => rsx! {
                                div { class: "row",
                                    button {
                                        class: "btn-secondary",
                                        onclick: move |_| {
                                            typed.set(String::new());
                                            computed.set(Computed::Open);
                                            meaning_open.set(false);
                                            verdict.set(None);
                                            started.set(crate::case::now_ms());
                                        },
                                        {cx::AGAIN}
                                    }
                                    button { class: "btn-primary", onclick: move |_| on_back.call(()), {cx::BACK} }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}
