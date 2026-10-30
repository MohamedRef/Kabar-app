//! Update policy, offers and observable state; public names remain in update.

use super::*;

/// What the learner chose in Settings › About › Updates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Check and download by itself; install when it's calm (Mac: next
    /// launch; Android: when Kabar is in the background and may update
    /// quietly, otherwise one tap).
    #[default]
    Auto,
    /// Check by itself and say so; download and install on a tap.
    Notify,
    /// Only when "Check now" is pressed.
    Off,
}
impl Mode {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Notify, Self::Off];
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub mode: Mode,
    /// Release builds only (a preview build always follows `preview`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<Channel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check_ms: Option<u64>,
    /// The banner for this offer (`version+build`) was closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed: Option<String>,
    /// An automatic install of this download failed once; wait for a tap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_failed: Option<String>,
    /// Announce a new version with a system notification (on by default).
    #[serde(default = "yes")]
    pub notify: bool,
    /// The last notification sent (`version+build:stage`), so each goes out once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notified: Option<String>,
    /// What a notification needs when no UI is running (Android's
    /// background job): the app writes it whenever it changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<NoticeContext>,
    /// The build (`version+build`) that ran last, to notice an update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<String>,
    /// Set at the first launch after an update to the build that ran
    /// before: "What's new" opens once for what came since, then clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whats_new_from: Option<String>,
}
pub(super) fn yes() -> bool {
    true
}
impl Default for Prefs {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            channel: None,
            last_check_ms: None,
            dismissed: None,
            auto_failed: None,
            notify: true,
            notified: None,
            notice: None,
            last_run: None,
            whats_new_from: None,
        }
    }
}
/// The learner's language and quiet hours, copied from the app's settings
/// and coach profile into the device-local update prefs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeContext {
    pub lang: UiLanguage,
    /// Kak Nur's quiet hours, if a coach profile exists: no notification
    /// goes out inside them (the next check after they end sends it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet: Option<QuietHours>,
    /// The device's offset from UTC, for the quiet hours where the
    /// platform doesn't pass the local time in.
    #[serde(default)]
    pub utc_offset_min: i16,
}
/// The "new version" notification, as `app.kabar.UpdateNotice.show` and the
/// Mac read it (JSON).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Notice {
    /// `version+build:stage`: remembered once sent.
    pub key: String,
    pub title: String,
    pub body: String,
    /// What the notification's button does in the app: `download` or
    /// `install`; empty for no button.
    pub action: &'static str,
    pub button: String,
    /// The Android notification channel's name, in the learner's language.
    pub channel: String,
}
/// Which step a notification announces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NoticeStage {
    /// Out, not downloaded yet (Tell me first, or waiting for Wi-Fi).
    Available,
    /// Downloaded and verified; one tap installs it.
    Ready,
    /// Mac: swapped in; it starts at the next launch.
    Installed,
}
impl NoticeStage {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Ready => "ready",
            Self::Installed => "installed",
        }
    }
}
/// What's new in `lang`, falling back to the other languages the feed has.
pub(crate) fn notes_in(lang: UiLanguage, n: &UpdateNotes) -> String {
    let pick = |a: &str, b: &str, c: &str| {
        [a, b, c].into_iter().find(|s| !s.trim().is_empty()).unwrap_or("").trim().to_owned()
    };
    match lang {
        UiLanguage::ArEg => pick(&n.ar_eg, &n.ar, &n.en),
        UiLanguage::Ar => pick(&n.ar, &n.ar_eg, &n.en),
        UiLanguage::En => pick(&n.en, &n.ar_eg, &n.ar),
    }
}
/// The notification due for `s`, if any: one per offer and step, never for
/// an offer whose banner was closed, never inside quiet hours. `local` is the
/// device's wall clock when the platform knows it (Android's job passes it).
pub fn notice(s: &Status, local: Option<LocalTime>, now_ms: u64) -> Option<Notice> {
    use text::{U, more_changes, ut};
    if s.off.is_some() || s.prefs.mode == Mode::Off || !s.prefs.notify {
        return None;
    }
    let (offer, stage) = match &s.phase {
        Phase::Available(o) => (o, NoticeStage::Available),
        Phase::Ready(o) | Phase::NeedsPermission(o) => (o, NoticeStage::Ready),
        Phase::Installed(o) => (o, NoticeStage::Installed),
        _ => return None,
    };
    if s.prefs.dismissed.as_deref() == Some(offer.key().as_str()) {
        return None;
    }
    let key = format!("{}:{}", offer.key(), stage.as_str());
    if s.prefs.notified.as_deref() == Some(key.as_str()) {
        return None;
    }
    let ctx = s.prefs.notice.unwrap_or_default();
    let local = local.unwrap_or_else(|| LocalTime::from_unix_ms(now_ms, ctx.utc_offset_min));
    if ctx.quiet.is_some_and(|q| q.contains(local)) {
        return None;
    }
    let lang = ctx.lang;
    let (headline, hint, action, button) = match stage {
        NoticeStage::Available => (U::NoticeAvailable, U::NoticeTapAvailable, "download", U::Download),
        NoticeStage::Ready => (U::NoticeReady, U::NoticeTapReady, "install", U::InstallNow),
        NoticeStage::Installed => (U::NoticeInstalled, U::NoticeTapInstalled, "", U::RestartNow),
    };
    // A Mac install starts with "Install and restart"; Android's button is
    // "Install now" (the system may still ask to confirm).
    let button = if action.is_empty() { String::new() } else { ut(lang, button).to_owned() };
    let mut whats_new = if offer.changes.is_empty() {
        notes_in(lang, &offer.notes)
    } else {
        changelog::notice_lines(lang, &offer.changes, |n| more_changes(lang, n))
    };
    if whats_new.chars().count() > 300 {
        whats_new = whats_new.chars().take(299).collect::<String>() + "…";
    }
    let body = if whats_new.is_empty() {
        ut(lang, hint).to_owned()
    } else if offer.changes.is_empty() {
        format!("{}: {whats_new}\n{}", ut(lang, U::WhatsNew), ut(lang, hint))
    } else {
        format!("{whats_new}\n{}", ut(lang, hint))
    };
    Some(Notice {
        key,
        title: format!("{} · {}", ut(lang, headline), offer.label()),
        body,
        action,
        button,
        channel: ut(lang, U::NoticeChannel).to_owned(),
    })
}
/// Why this build doesn't update itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Off {
    /// A development build (`dx serve`, `cargo run`).
    DevBuild,
    /// No update key yet: `just update-keygen` hasn't run.
    NoKey,
    /// No builds are published for this system (Linux, x86 Android).
    NoPlatform,
    /// The Mac app runs from a build folder, not from a Kabar.app.
    NotInBundle,
}
/// A newer build, from a verified manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub version: String,
    pub build: u64,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub notes: UpdateNotes,
    pub asset: UpdateAsset,
    /// What it brings: the feed's builds after the installed one, newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<BuildChanges>,
}
impl Offer {
    pub(super) fn from_manifest(m: &UpdateManifest, asset: UpdateAsset, me: &Installed) -> Self {
        // A content-only feed carries an older app; offer that build.
        let build = m.build_of(&asset);
        let to = changelog::order(&m.version, build);
        let changes = match to {
            Some(to) => changelog::between(&m.history, Some((me.version, me.build)), to)
                .into_iter()
                .cloned()
                .collect(),
            None => Vec::new(),
        };
        Self {
            version: m.version.clone(),
            build,
            commit: m.commit.clone(),
            notes: m.notes.clone(),
            asset,
            changes,
        }
    }

