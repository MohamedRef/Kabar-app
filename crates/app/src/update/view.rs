//! Settings › About › Updates, the gentle "new Kabar is ready" card on
//! Today, and the hooks that follow the updater from the UI.

use dioxus::prelude::*;
use kabar_core::locale::{UiLanguage, eastern_arabic_digits};
use kabar_protocol::update::{Channel, UpdateNotes};

use super::text::{U, ut};
use super::{Failure, InstallTexts, Mode, Off, Offer, Phase, Status, Updater};
use crate::{Screen, Ui};

const UPDATE_CSS: &str = r#"
.update-state { display: grid; gap: 0.4rem; padding-block: 0.6rem; padding-inline: 0.75rem;
  border-radius: 12px; background: var(--surface-2); margin-block: 0.5rem; }
.update-state p { margin: 0; }
.update-state progress { inline-size: 100%; accent-color: var(--sawah); }
.update-notes { white-space: pre-line; }
.update-check { margin-block-end: 0.75rem; }
.update-modes { display: grid; gap: 0.35rem; }
.update-modes label { display: grid; grid-template-columns: auto 1fr; column-gap: 0.5rem; align-items: start; }
.update-modes .muted { grid-column: 2; font-size: 0.85rem; }
.update-banner { display: flex; flex-wrap: wrap; gap: 0.5rem 0.75rem; align-items: center;
  border-inline-start: 4px solid var(--sawah); }
.update-banner .ub-text { flex: 1 1 14rem; min-inline-size: 0; }
.update-banner .ub-text p { margin: 0; }
"#;

/// The updater's status, following its thread.
pub fn use_update_status() -> Signal<Option<Status>> {
    let mut status = use_signal(|| super::get().map(Updater::status));
    use_future(move || async move {
        let Some(u) = super::get() else { return };
        let mut rx = u.subscribe();
        while rx.changed().await.is_ok() {
            let now = rx.borrow_and_update().clone();
            if status.peek().as_ref() != Some(&now) {
                status.set(Some(now));
            }
        }
    });
    status
}

fn texts(lang: UiLanguage) -> InstallTexts {
    InstallTexts {
        done_title: ut(lang, U::DoneTitle).into(),
        done_body: ut(lang, U::DoneBody).into(),
        ready_title: ut(lang, U::ReadyTitle).into(),
    }
}

/// Runs a blocking updater call off the UI thread.
fn work(f: impl FnOnce(&'static Updater) + Send + 'static) {
    if let Some(u) = super::get() {
        let _ = std::thread::Builder::new().name("kabar-update".into()).spawn(move || f(u));
    }
}

fn notes(lang: UiLanguage, n: &UpdateNotes) -> String {
    let pick = |a: &str, b: &str, c: &str| {
        [a, b, c].into_iter().find(|s| !s.trim().is_empty()).unwrap_or("").to_owned()
    };
    match lang {
        UiLanguage::ArEg => pick(&n.ar_eg, &n.ar, &n.en),
        UiLanguage::Ar => pick(&n.ar, &n.ar_eg, &n.en),
        UiLanguage::En => pick(&n.en, &n.ar_eg, &n.ar),
    }
}

fn megabytes(ui: &Ui, bytes: u64) -> String {
    let s = format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0));
    if ui.eastern() { eastern_arabic_digits(&s) } else { s }
}

fn channel_label(c: Channel) -> U {
    match c {
        Channel::Preview => U::Preview,
        Channel::Alpha => U::Alpha,
        Channel::Beta => U::Beta,
        Channel::Stable => U::Stable,
    }
}

