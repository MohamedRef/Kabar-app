//! Settings › Updates, the gentle "new Kabar is ready" card on
//! Today, and the hooks that follow the updater from the UI (installs on
//! Android, and what the "new version" notification needs and opens).

use dioxus::prelude::*;
use kabar_core::locale::{UiLanguage, eastern_arabic_digits};
use kabar_protocol::update::{Channel, UpdateNotes};

use super::sheet::{SheetMode, UpdateSheetNav};
use super::text::{U, more_changes, ut};
use super::{Failure, InstallTexts, Mode, NoticeContext, Off, Offer, Phase, Status, Updater};
use crate::coach::Coach;
use crate::settings::{Section, SettingsNav};
use crate::{Screen, Ui};

pub(super) const UPDATE_CSS: &str = r#"
.update-state { display: grid; gap: 0.4rem; padding-block: 0.6rem; padding-inline: 0.75rem;
  border-radius: 12px; background: var(--surface-2); margin-block: 0.5rem; }
.update-state p { margin: 0; }
.update-state progress { inline-size: 100%; accent-color: var(--sawah); }
.update-notes { white-space: pre-line; }
.update-notes-label { display: block; }
.update-covers { margin-block: 0.5rem 0; }
.update-found p { margin-block: 0.15rem; }
.row-copy small.update-waiting { color: var(--primary); font-weight: 600; }
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

pub(super) fn texts(lang: UiLanguage) -> InstallTexts {
    InstallTexts {
        lang,
        done_title: ut(lang, U::DoneTitle).into(),
        done_body: ut(lang, U::DoneBody).into(),
        ready_title: ut(lang, U::ReadyTitle).into(),
    }
}

/// Runs a blocking updater call off the UI thread.
pub(super) fn work(f: impl FnOnce(&'static Updater) + Send + 'static) {
    if let Some(u) = super::get() {
        let _ = std::thread::Builder::new().name("kabar-update".into()).spawn(move || f(u));
    }
}

fn notes(lang: UiLanguage, n: &UpdateNotes) -> String {
    super::notes_in(lang, n)
}

pub(super) fn megabytes(ui: &Ui, bytes: u64) -> String {
    let s = format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0));
    if ui.eastern() { eastern_arabic_digits(&s) } else { s }
}

