//! What one update phase says and offers: its headline, the detail under
//! it, a hint and the buttons. Settings › Updates, the Today banner and the
//! "what's new" sheet all read it from here, so a failure can't show in one
//! and hide in another. Also the mobile-data question a manual download
//! asks first.
//!
//! Adapted from batna-app main: `crates/app/src/update/stage.rs` (three
//! UI languages, the context updater, no preview-key steps).

use dioxus::prelude::*;
use kabar_core::locale::UiLanguage;
use kabar_protocol::update::Channel;

use super::text::{U, ut};
use super::view::{UPDATE_CSS, failure_text, megabytes, texts, updater, use_update_status, work};
use super::{Failure, Offer, Phase, Status, Updater};
use crate::Ui;

/// A manual download asked for on mobile data: the question is open
/// ([`MeteredConfirm`], once in the shell, asks it).
pub static METERED_ASK: GlobalSignal<bool> = Signal::global(|| false);

#[cfg(test)]
thread_local! {
    /// Tests pretend to be on mobile data.
    pub static FAKE_METERED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn metered() -> bool {
    #[cfg(test)]
    if FAKE_METERED.with(std::cell::Cell::get) {
        return true;
    }
    crate::platform::metered_network()
}

/// A button a phase offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Download,
    Install,
    /// Mac: start the new Kabar.
    Restart,
    /// Android: the "install unknown apps" switch for Kabar.
    OpenPermission,
    /// The channel's release page, to install one build by hand.
    OpenReleases,
    /// Back to alpha, the release channel that always has builds.
    FollowAlpha,
}

impl Act {
    pub fn label(self, phase: &Phase) -> U {
        match self {
            Self::Download if matches!(phase, Phase::Failed { .. }) => U::TryAgain,
            Self::Download => U::Download,
            Self::Install => match phase {
                Phase::Ready(_) if cfg!(target_os = "macos") => U::InstallRestart,
                Phase::Ready(_) => U::InstallNow,
                _ => U::TryAgain,
            },
            Self::Restart => U::RestartNow,
            Self::OpenPermission => U::OpenPermission,
            Self::OpenReleases => U::OpenReleases,
            Self::FollowAlpha => U::FollowAlpha,
        }
    }

    /// Does it, off the UI thread where it blocks. Call it in a handler.
    pub fn run(self, lang: UiLanguage) {
        match self {
            // On mobile data the learner decides first: now, or on Wi-Fi.
            Self::Download if metered() => *METERED_ASK.write() = true,
            Self::Download => work(Updater::download),
            Self::Install => {
                let t = texts(lang);
                work(move |u| u.install(t));
            }
            Self::Restart => {
                #[cfg(target_os = "macos")]
                super::restart();
            }
            Self::OpenPermission =>
            {
                #[cfg(target_os = "android")]
                if let Err(e) = crate::platform::android::open_install_permission() {
                    crate::diag::report!("{e}");
                }
            }
            Self::OpenReleases => {
                let _ = crate::platform::open_url(&super::sheet::releases_url());
            }
            Self::FollowAlpha => work(|u| u.set_channel(Channel::Alpha)),
        }
    }
}

/// Everything a phase shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    pub headline: U,
    /// The offer it is about (a failure may have none).
    pub offer: Option<Offer>,
    pub failed: bool,
    /// The system's own words for a failure (LTR, small).
    pub detail: Option<String>,
    /// One calm line under the headline.
    pub hint: Option<U>,
    /// (done, total) bytes while downloading.
    pub progress: Option<(u64, u64)>,
    /// The first is the main button.
    pub acts: Vec<Act>,
}

