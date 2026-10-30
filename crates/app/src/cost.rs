//! Cost & limits (spec §19): "$0.00 this month", each free allowance with
//! its reset time, warnings from 80%, Cloudflare's Free-plan limits, and the
//! confirm card that stands before anything that could cost money.

pub mod logic;

use dioxus::prelude::*;
use kabar_protocol::ai::EngineId;
use kabar_protocol::cloud::{ServiceUsage, UsageReport, UsageService};

use self::logic::Level;
use crate::account::{Cloud, problem_msg};
use crate::cloud::SyncState;
use crate::coach::Coach;
use crate::i18n::Msg;
use crate::{Screen, Ui};

#[component]
pub fn CostScreen() -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let status = cloud.status.read().clone();
    let connected = status.configured && status.sync != SyncState::SignedOut;
    let configured = status.configured;

    // Ask for fresh counters when the screen opens and whenever sync comes back.
    let state = use_memo(move || cloud.status.read().sync);
    use_effect(move || {
        if matches!(state(), SyncState::Live | SyncState::CatchingUp | SyncState::Offline) {
            cloud.send(crate::cloud::Command::LoadUsage);
        }
    });

    let usage = cloud.usage.read().clone();
    let report = match &usage {
        Some(Ok(r)) if connected => Some(r.clone()),
        _ => None,
    };
    let spend = report.as_ref().map_or(0, |r| r.spend_usd_cents);

    rsx! {
        div { class: "page-head",
            div {
                h1 { "{ui.t(Msg::CostTitle)}" }
            }
            if connected {
                button { class: "tonal", onclick: move |_| cloud.send(crate::cloud::Command::LoadUsage), "{ui.t(Msg::DevicesRefresh)}" }
            }
        }
        crate::ui::DivCard { class: "cost-head",
            p { class: "cost-total",
                bdi { class: "cost-amount latin", dir: "ltr", "{logic::dollars(spend)}" }
                " "
                span { class: "cost-period", "{ui.t(Msg::CostThisMonth)}" }
            }
            p { class: "muted", "{ui.t(Msg::CostHardStops)}" }
        }
        match (connected, usage) {
            (true, Some(Ok(_))) => rsx! {},
            (true, None) => rsx! {
                div { class: "card", p { class: "muted", aria_live: "polite", "{ui.t(Msg::CostLoading)}" } }
            },
            (true, Some(Err(p))) => rsx! {
                crate::ui::DivCard {
                    p { class: "notice", role: "status", "{ui.t(problem_msg(p))}" }
                    p { class: "muted", "{ui.t(Msg::CostOffline)}" }
                }
            },
            (false, _) => rsx! {
                // Signed out: the whole card opens sign-in (the button's click bubbles up).
                div {
                    class: if status.configured { "card card-action" } else { "card" },
                    onclick: move |_| {
                        if configured {
                            let mut s = ui.screen;
                            s.set(Screen::Devices);
                        }
                    },
                    p { "{ui.t(Msg::CostOffline)}" }
                    if status.configured {
                        button { "{ui.t(Msg::CloudSignIn)}" }
                    }
                }
            },
        }
        if let Some(report) = report {
            Allowances { report: report.clone() }
            Platform { report }
        } else {
            crate::ui::DivCard {
                h2 { "{ui.t(Msg::CostPlatformTitle)}" }
                p { class: "muted", "{ui.t(Msg::CostPlatformBody)}" }
            }
        }
        ClaudeCard {}
    }
}

#[component]
fn Allowances(report: UsageReport) -> Element {
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let (lang, eastern) = (ui.lang(), ui.eastern());
    let (now, offset) = (coach.now(), coach.offset());
    // Claude has its own card; unknown services from a newer server are skipped.
    let rows: Vec<ServiceUsage> = report
        .services
        .iter()
        .filter(|u| logic::service_name(u.service).is_some() && u.service != UsageService::Claude)
        .cloned()
        .collect();
    let warnings: Vec<String> =
        report.services.iter().filter_map(|u| logic::warning(u, now, offset, lang, eastern)).collect();

    rsx! {
        if !warnings.is_empty() {
            div { class: "card cost-warnings", role: "status",
                for w in warnings {
                    p { key: "{w}", class: "notice", "{w}" }
                }
            }
        }
        crate::ui::DivCard {
            h2 { "{ui.t(Msg::CostServicesTitle)}" }
            ul { class: "cost-list",
                for u in rows {
                    UsageRow { key: "{u.service:?}", usage: u }
                }
            }
        }
    }
}