/// What went wrong, in one line (the card's headline, the sheet's note).
/// A network failure with an offer in hand broke the download, not the check.
pub(super) fn failure_text(why: &Failure, offer: Option<&Offer>) -> U {
    match why {
        Failure::Network if offer.is_some() => U::FailedDownload,
        Failure::Network => U::FailedNetwork,
        Failure::Verify(_) => U::FailedVerify,
        Failure::Install(_) => U::FailedInstall,
        Failure::TooNew => U::TooNew,
        Failure::KeyChanged => U::KeyChanged,
    }
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
pub(super) fn primary_action(lang: UiLanguage, phase: &Phase) -> Option<(U, fn(UiLanguage))> {
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

/// What the updater is doing, in a headline (`None` before the first check).
fn headline(phase: &Phase) -> Option<U> {
    Some(match phase {
        Phase::Idle => return None,
        Phase::Checking => U::Checking,
        Phase::UpToDate => U::UpToDate,
        Phase::NotForThisDevice => U::NotForDevice,
        Phase::Available(_) => U::Available,
        Phase::Downloading { .. } => U::Downloading,
        Phase::Ready(_) => U::Ready,
        Phase::Installing(_) => U::Installing,
        Phase::NeedsPermission(_) => U::NeedsPermission,
        Phase::Installed(_) => U::Installed,
        Phase::Failed { why, offer } => failure_text(why, offer.as_ref()),
    })
}

/// "Last checked: Today 14:32", or "Not checked yet".
fn last_checked(ui: &Ui, s: &Status) -> String {
    let lang = ui.lang();
    match s.prefs.last_check_ms {
        Some(ms) => ut(lang, U::LastChecked).replace("{when}", &super::sheet::moment(ui, ms, s)),
        None => ut(lang, U::NeverChecked).to_owned(),
    }
}

/// The Updates row's line in the Settings list: live state, else when the
/// last check ran.
pub fn summary(ui: &Ui, s: &Status) -> String {
    let lang = ui.lang();
    if s.off.is_some() {
        return ut(lang, U::OffShort).into();
    }
    if s.checking_all {
        return ut(lang, U::Checking).into();
    }
    match &s.phase {
        Phase::Failed { offer: None, .. } => ut(lang, U::CheckFailedShort).into(),
        Phase::Idle | Phase::UpToDate | Phase::NotForThisDevice | Phase::Failed { .. } => {
            if s.content_waiting.is_some() {
                ut(lang, U::ContentWaitingShort).into()
            } else {
                last_checked(ui, s)
            }
        }
        p => headline(p).map(|h| ut(lang, h).to_owned()).unwrap_or_default(),
    }
}

/// Settings › Updates: where the app and its lessons stand, one button that
/// checks for both (and refreshes downloaded audio packs), and how updates
/// arrive.
#[component]
pub fn UpdatesCard() -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let status = use_update_status();
    let Some(s) = status.read().clone() else { return rsx! {} };
    let version = env!("CARGO_PKG_VERSION");
    let commit = super::commit();
    let busy = s.checking_all
        || matches!(s.phase, Phase::Checking | Phase::Downloading { .. } | Phase::Installing(_));
    let has_offer = matches!(
        s.phase,
        Phase::Available(_)
            | Phase::Downloading { .. }
            | Phase::Ready(_)
            | Phase::NeedsPermission(_)
            | Phase::Installed(_)
    );
    // What the last check found besides the app, once it's done.
    let done = !busy && s.prefs.last_check_ms.is_some();
    let content_found = s.content_check.filter(|_| done).map(|c| match c {
        super::content::Refresh::Current => U::ContentSame,
        super::content::Refresh::Taken(_) => U::ContentWaitingShort,
        super::content::Refresh::Failed => U::ContentFailed,
    });
    let packs = (done && s.packs_refreshed > 0)
        .then(|| ut(lang, U::PacksRefreshed).replace("{n}", &ui.num(s.packs_refreshed as u64)));
    let content_line = match (s.content_build, s.content_since_ms) {
        (0, _) => ut(lang, U::ContentBuiltIn).to_owned(),
        (b, since) => {
            let mut line = format!("{} · {} {}", ut(lang, U::ContentUpdated), ut(lang, U::Build), ui.num(b));
            if let Some(ms) = since {
                line += &format!(" · {}", super::sheet::moment(&ui, ms, &s));
            }
            line
        }
    };
    let app_icon = crate::icons::settings_section(if cfg!(target_os = "android") {
        Section::Android
    } else {
        Section::Mac
    });
    let sheet = try_use_context::<UpdateSheetNav>();
    let open_history = move |_| {
        if let Some(sheet) = sheet {
            sheet.open(SheetMode::History);
        }
    };

    rsx! {
        style { {UPDATE_CSS} }
        crate::ui::Card { element: crate::ui::CardElement::Div, class: "update-now",
            if let Some(off) = s.off {
                p { strong { "{ut(lang, U::OffShort)}" } }
                p { class: "muted settings-hint", role: "status",
                    {ut(lang, match off {
                        Off::DevBuild => U::OffDev,
                        Off::NoKey => U::OffKey,
                        Off::NoPlatform => U::OffPlatform,
                        Off::NotInBundle => U::OffBundle,
                    })}
                }
            } else {
                if s.checking_all && !matches!(s.phase, Phase::Checking) {
                    div { class: "update-state", role: "status", aria_live: "polite",
                        p { strong { "{ut(lang, U::Checking)}" } }
                    }
                } else {
                    PhaseView { phase: s.phase.clone(), auto: s.prefs.mode == Mode::Auto }
                }
                div { class: "update-found", role: "status", aria_live: "polite",
                    p { class: "muted small", "{last_checked(&ui, &s)}" }
                    if let Some(u) = content_found {
                        p { class: "small",
                            if u == U::ContentSame { "✓ " }
                            "{ut(lang, u)}"
                        }
                    }
                    if let Some(p) = packs {
                        p { class: "small", "✓ {p}" }
                    }
                }
                if s.content_waiting.is_some() {
                    div { class: "update-state",
                        p { "{ut(lang, U::ContentWaiting)}" }
                        if cfg!(target_os = "macos") {
                            div { class: "row",
                                button { class: "primary", onclick: move |_| restart_now(), "{ut(lang, U::RestartNow)}" }
                            }
                        } else {
                            p { class: "muted small", "{ut(lang, U::ReopenHint)}" }
                        }
                    }
                }
                p { class: "muted small update-covers", "{ut(lang, U::CheckCovers)}" }
                div { class: "row update-check",
                    button {
                        class: if has_offer || s.content_waiting.is_some() { "" } else { "primary" },
                        disabled: busy,
                        onclick: move |_| work(Updater::check_everything),
                        "{ut(lang, U::CheckNow)}"
                    }
                    if let Some(sheet) = sheet {
                        if has_offer {
                            button { onclick: move |_| sheet.open(SheetMode::Offer), "{ut(lang, U::WhatsNewAsk)}" }
                        }
                    }
                }
            }
        }

        div { class: "section-head", h2 { "{ut(lang, U::OnThisDevice)}" } }
        crate::ui::Card { element: crate::ui::CardElement::Div, class: "grouped",
            button { class: "activity-row", onclick: open_history,
                span { class: "row-icon", aria_hidden: "true", {app_icon} }
                span { class: "row-copy",
                    strong { "{ut(lang, U::AppRow)}" }
                    // Each Latin piece isolated on its own: inside one LTR run the
                    // Arabic word "build" pulled the numbers around it out of order.
                    small {
                        bdi { dir: "ltr", "{version}" }
                        if s.build > 0 {
                            " · {ut(lang, U::Build)} "
                            bdi { dir: "ltr", "{ui.num(s.build)}" }
                        }
                        if let Some(c) = commit {
                            " · "
                            bdi { dir: "ltr", class: "latin", "{c}" }
                        }
                    }
                }
                span { class: "row-end", span { class: "chev", aria_hidden: "true" } }
            }
            button { class: "activity-row", onclick: open_history,
                span { class: "row-icon", aria_hidden: "true", {crate::icons::settings_section(Section::Learning)} }
                span { class: "row-copy",
                    strong { "{ut(lang, U::ContentRow)}" }
                    small { "{content_line}" }
                    if s.content_waiting.is_some() {
                        small { class: "update-waiting", "{ut(lang, U::ContentWaitingShort)}" }
                    }
                }
                span { class: "row-end", span { class: "chev", aria_hidden: "true" } }
            }
            button { class: "activity-row", onclick: open_history,
                span { class: "row-icon", aria_hidden: "true", {crate::icons::settings_section(Section::Updates)} }
                span { class: "row-copy",
                    strong { "{ut(lang, U::History)}" }
                    small { "{ut(lang, U::HistoryHint)}" }
                }
                span { class: "row-end", span { class: "chev", aria_hidden: "true" } }
            }
        }

        if s.off.is_none() {
            div { class: "section-head", h2 { "{ut(lang, U::HowItWorks)}" } }
            crate::ui::Card { element: crate::ui::CardElement::Div, class: "",
                div { class: "field",
                    span { class: "label", id: "update-mode-label",
                        "{ut(lang, U::Mode)}"
                        crate::settings::Scope { device_only: true }
                    }
                    div { class: "segmented", role: "radiogroup", aria_labelledby: "update-mode-label",
                        for m in Mode::ALL {
                            button {
                                key: "{m:?}",
                                role: "radio",
                                aria_checked: if s.prefs.mode == m { "true" } else { "false" },
                                onclick: move |_| {
                                    if let Some(u) = super::get() {
                                        u.set_mode(m);
                                    }
                                },
                                {ut(lang, match m { Mode::Auto => U::ModeAuto, Mode::Notify => U::ModeNotify, Mode::Off => U::ModeOff })}
                            }
                        }
                    }
                    p { class: "muted small",
                        {ut(lang, match s.prefs.mode { Mode::Auto => U::ModeAutoHint, Mode::Notify => U::ModeNotifyHint, Mode::Off => U::ModeOffHint })}
                    }
                }
                if s.prefs.mode != Mode::Off {
                    div { class: "field",
                        label { class: "switch-row",
                            input {
                                class: "switch",
                                role: "switch",
                                r#type: "checkbox",
                                checked: s.prefs.notify,
                                onchange: move |e: FormEvent| {
                                    if let Some(u) = super::get() {
                                        u.set_notify(e.checked());
                                    }
                                },
                            }
                            " {ut(lang, U::NotifyToggle)}"
                            crate::settings::Scope { device_only: true }
                        }
                        p { class: "muted small", "{ut(lang, U::NotifyToggleHint)}" }
                    }
                }
                div { class: "field",
                    span { class: "label", id: "update-channel-label",
                        "{ut(lang, U::Channel)}"
                        crate::settings::Scope { device_only: true }
                    }
                    if s.preview {
                        span { class: "pill", "{ut(lang, U::Preview)}" }
                        p { class: "muted small", "{ut(lang, U::PreviewHint)}" }
                    } else {
                        div { class: "segmented", role: "radiogroup", aria_labelledby: "update-channel-label",
                            for c in Channel::RELEASE {
                                button {
                                    key: "{c.as_str()}",
                                    role: "radio",
                                    aria_checked: if s.channel == c { "true" } else { "false" },
                                    disabled: busy,
                                    onclick: move |_| work(move |u| u.set_channel(c)),
                                    "{ut(lang, channel_label(c))}"
                                }
                            }
                        }
                        p { class: "muted small", "{ut(lang, U::ChannelHint)}" }
                    }
                }
                p { class: "muted small", "{ut(lang, U::Safety)}" }
                p { class: "muted small", "{ut(lang, U::Packs)}" }
            }
        }
    }
}

fn restart_now() {
    #[cfg(target_os = "macos")]
    super::restart();
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
    let headline = headline(&phase);
    let Some(headline) = headline else { return rsx! {} };
    let detail = match &phase {
        Phase::Failed { why: Failure::Verify(d) | Failure::Install(d), .. } => Some(d.clone()),
        _ => None,
    };
    let progress = match &phase {
        Phase::Downloading { offer, done } => Some((*done, offer.asset.size.max(1))),
        _ => None,
    };
    let what_new = offer
        .as_ref()
        .map(|o| {
            if o.changes.is_empty() {
                notes(lang, &o.notes)
            } else {
                super::changelog::notice_lines(lang, &o.changes, |n| more_changes(lang, n))
            }
        })
        .filter(|n| !n.is_empty());
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
                        span { class: "muted",
                            " · "
                            bdi { dir: "ltr", "{megabytes(&ui, o.asset.size)}" }
                        }
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
                        span { class: "muted update-notes-label", "{ut(lang, U::WhatsNew)}" }
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
/// next version. Without one, new lessons that came over the air get a
/// short note until Kabar is reopened.
#[component]
pub fn UpdateBanner() -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    let status = use_update_status();
    let mut hide_content = use_signal(|| false);
    let Some(s) = status.read().clone() else { return rsx! {} };
    let Some(offer) = s.banner().cloned() else {
        // New lessons downloaded over the air: a quiet note until reopened.
        if s.content_waiting.is_none() || hide_content() {
            return rsx! {};
        }
        return rsx! {
            style { {UPDATE_CSS} }
            crate::ui::Card { class: "update-banner", role: "status",
                div { class: "ub-text",
                    p { strong { "{ut(lang, U::ContentWaitingShort)}" } }
                    p { class: "muted small", "{ut(lang, U::ReopenHint)}" }
                }
                div { class: "row",
                    if cfg!(target_os = "macos") {
                        button { class: "primary", onclick: move |_| restart_now(), "{ut(lang, U::RestartNow)}" }
                    }
                    button { onclick: move |_| hide_content.set(true), "{ut(lang, U::GotIt)}" }
                }
            }
        };
    };
    let Some((label, run)) = primary_action(lang, &s.phase) else { return rsx! {} };
    let headline = match s.phase {
        Phase::Available(_) => U::Available,
        Phase::Installed(_) => U::Installed,
        Phase::NeedsPermission(_) => U::NeedsPermission,
        _ => U::Ready,
    };
    // The banner stays short: two lines, the sheet has the rest.
    let what_new = if offer.changes.is_empty() {
        notes(lang, &offer.notes)
    } else {
        super::changelog::lines_upto(lang, &offer.changes, |n| more_changes(lang, n), 2)
    };
    let sheet = try_use_context::<UpdateSheetNav>();
    rsx! {
        style { {UPDATE_CSS} }
        crate::ui::Card { class: "update-banner", role: "status",
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
                if let Some(sheet) = sheet {
                    button { onclick: move |_| sheet.open(SheetMode::Offer), "{ut(lang, U::WhatsNewAsk)}" }
                }
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

/// Keeps the updater's copy of the language and quiet hours current (the
/// background job words its notification with them), and follows a tap on
/// the notification: the "what's new" sheet opens, and the notification's
/// button starts the download or the install. Mounted once in the shell.
pub fn use_update_notices(ui: Ui, nav: SettingsNav) {
    let coach = try_use_context::<Coach>();
    let sheet = try_use_context::<UpdateSheetNav>();
    use_effect(move || {
        let lang = ui.lang();
        let (quiet, offset) = match coach {
            Some(c) => {
                let quiet = c.profile.read().as_ref().map(|p| p.quiet_hours);
                let offset = (*c.device_offset.read()).unwrap_or_else(|| c.offset());
                (quiet, offset)
            }
            None => (None, 0),
        };
        if let Some(u) = super::get() {
            u.set_notice_context(NoticeContext { lang, quiet, utc_offset_min: offset });
        }
    });
    use_future(move || async move {
        if !cfg!(target_os = "android") || super::get().is_none() {
            return;
        }
        let mut eval = document::eval(
            r#"
            // The tap's activity is translucent: a tap while Kabar is open
            // may never hide the page, so look on focus and every 2 s too.
            const send = () => { if (!document.hidden) dioxus.send(true); };
            document.addEventListener("visibilitychange", send);
            window.addEventListener("focus", send);
            window.addEventListener("pageshow", send);
            setInterval(send, 2000);
            send();
            await new Promise(() => {});
            "#,
        );
        while eval.recv::<bool>().await.is_ok() {
            let Some(tap) = crate::platform::update_notice::take_tap() else { continue };
            // The sheet says what's new and holds the button; without it
            // (never in the shell), the Updates card does.
            match sheet {
                Some(sheet) => sheet.open_offer(),
                None => nav.show(Some(Section::Updates)),
            }
            let lang = ui.lang();
            match tap.as_str() {
                "download" => work(Updater::download),
                "install" => {
                    let t = texts(lang);
                    work(move |u| u.install(t));
                }
                _ => {}
            }
        }
    });
}