/// The button that moves an offer forward, for the card and the banner.
fn primary_action(lang: UiLanguage, phase: &Phase) -> Option<(U, fn(UiLanguage))> {
    fn download(_: UiLanguage) {
        work(Updater::download);
    }
    fn install(lang: UiLanguage) {
        let t = texts(lang);
        work(move |u| u.install(t));
    }
    fn restart(_: UiLanguage) {
        #[cfg(target_os = "macos")]
        super::restart();
    }
    let _ = lang;
    match phase {
        Phase::Available(_) => Some((U::Download, download)),
        Phase::Ready(_) if cfg!(target_os = "macos") => Some((U::InstallRestart, install)),
        Phase::Ready(_) => Some((U::InstallNow, install)),
        Phase::NeedsPermission(_) => Some((U::TryAgain, install)),
        Phase::Installed(_) => Some((U::RestartNow, restart)),
        Phase::Failed { offer: Some(_), why: Failure::Install(_) } => Some((U::TryAgain, install)),
        Phase::Failed { offer: Some(_), why: Failure::Network | Failure::Verify(_) } => {
            Some((U::TryAgain, download))
        }
        _ => None,
    }
}

/// Settings › About › Updates.
#[component]
pub fn UpdatesCard() -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let status = use_update_status();
    let Some(s) = status.read().clone() else { return rsx! {} };
    let version = env!("CARGO_PKG_VERSION");
    let build = super::get().map_or(0, |u| u.info.build);
    let commit = super::commit();
    let busy = matches!(s.phase, Phase::Checking | Phase::Downloading { .. } | Phase::Installing(_));

    rsx! {
        style { {UPDATE_CSS} }
        section { class: "card", aria_labelledby: "updates-title",
            h2 { id: "updates-title", "{ut(lang, U::Title)}" }
            div { class: "field",
                span { class: "label", "{ut(lang, U::ThisBuild)}" }
                // Each Latin piece isolated on its own: inside one LTR run the
                // Arabic word "build" pulled the numbers around it out of order.
                span {
                    code { dir: "ltr", bdi { "{version}" } }
                    if build > 0 {
                        " · {ut(lang, U::Build)} "
                        code { dir: "ltr", bdi { "{build}" } }
                    }
                    if let Some(c) = commit {
                        " · "
                        code { dir: "ltr", bdi { "{c}" } }
                    }
                }
            }
            if let Some(off) = s.off {
                p { class: "muted settings-hint", role: "status",
                    {ut(lang, match off {
                        Off::DevBuild => U::OffDev,
                        Off::NoKey => U::OffKey,
                        Off::NoPlatform => U::OffPlatform,
                        Off::NotInBundle => U::OffBundle,
                    })}
                }
            } else {
                PhaseView { phase: s.phase.clone(), auto: s.prefs.mode == Mode::Auto }
                div { class: "row update-check",
                    button {
                        disabled: busy,
                        onclick: move |_| work(|u| u.check(false)),
                        "{ut(lang, U::CheckNow)}"
                    }
                }
                div { class: "field",
                    span { class: "label", "{ut(lang, U::Mode)}" crate::settings::Scope { device_only: true } }
                    div { class: "update-modes", role: "radiogroup",
                        for m in Mode::ALL {
                            label { key: "{m:?}",
                                input {
                                    r#type: "radio",
                                    name: "update-mode",
                                    checked: s.prefs.mode == m,
                                    onchange: move |_| {
                                        if let Some(u) = super::get() {
                                            u.set_mode(m);
                                        }
                                    },
                                }
                                span {
                                    {ut(lang, match m { Mode::Auto => U::ModeAuto, Mode::Notify => U::ModeNotify, Mode::Off => U::ModeOff })}
                                }
                                span { class: "muted",
                                    {ut(lang, match m { Mode::Auto => U::ModeAutoHint, Mode::Notify => U::ModeNotifyHint, Mode::Off => U::ModeOffHint })}
                                }
                            }
                        }
                    }
                }
                div { class: "field",
                    span { class: "label", "{ut(lang, U::Channel)}" crate::settings::Scope { device_only: true } }
                    if s.preview {
                        span { class: "pill", "{ut(lang, U::Preview)}" }
                        p { class: "muted settings-hint", "{ut(lang, U::PreviewHint)}" }
                    } else {
                        div { class: "row", role: "group",
                            for c in Channel::RELEASE {
                                button {
                                    key: "{c.as_str()}",
                                    aria_pressed: if s.channel == c { "true" } else { "false" },
                                    disabled: busy,
                                    onclick: move |_| work(move |u| u.set_channel(c)),
                                    "{ut(lang, channel_label(c))}"
                                }
                            }
                        }
                        p { class: "muted settings-hint", "{ut(lang, U::ChannelHint)}" }
                    }
                }
            }
            p { class: "muted settings-hint", "{ut(lang, U::Safety)}" }
            p { class: "muted settings-hint", "{ut(lang, U::Packs)}" }
        }
    }
}