#[component]
fn UsageRow(usage: ServiceUsage) -> Element {
    let ui = use_context::<Ui>();
    let coach = use_context::<Coach>();
    let (lang, eastern) = (ui.lang(), ui.eastern());
    let u = usage;
    let name = logic::service_name(u.service).unwrap_or_default();
    let pct = logic::percent(u.used, u.limit);
    let usable = u.enabled && u.configured;
    let level = if usable { logic::level(u.used, u.limit) } else { Level::Calm };
    let unit = logic::unit_msg(u.unit).map(|m| ui.t(m)).unwrap_or_default();
    let resets = ui.t(Msg::CostResets).replace(
        "{when}",
        &logic::when(logic::reset(u.resets_at_ms, coach.now(), coach.offset()), lang, eastern),
    );
    let used = logic::grouped(u.used, lang, eastern);
    let limit = logic::grouped(u.limit, lang, eastern);

    rsx! {
        li { class: "cost-row {level.class()}",
            div { class: "row spread",
                bdi { class: "cost-name latin", "{name}" }
                if !u.configured {
                    span { class: "pill", "{ui.t(Msg::CostNotSetUp)}" }
                } else if !u.enabled {
                    span { class: "pill", "{ui.t(Msg::CostOff)}" }
                }
            }
            div {
                class: "cost-bar",
                role: "progressbar",
                aria_valuemin: "0",
                aria_valuemax: "100",
                aria_valuenow: "{pct}",
                aria_label: "{name}",
                div { class: "cost-fill {level.class()}", style: "inline-size: {pct}%" }
            }
            div { class: "row spread cost-meta muted",
                span { bdi { "{used} / {limit}" } " {unit}" }
                span { "{resets}" }
            }
        }
    }
}

#[component]
fn Platform(report: UsageReport) -> Element {
    let ui = use_context::<Ui>();
    let (lang, eastern) = (ui.lang(), ui.eastern());
    rsx! {
        crate::ui::DivCard {
            h2 { "{ui.t(Msg::CostPlatformTitle)}" }
            p { class: "muted", "{ui.t(Msg::CostPlatformBody)}" }
            ul { class: "cost-platform",
                for (i, p) in report.platform.iter().enumerate() {
                    if let (Some(unit), Some(per)) = (logic::unit_msg(p.unit), logic::period_msg(p.period)) {
                        li { key: "{i}",
                            bdi { class: "latin", "{p.name}" }
                            span { class: "muted",
                                " · "
                                bdi { "{logic::grouped(p.limit, lang, eastern)}" }
                                " {ui.t(unit)} {ui.t(per)}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Claude, the one engine that could cost money. There is no engine switch in
/// the app yet (the AI router's `RouterSettings` keeps Claude disabled and
/// `allow_paid` off; Kabar Cloud has `CLAUDE_ENABLED`), so "Yes" explains the
/// server switch. When the AI engines screen lands, its Claude toggle must go
/// through [`PaidConfirm`] (see [`logic::needs_paid_confirm`]).
#[component]
fn ClaudeCard() -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let mut asking = use_signal(|| false);
    let mut said_yes = use_signal(|| false);
    let claude = match &*cloud.usage.read() {
        Some(Ok(r)) => r.services.iter().find(|u| u.service == UsageService::Claude).cloned(),
        _ => None,
    };
    let on = claude.as_ref().is_some_and(|c| c.enabled && c.configured);

    rsx! {
        crate::ui::DivCard {
            h2 { "{ui.t(Msg::CostClaudeTitle)}" }
            if on {
                p { "{ui.t(Msg::CostClaudeOnBody)}" }
                if let Some(c) = claude {
                    ul { class: "cost-list", UsageRow { usage: c } }
                }
            } else if said_yes() {
                p { class: "note", "{ui.t(Msg::CostClaudeHowTo)}" }
            } else if asking() {
                PaidConfirm {
                    engine: EngineId::Claude,
                    onanswer: move |yes: bool| {
                        asking.set(false);
                        said_yes.set(yes);
                    },
                }
            } else {
                p { class: "muted", "{ui.t(Msg::CostClaudeOffBody)}" }
                button { onclick: move |_| asking.set(true), "{ui.t(Msg::CostClaudeTurnOn)}" }
            }
        }
    }
}

/// "This could cost money": asked before any paid engine is switched on.
/// No is the default (focused, first, and the primary button).
#[component]
pub fn PaidConfirm(engine: EngineId, onanswer: EventHandler<bool>) -> Element {
    let ui = use_context::<Ui>();
    if !logic::needs_paid_confirm(engine) {
        // Free engines need no question: callers switch them directly.
        return rsx! {};
    }
    rsx! {
        div { class: "cost-confirm", role: "alertdialog", aria_labelledby: "cost-confirm-title",
            h3 { id: "cost-confirm-title", "{ui.t(Msg::CostConfirmTitle)}" }
            p { "{ui.t(Msg::CostConfirmBody)}" }
            div { class: "row",
                button { class: "primary", autofocus: true, "data-back": "true", onclick: move |_| onanswer.call(false), "{ui.t(Msg::CostConfirmNo)}" }
                button { onclick: move |_| onanswer.call(true), "{ui.t(Msg::CostConfirmYes)}" }
            }
        }
    }
}
