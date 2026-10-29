//! Account & devices: Google sign-in, sync status, the Devices list with
//! remote sign-out, and a small "save a word" box to watch sync happen.

use dioxus::prelude::*;
use kabar_core::locale::{UiLanguage, format_number};
use kabar_protocol::cloud::{DeviceSummary, Platform, UsageReport};
use kabar_protocol::sync::OpKind;
use tokio::sync::mpsc::UnboundedSender;

use crate::cloud::{CloudStatus, Command, Problem, SyncState};
use crate::i18n::Msg;
use crate::{Db, Ui};

/// Kabar Cloud state for the UI, shared through context.
#[derive(Clone, Copy)]
pub struct Cloud {
    pub status: Signal<CloudStatus>,
    pub devices: Signal<Option<Result<Vec<DeviceSummary>, Problem>>>,
    /// The cost dashboard's counters, once asked for.
    pub usage: Signal<Option<Result<UsageReport, Problem>>>,
    pub signing_in: Signal<bool>,
    /// Bumped when remote ops change rows the UI shows.
    pub revision: Signal<u64>,
    pub commands: Signal<Option<UnboundedSender<Command>>>,
}

impl Cloud {
    pub fn send(&self, command: Command) {
        if let Some(tx) = &*self.commands.peek() {
            tx.send(command).ok();
        }
    }
}

pub fn problem_msg(p: Problem) -> Msg {
    match p {
        Problem::NotConfigured => Msg::CloudNotConfigured,
        Problem::NotAllowed => Msg::ProblemNotAllowed,
        Problem::SignInCancelled => Msg::ProblemSignInCancelled,
        Problem::SignInFailed => Msg::ProblemSignInFailed,
        Problem::SignedOutRemotely => Msg::ProblemSignedOutRemotely,
        Problem::Network => Msg::ProblemNetwork,
        Problem::Server => Msg::ProblemServer,
        Problem::Outdated => Msg::ProblemOutdated,
    }
}

pub(crate) fn sync_msg(s: SyncState) -> Option<Msg> {
    match s {
        SyncState::SignedOut => None,
        SyncState::Connecting => Some(Msg::SyncConnecting),
        SyncState::CatchingUp => Some(Msg::SyncCatchingUp),
        SyncState::Live => Some(Msg::SyncLive),
        SyncState::Offline => Some(Msg::SyncOffline),
        SyncState::Stopped => Some(Msg::SyncStopped),
    }
}

/// "3 min ago" with Arabic dual and plural forms.
pub fn ago(lang: UiLanguage, eastern: bool, then_ms: u64, now_ms: u64) -> String {
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    if secs < 60 {
        return crate::i18n::t(lang, Msg::JustNow).to_string();
    }
    let (n, unit) = match secs {
        s if s < 3600 => (s / 60, 0),
        s if s < 86_400 => (s / 3600, 1),
        s => (s / 86_400, 2),
    };
    let num = format_number(n, lang, eastern);
    match lang {
        UiLanguage::En => {
            let unit = ["min", "h", "d"][unit];
            format!("{num} {unit} ago")
        }
        UiLanguage::ArEg | UiLanguage::Ar => {
            // [one, two, 3–10, 11+]
            let forms = [
                ["دقيقة", "دقيقتين", "دقايق", "دقيقة"],
                ["ساعة", "ساعتين", "ساعات", "ساعة"],
                ["يوم", "يومين", "أيام", "يوم"],
            ][unit];
            let forms = if lang == UiLanguage::Ar && unit == 0 {
                ["دقيقة", "دقيقتين", "دقائق", "دقيقة"]
            } else {
                forms
            };
            let since = if lang == UiLanguage::ArEg { "من" } else { "منذ" };
            match n {
                1 => format!("{since} {}", forms[0]),
                2 => format!("{since} {}", forms[1]),
                3..=10 => format!("{since} {num} {}", forms[2]),
                _ => format!("{since} {num} {}", forms[3]),
            }
        }
    }
}

fn now_ms() -> u64 {
    crate::cloud::engine::now_ms()
}