#[component]
fn PhaseView(phase: Phase, auto: bool) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let action = primary_action(lang, &phase);
    let offer: Option<Offer> = match &phase {
        Phase::Available(o)
        | Phase::Ready(o)
        | Phase::Installing(o)
        | Phase::NeedsPermission(o)
        | Phase::Installed(o)
        | Phase::Downloading { offer: o, .. } => Some(o.clone()),
        Phase::Failed { offer, .. } => offer.clone(),
        _ => None,
    };
    let headline = match &phase {
        Phase::Idle => None,
        Phase::Checking => Some(U::Checking),
        Phase::UpToDate => Some(U::UpToDate),
        Phase::NotForThisDevice => Some(U::NotForDevice),
        Phase::Available(_) => Some(U::Available),
        Phase::Downloading { .. } => Some(U::Downloading),
        Phase::Ready(_) => Some(U::Ready),
        Phase::Installing(_) => Some(U::Installing),
        Phase::NeedsPermission(_) => Some(U::NeedsPermission),
        Phase::Installed(_) => Some(U::Installed),
        Phase::Failed { why: Failure::Network, .. } => Some(U::FailedNetwork),
        Phase::Failed { why: Failure::Verify(_), .. } => Some(U::FailedVerify),
        Phase::Failed { why: Failure::Install(_), .. } => Some(U::FailedInstall),
        Phase::Failed { why: Failure::TooNew, .. } => Some(U::TooNew),
        Phase::Failed { why: Failure::KeyChanged, .. } => Some(U::KeyChanged),
    };
    let Some(headline) = headline else { return rsx! {} };
    let detail = match &phase {
        Phase::Failed { why: Failure::Verify(d) | Failure::Install(d), .. } => Some(d.clone()),
        _ => None,
    };
    let progress = match &phase {
        Phase::Downloading { offer, done } => Some((*done, offer.asset.size.max(1))),
        _ => None,
    };
    let what_new = offer.as_ref().map(|o| notes(lang, &o.notes)).filter(|n| !n.is_empty());
    rsx! {
        div { class: "update-state", role: "status", aria_live: "polite",
            p {
                strong {
                    if matches!(phase, Phase::UpToDate) { "✓ " }
                    "{ut(lang, headline)}"
                }
                if let Some(o) = offer.as_ref().filter(|_| !matches!(phase, Phase::Failed { .. })) {
                    " "
                    bdi { dir: "ltr", class: "latin", "{o.label()}" }
                    if matches!(phase, Phase::Available(_)) {
                        span { class: "muted", " · {megabytes(&ui, o.asset.size)}" }
                    }
                }
            }
            if let Some((done, total)) = progress {
                progress { max: "{total}", value: "{done}" }
            }
            if matches!(phase, Phase::Installing(_)) && cfg!(target_os = "android") {
                p { class: "muted", "{ut(lang, U::InstallingAndroid)}" }
            }
            if matches!(phase, Phase::Available(_)) && auto && cfg!(target_os = "android") {
                p { class: "muted", "{ut(lang, U::WaitingWifi)}" }
            }
            if matches!(phase, Phase::Installed(_)) {
                p { class: "muted", "{ut(lang, U::NextLaunch)}" }
            }
            if let Some(n) = what_new {
                if !matches!(phase, Phase::Failed { .. }) {
                    p { class: "update-notes",
                        span { class: "muted", "{ut(lang, U::WhatsNew)}: " }
                        "{n}"
                    }
                }
            }
            if let Some(d) = detail {
                p { class: "muted lp-small", dir: "ltr", bdi { "{d}" } }
            }
            if let Some((label, run)) = action {
                div { class: "row",
                    button { class: "primary", onclick: move |_| run(lang), "{ut(lang, label)}" }
                    if matches!(phase, Phase::NeedsPermission(_)) {
                        button { onclick: move |_| open_install_permission(), "{ut(lang, U::OpenPermission)}" }
                    }
                }
            }
        }
    }
}

