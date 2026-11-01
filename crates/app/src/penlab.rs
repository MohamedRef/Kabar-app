//! Pen Lab screen: ink canvas + live report of which pen events arrive.

use dioxus::prelude::*;
use kabar_core::penlab::PenLab as Lab;
use kabar_protocol::pen::{PenLabReport, PenSample, Percentiles, WebViewInfo};
use serde::Deserialize;

use crate::i18n::Msg;
use crate::store::PenLabRun;
use crate::{Db, Ui, platform};

const PENLAB_JS: &str = include_str!("../assets/penlab.js");

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum FromInk {
    Info { info: WebViewInfo },
    Samples { samples: Vec<PenSample> },
}

#[component]
pub fn PenLab() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let mut lab = use_signal(Lab::new);
    let mut info = use_signal(WebViewInfo::default);
    let mut status = use_signal(|| None::<Msg>);
    let model = use_hook(platform::device_model);
    let mut runs = use_signal(|| db.0.borrow().pen_lab_runs().unwrap_or_default());

    use_future(move || async move {
        let mut eval = document::eval(PENLAB_JS);
        while let Ok(msg) = eval.recv::<FromInk>().await {
            match msg {
                FromInk::Info { info: i } => info.set(i),
                FromInk::Samples { samples } => lab.write().ingest(&samples),
            }
        }
    });

    let report = lab.read().report(info.read().clone(), model.clone());
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();

    rsx! {
        crate::ui::DivCard {
            h2 { bdi { "Pen Lab" } }
        }
        div { class: "penlab",
            div {
                div { id: "penlab-ink", class: "ink-wrap" }
                div { class: "row", style: "margin-block-start: 0.75rem",
                    button {
                        onclick: move |_| {
                            lab.write().clear();
                            document::eval(r#"document.getElementById("penlab-ink")?.dispatchEvent(new Event("kabar-clear"))"#);
                        },
                        "{ui.t(Msg::PenLabClear)}"
                    }
                    button {
                        class: "primary",
                        onclick: {
                            let db = db.clone();
                            let report = report.clone();
                            move |_| {
                                let saved = db.0.borrow_mut().save_pen_lab_run(&report);
                                if saved.is_ok() {
                                    runs.set(db.0.borrow().pen_lab_runs().unwrap_or_default());
                                    status.set(Some(Msg::PenLabSaved));
                                }
                            }
                        },
                        "{ui.t(Msg::PenLabSave)}"
                    }
                    button {
                        onclick: {
                            let json = json.clone();
                            move |_| {
                                copy_to_clipboard(&json);
                                status.set(Some(Msg::PenLabCopied));
                            }
                        },
                        "{ui.t(Msg::PenLabCopy)}"
                    }
                    if let Some(s) = status() {
                        span { class: "ok", "{ui.t(s)}" }
                    }
                }
            }
            crate::ui::Card { element: crate::ui::CardElement::Div, class: "stats", dir: "ltr", lang: "en",
                dl {
                    dt { "Device" }
                    dd { {report.device_model.clone().unwrap_or_else(|| "?".into())} }
                    dt { "Events" }
                    dd { "{lab.read().total_events()}" }
                    dt { "Hover" }
                    dd { YesNo { on: report.hover_seen } }
                    dt { "Pressure" }
                    dd { {pressure(&report)} }
                    dt { "Tilt" }
                    dd { YesNo { on: report.tilt_seen } }
                    dt { "Twist" }
                    dd { YesNo { on: report.twist_seen } }
                    dt { "Barrel button" }
                    dd { YesNo { on: report.barrel_button_seen } }
                    dt { "Eraser" }
                    dd { YesNo { on: report.eraser_seen } }
                    dt { "Palm touches" }
                    dd { "{report.palm_touches_while_pen_in_range}" }
                    dt { "Coalesced" }
                    dd { {api(report.webview.coalesced_api, report.coalesced_avg)} }
                    dt { "Predicted" }
                    dd { {api(report.webview.predicted_api, report.predicted_avg)} }
                    dt { "Desync canvas" }
                    dd { YesNo { on: report.webview.desynchronized_canvas } }
                    dt { "Pen rate" }
                    dd { {report.pen_move_rate_hz.map(|hz| format!("{hz:.0} Hz")).unwrap_or_else(|| "–".into())} }
                    dt { "Input → handler" }
                    dd { {ms(&report.input_delay_ms)} }
                    dt { "Handler → frame" }
                    dd { {ms(&report.frame_delay_ms)} }
                }
                details {
                    summary { "{ui.t(Msg::PenLabEvents)}" }
                    ul { class: "counts mono",
                        for (k, v) in report.counts.iter() {
                            li { key: "{k}", "{k}: {v}" }
                        }
                    }
                }
            }
        }
        if !runs.read().is_empty() {
            crate::ui::DivCard {
                h2 { "{ui.t(Msg::PenLabSavedRuns)}" }
                for run in runs.read().iter() {
                    SavedRun { key: "{run.id}", run: run.clone() }
                }
            }
        }
    }
}

#[component]
fn YesNo(on: bool) -> Element {
    rsx! { span { class: if on { "ok" } else { "no" }, if on { "yes" } else { "no" } } }
}

#[component]
fn SavedRun(run: PenLabRun) -> Element {
    let json = serde_json::to_string_pretty(&run.report).unwrap_or_default();
    let model = run.report.device_model.clone().unwrap_or_else(|| "?".into());
    rsx! {
        div { class: "row", dir: "ltr",
            code { "{model} · {run.report.counts.values().sum::<u64>()} events · hlc {run.hlc.wall_ms}" }
            button { onclick: move |_| copy_to_clipboard(&json), "JSON" }
        }
    }
}

fn copy_to_clipboard(text: &str) {
    let literal = serde_json::to_string(text).unwrap_or_default();
    document::eval(&format!("navigator.clipboard.writeText({literal})"));
}

fn pressure(r: &PenLabReport) -> String {
    match (r.pressure_min, r.pressure_max) {
        (Some(lo), Some(hi)) => format!("{lo:.3}–{hi:.3} · {} levels", r.pressure_levels_seen),
        _ => "–".into(),
    }
}

fn api(supported: bool, avg: Option<f64>) -> String {
    match (supported, avg) {
        (false, _) => "not supported".into(),
        (true, Some(a)) => format!("yes · avg {a:.1}/event"),
        (true, None) => "yes".into(),
    }
}

fn ms(p: &Percentiles) -> String {
    if p.n == 0 {
        return "–".into();
    }
    format!("p50 {:.1} · p95 {:.1} · max {:.1} ms", p.p50, p.p95, p.max)
}