#[component]
pub fn AccountScreen() -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let status = cloud.status.read().clone();
    let signed_in = status.email.is_some() || status.sync != SyncState::SignedOut;

    // Load the device list when this screen opens, and again after sign-in.
    let state = use_memo(move || cloud.status.read().sync);
    use_effect(move || {
        if matches!(state(), SyncState::Live | SyncState::CatchingUp) {
            cloud.send(Command::LoadDevices);
        }
    });

    rsx! {
        div { class: "card",
            h2 { bdi { lang: "en", class: "latin", "Kabar Cloud" } }
            if !status.configured {
                p { class: "muted", "{ui.t(Msg::CloudNotConfigured)}" }
            } else if signed_in {
                SignedIn { status: status.clone() }
            } else {
                p { "{ui.t(Msg::CloudIntro)}" }
                if *cloud.signing_in.read() {
                    p { class: "muted", aria_live: "polite", "{ui.t(Msg::CloudSigningIn)}" }
                } else {
                    button { class: "primary", onclick: move |_| cloud.send(Command::SignInGoogle), "{ui.t(Msg::CloudSignIn)}" }
                }
            }
            if let Some(p) = status.problem {
                p { class: "notice", role: "status", "{ui.t(problem_msg(p))}" }
            }
        }
        if signed_in {
            DevicesCard {}
            TrySyncCard {}
        }
    }
}

#[component]
fn SignedIn(status: CloudStatus) -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let settings = ui.settings.read().clone();
    let email = status.email.clone().unwrap_or_default();
    let last = status.last_sync_ms.map(|ms| ago(settings.ui_language, settings.eastern_digits, ms, now_ms()));
    let pending = format_number(status.pending, settings.ui_language, settings.eastern_digits);

    rsx! {
        div { class: "field",
            span { class: "label", "{ui.t(Msg::CloudSignedInAs)}" }
            bdi { class: "latin", "{email}" }
        }
        if status.sync == SyncState::Offline {
            crate::misc_art::Picture { art: crate::misc_art::Art::Offline, class: "empty-art small" }
        }
        div { class: "field row",
            if let Some(msg) = sync_msg(status.sync) {
                span {
                    class: if status.sync == SyncState::Live { "pill ok" } else { "pill" },
                    aria_live: "polite",
                    "{ui.t(msg)}"
                }
            }
            if status.pending > 0 {
                span { class: "muted", "{pending} {ui.t(Msg::SyncPending)}" }
            }
            if let Some(last) = last {
                span { class: "muted", "{ui.t(Msg::SyncLast)}: {last}" }
            }
        }
        button { onclick: move |_| cloud.send(Command::SignOut), "{ui.t(Msg::CloudSignOut)}" }
    }
}