/// What `phase` shows; `None` before any check. `android` picks the
/// Android-only hint (a parameter, so tests can ask for it).
pub fn stage(phase: &Phase, android: bool) -> Option<Stage> {
    let offer = match phase {
        Phase::Available(o)
        | Phase::Ready(o)
        | Phase::Installing(o)
        | Phase::NeedsPermission(o)
        | Phase::Installed(o)
        | Phase::Downloading { offer: o, .. } => Some(o.clone()),
        Phase::Failed { offer, .. } => offer.clone(),
        _ => None,
    };
    let (headline, hint, acts) = match phase {
        Phase::Idle => return None,
        Phase::Checking => (U::Checking, None, vec![]),
        Phase::UpToDate => (U::UpToDate, None, vec![]),
        Phase::NotForThisDevice => (U::NotForDevice, None, vec![]),
        Phase::Available(_) => (U::Available, None, vec![Act::Download]),
        Phase::Downloading { .. } => (U::Downloading, None, vec![]),
        Phase::Ready(_) => (U::Ready, None, vec![Act::Install]),
        Phase::Installing(_) => (U::Installing, android.then_some(U::InstallingAndroid), vec![]),
        Phase::NeedsPermission(_) => {
            (U::NeedsPermission, Some(U::PermissionHint), vec![Act::OpenPermission, Act::Install])
        }
        Phase::Installed(_) => (U::Installed, Some(U::NextLaunch), vec![Act::Restart]),
        Phase::Failed { why, offer } => {
            let retry = |a: Act| if offer.is_some() { vec![a] } else { vec![] };
            let acts = match why {
                Failure::Network | Failure::Verify(_) => retry(Act::Download),
                Failure::Install(_) => retry(Act::Install),
                Failure::TooNew | Failure::KeyChanged => vec![Act::OpenReleases],
                Failure::NotPublished(c) if *c != Channel::Alpha => vec![Act::FollowAlpha],
                Failure::NotPublished(_) => vec![],
            };
            (failure_text(why, offer.as_ref()), None, acts)
        }
    };
    let detail = match phase {
        Phase::Failed { why: Failure::Verify(d) | Failure::Install(d), .. } => Some(d.clone()),
        _ => None,
    };
    let progress = match phase {
        Phase::Downloading { offer, done } => Some((*done, offer.asset.size.max(1))),
        _ => None,
    };
    Some(Stage {
        headline,
        offer,
        failed: matches!(phase, Phase::Failed { .. }),
        detail,
        hint,
        progress,
        acts,
    })
}

/// A button's text; a download names its size ("Download · 46.6 MB"), the
/// number kept LTR.
#[component]
pub fn ActLabel(act: Act, phase: Phase, size: Option<u64>) -> Element {
    let ui = use_context::<Ui>();
    let size = size.filter(|_| act == Act::Download).map(|b| megabytes(&ui, b));
    rsx! {
        "{ut(ui.lang(), act.label(&phase))}"
        if let Some(mb) = size {
            " · "
            bdi { dir: "ltr", "{mb}" }
        }
    }
}

/// A stage's hint and the system's detail: the same in Settings and in
/// the sheet.
#[component]
pub fn StageExtras(stage: Stage) -> Element {
    let lang = use_context::<Ui>().lang();
    rsx! {
        if let Some(h) = stage.hint {
            p { class: "muted", "{ut(lang, h)}" }
        }
        if let Some(d) = stage.detail {
            p { class: "muted lp-small", dir: "ltr", bdi { "{d}" } }
        }
    }
}

/// A stage's buttons, the first one primary.
#[component]
pub fn StageButtons(phase: Phase, acts: Vec<Act>, size: Option<u64>) -> Element {
    let lang = use_context::<Ui>().lang();
    if acts.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "row",
            for (i, act) in acts.into_iter().enumerate() {
                button {
                    key: "{i}",
                    class: if i == 0 { "primary" } else { "" },
                    onclick: move |_| act.run(lang),
                    ActLabel { act, phase: phase.clone(), size }
                }
            }
        }
    }
}

/// The mobile-data question, once in the shell.
#[component]
pub fn MeteredConfirm() -> Element {
    let status = use_update_status();
    rsx! { MeteredConfirmView { status } }
}

/// The question for a given status (what [`MeteredConfirm`] follows).
#[component]
pub fn MeteredConfirmView(status: Signal<Option<Status>>) -> Element {
    let ui = use_context::<Ui>();
    let lang = ui.lang();
    if !METERED_ASK() {
        return rsx! {};
    }
    let size = status
        .read()
        .as_ref()
        .and_then(|s| stage(&s.phase, false))
        .and_then(|st| st.offer)
        .map(|o| megabytes(&ui, o.asset.size));
    let close = move || *METERED_ASK.write() = false;
    // The updater is read here, in the component: handlers run outside it.
    let u = updater();
    rsx! {
        style { {UPDATE_CSS} }
        div { class: "um-backdrop", onclick: move |_| close() }
        div { class: "um-dialog", role: "alertdialog", aria_modal: "true", aria_labelledby: "um-title",
            h2 { id: "um-title", "{ut(lang, U::MeteredTitle)}" }
            p {
                "{ut(lang, U::MeteredBody)}"
                if let Some(mb) = size {
                    " ("
                    bdi { dir: "ltr", "{mb}" }
                    ")"
                }
            }
            p { class: "muted", "{ut(lang, U::MeteredNote)}" }
            div { class: "row",
                button {
                    class: "primary",
                    onclick: move |_| {
                        close();
                        if let Some(u) = u {
                            let _ = std::thread::Builder::new().name("kabar-update".into()).spawn(move || u.download());
                        }
                    },
                    "{ut(lang, U::MeteredNow)}"
                }
                button {
                    "data-back": "true",
                    onclick: move |_| {
                        close();
                        if let Some(u) = u {
                            u.download_on_wifi();
                        }
                    },
                    "{ut(lang, U::MeteredWait)}"
                }
            }
        }
    }
}