    /// `1.0.0-alpha.2`, or `0.1.0 (build 141)` for previews.
    pub fn label(&self) -> String {
        if self.build > 0 { format!("{} ({})", self.version, self.build) } else { self.version.clone() }
    }

    pub(super) fn key(&self) -> String {
        format!("{}+{}", self.version, self.build)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// No connection, or GitHub said no; try later.
    Network,
    /// The feed or the download didn't verify. Never installed.
    Verify(String),
    /// The system refused the install.
    Install(String),
    /// The feed needs a newer Kabar: install by hand once.
    TooNew,
    /// Preview only: CI's signing key was renewed; install the newest
    /// preview by hand once.
    KeyChanged,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Checking,
    UpToDate,
    /// Newer, but not published for this device yet.
    NotForThisDevice,
    Available(Offer),
    Downloading {
        offer: Offer,
        done: u64,
    },
    /// Downloaded and verified.
    Ready(Offer),
    Installing(Offer),
    /// Android: "Install unknown apps" is off for Kabar.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    NeedsPermission(Offer),
    /// Mac: the new Kabar.app is in place; it starts at the next launch.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Installed(Offer),
    Failed {
        offer: Option<Offer>,
        why: Failure,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub off: Option<Off>,
    pub phase: Phase,
    pub prefs: Prefs,
    /// The channel being followed right now.
    pub channel: Channel,
    /// A preview build: the channel is fixed.
    pub preview: bool,
    /// This build: version and build number.
    pub version: String,
    pub build: u64,
    /// The feed build whose content bundle is in use, when newer than this
    /// build (a content-only update), else 0.
    pub content_build: u64,
    /// When that bundle arrived (ms since the epoch), if known.
    pub content_since_ms: Option<u64>,
    /// The feed build of a newer content bundle that is downloaded and used
    /// from the next launch.
    pub content_waiting: Option<u64>,
    /// What the last check found for lessons and look.
    pub content_check: Option<content::Refresh>,
    /// How many offline audio packs the last check by hand refreshed.
    pub packs_refreshed: usize,
    /// A check by hand is running (the feed, then the audio packs).
    pub checking_all: bool,
    /// Every build's changes seen so far, newest first (changelog.rs).
    pub history: Vec<BuildChanges>,
}
impl Status {
    /// This build's place among the history's builds.
    pub fn current(&self) -> Option<(Version, u64)> {
        changelog::order(&self.version, self.shown_build())
    }

    /// The build number the learner is on: this build, or the content-only
    /// update on top of it.
    pub fn shown_build(&self) -> u64 {
        self.build.max(self.content_build)
    }

    /// "What's new" to open by itself after an update: the builds after the
    /// one that ran before, up to this one. Empty once seen.
    pub fn whats_new(&self) -> Vec<BuildChanges> {
        let (Some(from), Some(to)) = (self.prefs.whats_new_from.as_deref(), self.current()) else {
            return Vec::new();
        };
        let from = from.split_once('+').and_then(|(v, b)| changelog::order(v, b.parse().ok()?));
        if from.is_none() {
            return Vec::new();
        }
        changelog::between(&self.history, from, to).into_iter().cloned().collect()
    }

    /// The offer the banner should mention, unless closed for it.
    pub fn banner(&self) -> Option<&Offer> {
        let offer = match &self.phase {
            Phase::Available(o) if self.prefs.mode == Mode::Notify => o,
            Phase::Ready(o) | Phase::NeedsPermission(o) | Phase::Installed(o) => o,
            _ => return None,
        };
        (self.prefs.dismissed.as_deref() != Some(offer.key().as_str())).then_some(offer)
    }
}
/// What this binary is, from the build (CI sets the env vars for previews).
#[derive(Clone, Debug)]
pub(super) struct BuildInfo {
    pub(super) version: Version,
    pub(super) build: u64,
    pub(super) preview: bool,
    pub(super) key: Option<VerifyingKey>,
}
impl BuildInfo {
    pub(super) fn from_env() -> Self {
        let preview = option_env!("KABAR_UPDATE_CHANNEL") == Some("preview");
        let key_text = dev_override("KABAR_UPDATE_KEY").or_else(|| {
            if preview {
                option_env!("KABAR_UPDATE_KEY").map(str::to_owned)
            } else {
                Some(RELEASE_KEY.to_owned())
            }
        });
        let version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version {
            major: 0,
            minor: 0,
            patch: 0,
            stage: kabar_core::update::Stage::Alpha(0),
        });
        let build = dev_override("KABAR_BUILD")
            .as_deref()
            .or(option_env!("KABAR_BUILD"))
            .and_then(|b| b.trim().parse().ok())
            .unwrap_or(0);
        Self { version, build, preview, key: key_text.and_then(|k| parse_public_key(&k).ok()) }
    }

    /// A preview build follows `preview`; a release build its chosen
    /// release channel (alpha by default), never `preview`.
    pub(super) fn channel(&self, chosen: Option<Channel>) -> Channel {
        if self.preview {
            Channel::Preview
        } else {
            chosen.filter(|c| *c != Channel::Preview).unwrap_or(Channel::Alpha)
        }
    }

    pub(super) fn installed(&self, chosen: Option<Channel>) -> Installed {
        Installed { version: self.version, build: self.build, channel: self.channel(chosen) }
    }
}