#[component]
fn DevicesCard() -> Element {
    let ui = use_context::<Ui>();
    let cloud = use_context::<Cloud>();
    let settings = ui.settings.read().clone();
    let devices = cloud.devices.read().clone();
    let now = now_ms();

    rsx! {
        div { class: "card",
            div { class: "row spread",
                h2 { "{ui.t(Msg::DevicesTitle)}" }
                button { onclick: move |_| cloud.send(Command::LoadDevices), "{ui.t(Msg::DevicesRefresh)}" }
            }
            match devices {
                None => rsx! { p { class: "muted", "…" } },
                Some(Err(p)) => rsx! { p { class: "notice", "{ui.t(problem_msg(p))}" } },
                Some(Ok(list)) => rsx! {
                    ul { class: "devices",
                        for d in list {
                            li {
                                key: "{d.device_id}",
                                class: if d.revoked { "device revoked" } else { "device" },
                                div { class: "device-icon", aria_hidden: "true", {platform_icon(d.platform)} }
                                div { class: "device-main",
                                    div { class: "row",
                                        bdi { class: "device-name", "{d.name}" }
                                        if d.current {
                                            span { class: "pill", "{ui.t(Msg::DeviceThis)}" }
                                        }
                                    }
                                    div { class: "muted",
                                        if let Some(model) = &d.model {
                                            bdi { class: "latin", "{model}" }
                                            " · "
                                        }
                                        if d.revoked {
                                            "{ui.t(Msg::DeviceSignedOut)}"
                                        } else if let Some(ms) = d.last_sync_ms {
                                            "{ui.t(Msg::SyncLast)}: {ago(settings.ui_language, settings.eastern_digits, ms, now)}"
                                        } else {
                                            "{ui.t(Msg::DeviceNeverSynced)}"
                                        }
                                    }
                                }
                                if !d.current && !d.revoked {
                                    button {
                                        onclick: {
                                            let id = d.device_id.clone();
                                            move |_| cloud.send(Command::RevokeDevice(id.clone()))
                                        },
                                        "{ui.t(Msg::DeviceSignOut)}"
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

/// Saves a word to `word_status` and lists what this device holds, so a
/// phone → tablet round trip can be watched by eye.
#[component]
fn TrySyncCard() -> Element {
    let ui = use_context::<Ui>();
    let db = use_context::<Db>();
    let cloud = use_context::<Cloud>();
    let mut word = use_signal(String::new);
    // Re-read when remote ops arrive.
    let _ = cloud.revision.read();
    let words: Vec<String> =
        db.0.borrow()
            .synced_rows("word_status")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(_, p)| p.get("lexeme").and_then(|l| l.as_str()).map(String::from))
            .take(12)
            .collect();

    let mut save = {
        let db = db.clone();
        move || {
            let lexeme = word.peek().trim().to_lowercase();
            if lexeme.is_empty() || lexeme.chars().count() > 64 {
                return;
            }
            let payload = serde_json::json!({ "lexeme": lexeme, "lang": "id", "status": "learning" });
            let result = db.0.borrow_mut().write_synced(
                "word_status",
                &format!("id:{lexeme}"),
                OpKind::Upsert,
                Some(payload),
            );
            match result {
                Ok(_) => {
                    word.set(String::new());
                    let mut rev = cloud.revision;
                    rev += 1;
                    cloud.send(Command::Kick);
                }
                Err(e) => crate::diag::report!("saving the word failed: {e}"),
            }
        }
    };

    rsx! {
        div { class: "card",
            h2 { "{ui.t(Msg::TrySyncTitle)}" }
            p { class: "muted", "{ui.t(Msg::TrySyncBody)}" }
            form {
                class: "row",
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    save();
                },
                input {
                    class: "latin",
                    lang: "id",
                    dir: "ltr",
                    placeholder: "rumah",
                    autocapitalize: "off",
                    value: "{word}",
                    oninput: move |e: FormEvent| word.set(e.value()),
                }
                button { class: "primary", r#type: "submit", "{ui.t(Msg::TrySyncSave)}" }
            }
            if !words.is_empty() {
                p { class: "label", "{ui.t(Msg::TrySyncSaved)}" }
                div { class: "row",
                    for w in words {
                        span { key: "{w}", class: "pill latin", lang: "id", bdi { "{w}" } }
                    }
                }
            }
        }
    }
}

fn platform_icon(p: Platform) -> Element {
    let d = match p {
        // phone
        Platform::Android => "M8 3h8v18H8zM11 18h2",
        // laptop
        Platform::Macos => "M5 6h14v9H5zM3 18h18",
        Platform::Web | Platform::Other => "M4 5h16v11H4zM9 20h6M12 16v4",
    };
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    #[test]
    fn relative_times_use_arabic_number_forms() {
        let now = 100 * 86_400_000;
        let eg = |ms| ago(UiLanguage::ArEg, false, now - ms, now);
        assert_eq!(eg(10_000), "دلوقتي");
        assert_eq!(eg(MIN), "من دقيقة");
        assert_eq!(eg(2 * MIN), "من دقيقتين");
        assert_eq!(eg(5 * MIN), "من 5 دقايق");
        assert_eq!(eg(25 * MIN), "من 25 دقيقة");
        assert_eq!(eg(3 * 60 * MIN), "من 3 ساعات");
        assert_eq!(ago(UiLanguage::Ar, false, now - 5 * MIN, now), "منذ 5 دقائق");
        assert_eq!(ago(UiLanguage::Ar, true, now - 2 * 86_400_000, now), "منذ يومين");
        assert_eq!(ago(UiLanguage::En, false, now - 5 * MIN, now), "5 min ago");
    }
}