fn open_install_permission() {
    #[cfg(target_os = "android")]
    if let Err(e) = crate::platform::android::open_install_permission() {
        crate::diag::report!("{e}");
    }
}

/// The card on Today once there's something to do: a download to start
/// (Tell me first), an install, or a restart. Closing it hides it until the
/// next version.
#[component]
pub fn UpdateBanner() -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let status = use_update_status();
    let Some(s) = status.read().clone() else { return rsx! {} };
    let Some(offer) = s.banner().cloned() else { return rsx! {} };
    let Some((label, run)) = primary_action(lang, &s.phase) else { return rsx! {} };
    let headline = match s.phase {
        Phase::Available(_) => U::Available,
        Phase::Installed(_) => U::Installed,
        Phase::NeedsPermission(_) => U::NeedsPermission,
        _ => U::Ready,
    };
    let what_new = notes(lang, &offer.notes);
    rsx! {
        style { {UPDATE_CSS} }
        section { class: "card update-banner", role: "status",
            div { class: "ub-text",
                p {
                    strong { "{ut(lang, headline)}" }
                    " "
                    bdi { dir: "ltr", class: "latin", "{offer.label()}" }
                }
                if !what_new.is_empty() {
                    p { class: "muted update-notes", "{what_new}" }
                }
            }
            div { class: "row",
                button { class: "primary", onclick: move |_| run(lang), "{ut(lang, label)}" }
                button {
                    onclick: move |_| {
                        if let Some(u) = super::get() {
                            u.dismiss();
                        }
                    },
                    "{ut(lang, U::Later)}"
                }
            }
        }
    }
}

/// Android: follows an install in progress, picks up after the learner
/// allowed "Install unknown apps", and, in automatic mode, installs a ready
/// update quietly while Kabar is in the background (never mid-lesson, and
/// only where Android needs no confirmation). Mounted once in the shell.
pub fn use_android_install(ui: Ui) {
    let status = use_update_status();
    use_future(move || async move {
        if !cfg!(target_os = "android") || super::get().is_none() {
            return;
        }
        let mut eval = document::eval(
            r#"
            const send = () => dioxus.send(document.hidden);
            document.addEventListener("visibilitychange", send);
            setInterval(() => dioxus.send(null), 1000);
            await new Promise(() => {});
            "#,
        );
        while let Ok(hidden) = eval.recv::<Option<bool>>().await {
            let Some(s) = status.peek().clone() else { continue };
            let lang = ui.lang();
            match (&s.phase, hidden) {
                (Phase::Installing(_), _) => {
                    #[cfg(target_os = "android")]
                    if let Some(u) = super::get() {
                        // Back in front: a confirmation that arrived in the
                        // background (shown as a notification) opens now.
                        if hidden == Some(false) {
                            let _ = crate::platform::android::show_pending_confirm();
                        }
                        u.poll_install();
                    }
                }
                (Phase::NeedsPermission(_), Some(false)) if install_allowed() => {
                    let t = texts(lang);
                    work(move |u| u.install(t));
                }
                (Phase::Ready(o), Some(true)) if s.prefs.mode == Mode::Auto => {
                    let calm = !matches!(*ui.screen.peek(), Screen::Lesson | Screen::LevelCheck);
                    let tried = s.prefs.auto_failed.as_deref() == Some(o.key().as_str());
                    if calm && !tried && install_quietly() {
                        let t = texts(lang);
                        work(move |u| u.install(t));
                    }
                }
                _ => {}
            }
        }
    });
}

fn install_allowed() -> bool {
    #[cfg(target_os = "android")]
    {
        crate::platform::android::can_request_installs().unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}

fn install_quietly() -> bool {
    #[cfg(target_os = "android")]
    {
        install_allowed() && crate::platform::android::can_install_quietly().unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}
