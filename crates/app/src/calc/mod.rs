//! Calculators: the standalone screen (a tab of Cases), the generic form that
//! any calculator in `batna_core::calc` renders into, and the result card.
//! The `calc` study item lives in [`item`].

pub mod item;

use std::collections::BTreeMap;

use batna_core::calc::{self, Area, CalcError, CalcSpec, CalcValue, FieldKind, Inputs, Outcome};
use dioxus::prelude::*;

use crate::i18n::calc as tx;

fn area_label(area: Area) -> &'static str {
    match area {
        Area::Liver => tx::AREA_LIVER,
        Area::Ascites => tx::AREA_ASCITES,
        Area::Tumour => tx::AREA_TUMOUR,
        Area::AcuteFailure => tx::AREA_ACUTE,
        Area::Pancreas => tx::AREA_PANCREAS,
    }
}

const AREAS: [Area; 5] = [Area::Liver, Area::Ascites, Area::Tumour, Area::AcuteFailure, Area::Pancreas];

/// Every calculator, grouped by area. On phones the list and the open
/// calculator take turns; from 840dp they sit side by side.
#[component]
pub fn Calculators() -> Element {
    let mut open = use_signal(|| None::<&'static str>);
    let current = open();
    rsx! {
        div { class: "calcs", "data-open": if current.is_some() { "yes" } else { "no" },
            nav { class: "calc-list", aria_label: tx::PICK,
                for area in AREAS {
                    section { key: "{area:?}", class: "calc-group",
                        h2 { class: "small muted", {area_label(area)} }
                        div { class: "calc-chips",
                            for spec in calc::all().iter().filter(|s| s.area == area) {
                                button {
                                    key: "{spec.id}",
                                    class: "calc-chip",
                                    "aria-pressed": if current == Some(spec.id) { "true" } else { "false" },
                                    onclick: move |_| open.set(Some(spec.id)),
                                    bdi { {spec.name} }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "calc-pane",
                match current.and_then(calc::spec) {
                    Some(spec) => rsx! {
                        div { key: "{spec.id}",
                            button { class: "link-button calc-back", onclick: move |_| open.set(None), "‹ " {tx::BACK} }
                            CalcForm { id: spec.id, initial: Inputs::new() }
                        }
                    },
                    None => rsx! { p { class: "card muted", {tx::PICK} } },
                }
            }
        }
    }
}

/// The text a field shows for a value.
fn field_text(v: Option<&CalcValue>) -> String {
    match v {
        Some(CalcValue::Number(n)) => trim_num(*n),
        Some(CalcValue::Text(s)) => s.clone(),
        Some(CalcValue::Bool(b)) => if *b { "yes" } else { "no" }.to_owned(),
        None => String::new(),
    }
}

pub fn trim_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e12 {
        format!("{n:.0}")
    } else {
        format!("{n}").trim_end_matches('0').to_owned()
    }
}

/// A form for one calculator with a live result. `initial` pre-fills it
/// (a `calc` item's inputs); every field stays editable.
#[component]
pub fn CalcForm(id: &'static str, initial: Inputs) -> Element {
    let Some(spec) = calc::spec(id) else { return rsx! {} };
    let mut values = use_signal(|| initial.0.clone());
    let inputs = Inputs(
        values.read().iter().filter(|(_, v)| !is_blank(v)).map(|(k, v)| (k.clone(), v.clone())).collect(),
    );
    let result = (spec.compute)(&inputs);
    rsx! {
        div { class: "calc-form",
            section { class: "card calc-fields",
                h1 { bdi { {spec.name} } }
                for field in spec.fields {
                    div { key: "{field.key}", class: "calc-field",
                        label { r#for: "f-{field.key}",
                            bdi { {field.label} }
                            if let FieldKind::Number { unit, .. } = field.kind {
                                if !unit.is_empty() { " " bdi { class: "muted small", dir: "ltr", "({unit})" } }
                            }
                            if field.optional && !matches!(field.kind, FieldKind::Flag) {
                                span { class: "muted small", " · {tx::OPTIONAL}" }
                            }
                        }
                        {field_input(field, values)}
                        if !field.hint.is_empty() { bdi { class: "muted small", {field.hint} } }
                    }
                }
                button { class: "link-button", onclick: move |_| values.set(BTreeMap::new()), {tx::RESET} }
            }
            match result {
                Ok(o) => rsx! { OutcomeCard { id, outcome: o } },
                Err(e) => {
                    let (what, label) = error_text(spec, &e);
                    rsx! {
                        section { class: "card calc-wait",
                            p { class: "muted", {tx::FILL} }
                            p { class: "small", "{what}: " bdi { "{label}" } }
                        }
                    }
                }
            }
        }
    }
}

fn is_blank(v: &CalcValue) -> bool {
    matches!(v, CalcValue::Text(s) if s.trim().is_empty())
}

fn field_input(field: &'static calc::Field, mut values: Signal<BTreeMap<String, CalcValue>>) -> Element {
    let key = field.key;
    let current = values.read().get(key).cloned();
    match field.kind {
        FieldKind::Number { min, max, step, .. } => rsx! {
            input {
                id: "f-{key}",
                r#type: "number",
                inputmode: "decimal",
                dir: "ltr",
                min: "{min}",
                max: "{max}",
                step: "{step}",
                value: field_text(current.as_ref()),
                oninput: move |e: FormEvent| {
                    values.write().insert(key.to_owned(), CalcValue::Text(e.value()));
                },
            }
        },
        FieldKind::Flag => {
            let on = matches!(current, Some(CalcValue::Bool(true)))
                || matches!(&current, Some(CalcValue::Text(s)) if s == "yes" || s == "true");
            rsx! {
                div { class: "calc-toggle", role: "group",
                    button {
                        "aria-pressed": if on { "true" } else { "false" },
                        onclick: move |_| { values.write().insert(key.to_owned(), CalcValue::Bool(true)); },
                        {tx::YES}
                    }
                    button {
                        "aria-pressed": if on { "false" } else { "true" },
                        onclick: move |_| { values.write().insert(key.to_owned(), CalcValue::Bool(false)); },
                        {tx::NO}
                    }
                }
            }
        }
        FieldKind::Choice(options) => {
            let text = field_text(current.as_ref());
            let known = options.iter().any(|(id, _)| *id == text);
            rsx! {
                select {
                    id: "f-{key}",
                    dir: "ltr",
                    onchange: move |e: FormEvent| {
                        values.write().insert(key.to_owned(), CalcValue::Text(e.value()));
                    },
                    option { value: "", selected: text.is_empty(), "—" }
                    // A content alias ("mild") the list doesn't spell the same way.
                    if !known && !text.is_empty() { option { value: "{text}", selected: true, "{text}" } }
                    for (id, label) in options.iter() {
                        option { key: "{id}", value: *id, selected: text == *id, {*label} }
                    }
                }
            }
        }
    }
}

/// What is missing or wrong, and the field's label.
fn error_text(spec: &CalcSpec, e: &CalcError) -> (&'static str, String) {
    let label = |k: &str| spec.fields.iter().find(|f| f.key == k).map_or(k, |f| f.label).to_owned();
    match e {
        CalcError::Missing(k) => (tx::MISSING, label(k)),
        CalcError::Invalid { field, .. } => (tx::INVALID, label(field)),
        CalcError::Unknown(id) => (tx::INVALID, id.clone()),
    }
}

/// The result: the number or stage, its band and meaning, the breakdown,
/// and where it comes from.
#[component]
pub fn OutcomeCard(id: &'static str, outcome: Outcome) -> Element {
    let source = calc::spec(id).map_or("", |s| s.source);
    rsx! {
        section { class: "card calc-result", "aria-live": "polite",
            p { class: "small muted", {tx::RESULT} }
            div { class: "calc-headline",
                bdi { class: "calc-display", "{outcome.display}" }
                bdi { class: "calc-band", {outcome.label} }
            }
            bdi { class: "calc-meaning", "{outcome.meaning}" }
            details { class: "calc-parts",
                summary { {tx::BREAKDOWN} }
                ul {
                    for part in outcome.parts.iter() {
                        li { key: "{part.label}",
                            bdi { "{part.label}" }
                            span { class: "calc-points",
                                match part.met {
                                    Some(true) => "✓".to_owned(),
                                    Some(false) => "—".to_owned(),
                                    None => format!("{:.2}", part.points).trim_end_matches('0').trim_end_matches('.').to_owned(),
                                }
                            }
                        }
                    }
                }
            }
            for note in outcome.notes.iter() {
                bdi { key: "{note}", class: "small calc-note", "{note}" }
            }
            p { class: "small muted",
                {tx::SOURCE} ": " bdi { "{source}" }
                if let Some(page) = outcome.page { " · {tx::BOOK_PAGE} " bdi { "{page}" } }
            }
        }
    }
}
