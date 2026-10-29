//! App updates over the air (`docs/RELEASE.md` §5): Kabar checks its update
//! feed on GitHub Releases, verifies the feed's Ed25519 signature and each
//! download's SHA-256, then installs it: PackageInstaller on Android, a
//! swapped Kabar.app on the Mac. Offline packs ride along: a check also
//! refreshes installed packs that have a newer version.
//!
//! Device-local, never synced:
//!
//! ```text
//! <data>/updates/prefs.json    automatic / notify / off, channel, last check,
//!                              the last "new version" notification sent
//! <data>/updates/staged.json   the verified download waiting to install
//! <data>/updates/history.json  what each build changed (changelog.rs)
//! <data>/updates/<file>        that download (one at a time)
//! <data>/content/…             the content bundle in use (content.rs)
//! ```
//!
//! The work runs on its own thread; the UI follows [`Status`] through a
//! `tokio::sync::watch` channel.
//!
//! A new version is also announced with a system notification ([`Notice`]):
//! by the running app's own checks on the Mac and Android, and on Android by
//! a periodic background job (`app.kabar.UpdateCheckJob`) that runs
//! [`background_check`] even while Kabar is closed.

pub mod bundle;
pub mod changelog;
pub mod content;
pub mod sheet;
mod text;
pub mod view;

use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kabar_core::locale::UiLanguage;
use kabar_core::update::{
    Decision, Installed, UpdateError, VerifyingKey, Version, check_download, decide, parse_public_key,
    this_platform, verify_manifest,
};
use kabar_protocol::coach::{LocalTime, QuietHours};
use kabar_protocol::update::{
    BuildChanges, Channel, MANIFEST_FILE, SIGNATURE_FILE, UpdateAsset, UpdateManifest, UpdateNotes,
    UpdatePlatform,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

/// Where the feeds live: one GitHub release per channel of the public repo
/// (free; no token needed to download).
pub(super) const FEEDS_URL: &str = "https://github.com/MohamedRef/Kabar-app/releases/download";
/// The release public key (`just update-keygen` fills it once).
const RELEASE_KEY: &str = include_str!("../../update-key.pub");
/// How often a running Kabar looks for an update by itself.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
/// First automatic check after launch: late enough not to slow the start.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);
/// Android's background job runs about every four hours
/// (`UpdateCheckJob.PERIOD_MS`); it looks at the feed when the last look,
/// its own or the app's, is at least this old.
const BACKGROUND_EVERY: Duration = Duration::from_secs(3 * 3600);
/// `update.json` and its signature are small; anything bigger is refused.
const MAX_FEED_BYTES: u64 = 256 * 1024;
/// Largest build accepted (the APK is about 60 MB, Kabar.app less).
const MAX_BUILD_BYTES: u64 = 400 * 1024 * 1024;

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

fn yes() -> bool {
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
enum NoticeStage {
    /// Out, not downloaded yet (Tell me first, or waiting for Wi-Fi).
    Available,
    /// Downloaded and verified; one tap installs it.
    Ready,
    /// Mac: swapped in; it starts at the next launch.
    Installed,
}

impl NoticeStage {
    fn as_str(self) -> &'static str {
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
    fn from_manifest(m: &UpdateManifest, asset: UpdateAsset, me: &Installed) -> Self {
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

    fn key(&self) -> String {
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
struct BuildInfo {
    version: Version,
    build: u64,
    preview: bool,
    key: Option<VerifyingKey>,
}

impl BuildInfo {
    fn from_env() -> Self {
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
    fn channel(&self, chosen: Option<Channel>) -> Channel {
        if self.preview {
            Channel::Preview
        } else {
            chosen.filter(|c| *c != Channel::Preview).unwrap_or(Channel::Alpha)
        }
    }

    fn installed(&self, chosen: Option<Channel>) -> Installed {
        Installed { version: self.version, build: self.build, channel: self.channel(chosen) }
    }
}

/// The commit this build was made from (CI and `just release` set it).
pub fn commit() -> Option<&'static str> {
    option_env!("KABAR_COMMIT").filter(|c| !c.is_empty())
}

/// Run-time overrides for trying updates from a development build against a
/// local feed (`docs/RUN.md` §17). Release builds ignore them.
fn dev_override(name: &str) -> Option<String> {
    if cfg!(debug_assertions) { std::env::var(name).ok().filter(|v| !v.trim().is_empty()) } else { None }
}

pub struct Updater {
    dir: PathBuf,
    info: BuildInfo,
    platform: Option<UpdatePlatform>,
    /// A local feed folder instead of GitHub (development only).
    feed_override: Option<String>,
    /// Pause before re-reading a feed whose signature didn't match.
    retry_after: Duration,
    tx: watch::Sender<Status>,
    /// One check, download or install at a time.
    job: Mutex<()>,
}

/// The one updater in this process, whichever opens it first: the app's
/// start ([`init`]) or Android's background job ([`job_updater`]), which can
/// run before any UI. Two would share `updates/` with separate locks and
/// prefs: racing on one `.part` download and overwriting each other's
/// `prefs.json` (a lost "notified" mark sends the notification twice).
static UPDATER: OnceLock<Updater> = OnceLock::new();

/// [`UPDATER`], opened under `data_dir` if nothing has opened it yet.
fn shared<'a>(cell: &'a OnceLock<Updater>, data_dir: &Path) -> &'a Updater {
    cell.get_or_init(|| {
        let feed_override = dev_override("KABAR_UPDATE_URL");
        let platform = dev_override("KABAR_UPDATE_PLATFORM")
            .and_then(|p| UpdatePlatform::parse(&p))
            .or_else(this_platform);
        let off = if cfg!(debug_assertions) && feed_override.is_none() {
            Some(Off::DevBuild)
        } else if cfg!(target_os = "macos") && bundle::running_bundle().is_none() && feed_override.is_none() {
            Some(Off::NotInBundle)
        } else {
            None
        };
        Updater::new(data_dir.join("updates"), BuildInfo::from_env(), platform, feed_override, off)
    })
}

/// Opens the updater under the app's data directory (or takes the one the
/// background job already opened) and starts the background checks.
/// Called once at start.
pub fn init(data_dir: &Path) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    let updater = shared(&UPDATER, data_dir);
    STARTED.call_once(|| {
        crate::platform::update_notice::schedule(updater.background_checks());
        if updater.status().off.is_some() {
            return;
        }
        let _ = std::thread::Builder::new().name("kabar-update".into()).spawn(|| {
            std::thread::sleep(FIRST_CHECK_AFTER);
            loop {
                if let Some(u) = UPDATER.get() {
                    u.tick();
                }
                std::thread::sleep(Duration::from_secs(30 * 60));
            }
        });
    });
}

pub fn get() -> Option<&'static Updater> {
    UPDATER.get()
}

/// The updater Android's background job (`UpdateCheckJob`) works with, in a
/// process that may have no UI yet: the same one the app uses.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn job_updater(data_dir: &Path) -> &'static Updater {
    shared(&UPDATER, data_dir)
}

/// Android's background job: checks when due and returns the notification
/// to post, if any. Not marked as sent yet: the job calls
/// [`background_shown`] once Android has shown it.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn background_check(
    data_dir: &Path,
    metered: bool,
    can_notify: bool,
    local: LocalTime,
) -> Option<Notice> {
    job_updater(data_dir).background(metered, can_notify, local)
}

/// The job's notification `key` is up: it doesn't go out again.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn background_shown(data_dir: &Path, key: &str) {
    job_updater(data_dir).shown(key);
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn is_newer(me: &Installed, offer: &Offer) -> bool {
    Version::parse(&offer.version).is_some_and(|v| (v, offer.build) > (me.version, me.build))
}

impl Updater {
    fn new(
        dir: PathBuf,
        info: BuildInfo,
        platform: Option<UpdatePlatform>,
        feed_override: Option<String>,
        off: Option<Off>,
    ) -> Self {
        let prefs = load_prefs(&dir);
        let off = off.or(if platform.is_none() {
            Some(Off::NoPlatform)
        } else if info.key.is_none() {
            Some(Off::NoKey)
        } else {
            None
        });
        let mut prefs = prefs;
        let in_use = dir.parent().and_then(|d| content::in_use(d, &info));
        let content_build = in_use.as_ref().map_or(0, |c| c.build);
        let content_since_ms = in_use.and_then(|c| c.fetched_ms);
        let this = format!("{}+{}", info.version, info.build.max(content_build));
        if off.is_none() && prefs.last_run.as_deref() != Some(this.as_str()) {
            // Updated (not a first install): say what's new once.
            if let Some(prev) = prefs.last_run.take() {
                prefs.whats_new_from = Some(prev);
            }
            prefs.last_run = Some(this);
            if let Err(e) = save_prefs(&dir, &prefs) {
                crate::diag::report!("update prefs: {e}");
            }
        }
        let history = changelog::load(&dir);
        let installed = info.installed(prefs.channel);
        let mut phase = Phase::Idle;
        if off.is_none()
            && let Some(offer) = load_staged(&dir)
            && is_newer(&installed, &offer)
            && dir.join(&offer.asset.file).metadata().is_ok_and(|m| m.len() == offer.asset.size)
        {
            phase = Phase::Ready(offer);
        }
        let status = Status {
            off,
            phase,
            channel: installed.channel,
            preview: info.preview,
            prefs,
            version: info.version.to_string(),
            build: info.build,
            content_build,
            content_since_ms,
            content_waiting: None,
            content_check: None,
            packs_refreshed: 0,
            checking_all: false,
            history,
        };
        let (tx, _) = watch::channel(status);
        Self {
            dir,
            info,
            platform,
            feed_override,
            retry_after: Duration::from_secs(20),
            tx,
            job: Mutex::new(()),
        }
    }

    /// What's installed, on the channel chosen now.
    pub(super) fn installed(&self) -> Installed {
        self.info.installed(self.tx.borrow().prefs.channel)
    }

    /// The feed folder: the local override in development, else the
    /// channel's GitHub release.
    fn feed_url(&self) -> String {
        match &self.feed_override {
            Some(url) => url.trim_end_matches('/').to_owned(),
            None => format!("{FEEDS_URL}/{}", self.installed().channel.release_tag()),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<Status> {
        self.tx.subscribe()
    }

    pub fn status(&self) -> Status {
        self.tx.borrow().clone()
    }

    fn set_phase(&self, phase: Phase) {
        self.tx.send_modify(|s| s.phase = phase);
    }

    fn update_prefs(&self, f: impl FnOnce(&mut Prefs)) {
        self.tx.send_modify(|s| {
            f(&mut s.prefs);
            s.channel = self.info.channel(s.prefs.channel);
            if let Err(e) = save_prefs(&self.dir, &s.prefs) {
                crate::diag::report!("update prefs: {e}");
            }
        });
    }

    /// The background loop's step: check when due, then refresh outdated
    /// offline packs, then announce what it found.
    fn tick(&'static self) {
        if self.step(CHECK_EVERY, crate::platform::metered_network()) {
            crate::packs::update_installed(false);
        }
        self.send_notice(None, crate::platform::update_notice::post);
    }

    /// Checks when the last check is `every` old (true if it did), or starts
    /// the download an earlier check left waiting for Wi-Fi.
    fn step(&'static self, every: Duration, metered: bool) -> bool {
        let s = self.status();
        if s.off.is_some() || s.prefs.mode == Mode::Off {
            return false;
        }
        let due =
            s.prefs.last_check_ms.is_none_or(|t| now_ms().saturating_sub(t) >= every.as_millis() as u64);
        // Automatic downloads wait for Wi-Fi (a ~45 MB APK on mobile data,
        // up to every merge on the preview channel); the offer still shows.
        let auto = s.prefs.mode == Mode::Auto && !metered;
        if due {
            // Content bundles (~8 MB) wait for Wi-Fi too.
            self.check_with(auto, !metered);
        } else if auto && let Phase::Available(_) = s.phase {
            self.download();
        }
        due
    }

    /// Sends the notification due now through `post` (false: not shown, try
    /// again at the next check) and remembers it once shown.
    fn send_notice(&self, local: Option<LocalTime>, post: impl FnOnce(&Notice) -> bool) -> Option<Notice> {
        let n = notice(&self.status(), local, now_ms())?;
        if !post(&n) {
            return None;
        }
        self.update_prefs(|p| p.notified = Some(n.key.clone()));
        Some(n)
    }

    /// Android's background job: the same step as the app's loop, then the
    /// notification to post, if `can_notify` (notifications allowed, Kabar
    /// not in front). Not marked as sent: one Android doesn't show is tried
    /// again at the next run ([`Self::shown`] marks it).
    fn background(&'static self, metered: bool, can_notify: bool, local: LocalTime) -> Option<Notice> {
        self.step(BACKGROUND_EVERY, metered);
        if !can_notify {
            return None;
        }
        notice(&self.status(), Some(local), now_ms())
    }

    /// The notification `key` was shown.
    fn shown(&self, key: &str) {
        if self.status().prefs.notified.as_deref() != Some(key) {
            self.update_prefs(|p| p.notified = Some(key.to_owned()));
        }
    }

    /// Whether Android's background job should run: updates work here and
    /// aren't switched off.
    pub fn background_checks(&self) -> bool {
        let s = self.status();
        s.off.is_none() && s.prefs.mode != Mode::Off
    }

    /// Keeps the language and quiet hours the background job words and
    /// times its notification by.
    pub fn set_notice_context(&self, ctx: NoticeContext) {
        if self.status().prefs.notice != Some(ctx) {
            self.update_prefs(|p| p.notice = Some(ctx));
        }
    }

    pub fn set_notify(&self, on: bool) {
        self.update_prefs(|p| p.notify = on);
        if !on {
            crate::platform::update_notice::clear();
        }
    }

    /// Looks at the feed; with `download`, fetches a newer build right away.
    pub fn check(&'static self, download: bool) {
        self.check_with(download, true);
    }

    /// "Check for updates" by hand: the app, lessons and look, and the
    /// offline audio packs already on this device, in one go.
    pub fn check_everything(&'static self) {
        let mut started = false;
        self.tx.send_if_modified(|s| {
            started = !s.checking_all;
            if started {
                s.checking_all = true;
                s.packs_refreshed = 0;
            }
            started
        });
        if !started {
            return;
        }
        self.check(false);
        let reached = !matches!(self.status().phase, Phase::Failed { offer: None, .. });
        let n = if self.status().off.is_none() && reached { crate::packs::update_installed(true) } else { 0 };
        self.tx.send_modify(|s| {
            s.packs_refreshed = n;
            s.checking_all = false;
        });
    }

    /// [`Self::check`]; with `content`, a newer content bundle is fetched too.
    fn check_with(&'static self, download: bool, content: bool) {
        let Ok(_job) = self.job.try_lock() else { return };
        if self.status().off.is_some() {
            return;
        }
        let keep = match self.status().phase {
            // A finished download for the same offer stays; it's re-checked anyway.
            Phase::Ready(o) | Phase::Installed(o) | Phase::NeedsPermission(o) => Some(o),
            _ => None,
        };
        self.set_phase(Phase::Checking);
        let result = self.fetch_offer(content);
        self.update_prefs(|p| p.last_check_ms = Some(now_ms()));
        match result {
            Ok(Some(offer)) => {
                if let Some(k) = keep.filter(|k| k.asset == offer.asset) {
                    self.set_phase(match self.status().phase {
                        Phase::Checking => Phase::Ready(k),
                        other => other,
                    });
                    return;
                }
                self.set_phase(Phase::Available(offer.clone()));
                if download {
                    self.download_locked(offer);
                }
            }
            Ok(None) => {}
            Err(why) => self.set_phase(Phase::Failed { offer: None, why }),
        }
    }

    /// `Ok(None)` after setting UpToDate / NotForThisDevice.
    fn fetch_offer(&self, content: bool) -> Result<Option<Offer>, Failure> {
        let key = self.info.key.ok_or(Failure::Verify("no key".into()))?;
        let base = self.feed_url();
        let fetch = || -> Result<_, Failure> {
            let json = get_small(&format!("{base}/{MANIFEST_FILE}"))?;
            let sig = get_small(&format!("{base}/{SIGNATURE_FILE}"))?;
            let sig = String::from_utf8(sig).map_err(|_| Failure::Verify("signature".into()))?;
            Ok(verify_manifest(&json, &sig, &key))
        };
        // A feed caught mid-upload pairs a new signature with the old
        // manifest: look once more before calling it a failure.
        let m = match fetch()? {
            Err(UpdateError::NotSigned) => {
                std::thread::sleep(self.retry_after);
                fetch()?
            }
            first => first,
        };
        let m = m.map_err(|e| match e {
            // A preview whose CI key was renewed can't verify any newer feed.
            UpdateError::NotSigned if self.info.preview => Failure::KeyChanged,
            other => update_failure(other),
        })?;
        self.remember_history(&m.history);
        if content && let Some(data) = self.dir.parent() {
            let found = content::refresh(data, &self.installed(), &self.info, &m, &self.feed_url());
            self.tx.send_modify(|s| {
                s.content_check = Some(found);
                if let content::Refresh::Taken(build) = found {
                    s.content_waiting = Some(build);
                }
            });
        }
        let platform = self.platform.ok_or(Failure::Verify("no platform".into()))?;
        match decide(&self.installed(), &m, platform).map_err(update_failure)? {
            Decision::UpToDate => {
                self.set_phase(Phase::UpToDate);
                self.clear_downloads(None);
                Ok(None)
            }
            Decision::NotForThisPlatform => {
                self.set_phase(Phase::NotForThisDevice);
                Ok(None)
            }
            Decision::Available(asset) => {
                if asset.size > MAX_BUILD_BYTES {
                    return Err(Failure::Verify("build too large".into()));
                }
                Ok(Some(Offer::from_manifest(&m, asset, &self.installed())))
            }
        }
    }

    /// Keeps a verified feed's history, and lets a pending "What's new" go
    /// when the feed has nothing to say about this build.
    fn remember_history(&self, fresh: &[BuildChanges]) {
        let known = self.status().history;
        if let Some(all) = changelog::merge(&known, fresh) {
            if let Err(e) = write_json(&self.dir, changelog::FILE, &all) {
                crate::diag::report!("update history: {e}");
            }
            self.tx.send_modify(|s| s.history = all);
        }
        let s = self.status();
        if s.prefs.whats_new_from.is_some() && s.whats_new().is_empty() {
            self.update_prefs(|p| p.whats_new_from = None);
        }
    }

    /// "What's new" was seen (closed).
    pub fn close_whats_new(&self) {
        if self.status().prefs.whats_new_from.is_some() {
            self.update_prefs(|p| p.whats_new_from = None);
        }
    }

    /// Downloads the offered build (a tap in Notify mode).
    pub fn download(&'static self) {
        let Ok(_job) = self.job.try_lock() else { return };
        if let Phase::Available(offer) | Phase::Failed { offer: Some(offer), .. } = self.status().phase {
            self.download_locked(offer);
        }
    }

    fn download_locked(&self, offer: Offer) {
        let base = self.feed_url();
        let _ = fs::create_dir_all(&self.dir);
        self.clear_downloads(Some(&offer.asset.file));
        let path = self.dir.join(&offer.asset.file);
        self.set_phase(Phase::Downloading { offer: offer.clone(), done: 0 });
        let url = format!("{base}/{}", offer.asset.file);
        let mut result = self.fetch_build(&url, &offer, &path);
        // A connection that dropped mid-file is picked up again where it
        // stopped, a few times, before giving up until the next try.
        for wait in DOWNLOAD_RETRIES {
            match &result {
                Err(Fetch { why: Failure::Network, progressed: true }) => {
                    std::thread::sleep(self.retry_after * wait);
                    result = self.fetch_build(&url, &offer, &path);
                }
                _ => break,
            }
        }
        match result.map_err(|f| f.why) {
            Ok(()) => {
                if let Err(e) = save_staged(&self.dir, &offer) {
                    crate::diag::report!("update staged.json: {e}");
                }
                self.set_phase(Phase::Ready(offer));
            }
            Err(why) => {
                let _ = fs::remove_file(&path);
                self.set_phase(Phase::Failed { offer: Some(offer), why });
            }
        }
    }

    /// Streams the build to `<file>.part` while hashing it, checks size and
    /// SHA-256 against the signed manifest, then renames it into place. A
    /// part left by an interrupted try of the same build is continued with
    /// an HTTP Range request instead of starting over; the hash still covers
    /// every byte, old and new.
    fn fetch_build(&self, url: &str, offer: &Offer, path: &Path) -> Result<(), Fetch> {
        let (part, meta) = part_paths(path);
        let want = PartMeta { size: offer.asset.size, sha256: offer.asset.sha256.clone() };
        let on_disk = fs::metadata(&part).map_or(0, |m| m.len());
        let mut have = match read_part_meta(&meta) {
            Some(m) if m == want && on_disk < want.size => on_disk,
            _ => 0,
        };
        if have == 0 {
            let _ = fs::remove_file(&part);
            let json = serde_json::to_vec(&want).unwrap_or_default();
            fs::write(&meta, json).map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
        }
        let mut req = agent(Duration::from_secs(30 * 60)).get(url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let mut res = req.call().map_err(|_| Fetch::stopped(Failure::Network))?;
        match res.status().as_u16() {
            206 if have > 0 && range_start(&res) == Some(have) => {}
            // The server sent the whole file (or ignored the range): start over.
            200 => have = 0,
            // The range no longer fits this file: drop the part, next try starts fresh.
            416 => {
                let _ = fs::remove_file(&part);
                return Err(Fetch::stopped(Failure::Network));
            }
            _ => return Err(Fetch::stopped(Failure::Network)),
        }
        let mut hash = Sha256::new();
        let mut file = if have > 0 {
            let mut old =
                fs::File::open(&part).map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
            let read = std::io::copy(&mut (&mut old).take(have), &mut hash)
                .map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
            if read != have {
                let _ = fs::remove_file(&part);
                return Err(Fetch::stopped(Failure::Network));
            }
            fs::OpenOptions::new().append(true).open(&part)
        } else {
            fs::File::create(&part)
        }
        .map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
        let mut reader = res.body_mut().as_reader().take(offer.asset.size - have + 1);
        let (mut done, mut shown) = (have, have);
        self.set_phase(Phase::Downloading { offer: offer.clone(), done });
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let progressed = done > have;
            let n = reader.read(&mut buf).map_err(|_| Fetch { why: Failure::Network, progressed })?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
            done += n as u64;
            if done - shown > offer.asset.size / 100 {
                shown = done;
                self.set_phase(Phase::Downloading { offer: offer.clone(), done });
            }
        }
        file.sync_all().map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
        drop(file);
        if done < offer.asset.size {
            // The body ended early: keep what came, the next try continues it.
            return Err(Fetch { why: Failure::Network, progressed: done > have });
        }
        let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if let Err(e) = check_download(&offer.asset, done, &hex) {
            let _ = fs::remove_file(&part);
            let _ = fs::remove_file(&meta);
            if have > 0 {
                // The kept part was likely damaged: fetch it whole once more
                // (a retry from zero) before calling the build bad.
                return Err(Fetch { why: Failure::Network, progressed: true });
            }
            return Err(Fetch::stopped(update_failure(e)));
        }
        fs::rename(&part, path).map_err(|e| Fetch::stopped(Failure::Install(e.to_string())))?;
        let _ = fs::remove_file(&meta);
        Ok(())
    }

    /// Re-hashes the staged file: whatever sat on disk since the download
    /// is checked again before it's installed.
    fn verified_staged(&self, offer: &Offer) -> Result<PathBuf, Failure> {
        let path = self.dir.join(&offer.asset.file);
        let mut file = fs::File::open(&path).map_err(|_| Failure::Verify("download missing".into()))?;
        let mut hash = Sha256::new();
        let size = std::io::copy(&mut file, &mut hash).map_err(|e| Failure::Verify(e.to_string()))?;
        let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        check_download(&offer.asset, size, &hex).map_err(update_failure)?;
        Ok(path)
    }

    /// Installs the ready download (a tap, or calm moment in Auto mode).
    pub fn install(&'static self, texts: InstallTexts) {
        let Ok(_job) = self.job.try_lock() else { return };
        let offer = match self.status().phase {
            Phase::Ready(o)
            | Phase::NeedsPermission(o)
            | Phase::Failed { offer: Some(o), why: Failure::Install(_) } => o,
            _ => return,
        };
        let path = match self.verified_staged(&offer) {
            Ok(p) => p,
            Err(why) => {
                self.clear_downloads(None);
                self.set_phase(Phase::Failed { offer: Some(offer), why });
                return;
            }
        };
        self.set_phase(Phase::Installing(offer.clone()));
        // "Kabar was updated" says what's new when the feed says it.
        let mut texts = texts;
        let whats_new = notes_in(texts.lang, &offer.notes);
        if !whats_new.is_empty() {
            texts.done_body = whats_new;
        }
        let next = install_build(&path, &offer, &texts);
        if let Phase::Installed(_) = next {
            self.clear_downloads(None);
        }
        self.set_phase(next);
    }

    pub fn set_mode(&self, mode: Mode) {
        self.update_prefs(|p| p.mode = mode);
        crate::platform::update_notice::schedule(self.background_checks());
    }

    /// Switches release channel and looks again.
    pub fn set_channel(&'static self, channel: Channel) {
        if self.status().preview || channel == Channel::Preview {
            return;
        }
        self.update_prefs(|p| p.channel = Some(channel));
        self.clear_downloads(None);
        self.set_phase(Phase::Idle);
        let auto = self.status().prefs.mode == Mode::Auto;
        std::thread::spawn(move || self.check(auto));
    }

    pub fn dismiss(&self) {
        let key = self.status().banner().map(Offer::key);
        self.update_prefs(|p| p.dismissed = key);
        crate::platform::update_notice::clear();
    }

    /// Android: polls PackageInstaller after an install started.
    #[cfg(target_os = "android")]
    pub fn poll_install(&self) {
        use crate::platform::android::{InstallState, install_state};
        let Phase::Installing(offer) = self.status().phase else { return };
        match install_state() {
            // A success ends this process: the new Kabar starts in its place.
            Ok(InstallState::Working | InstallState::WaitingUser) => {}
            Ok(InstallState::Idle | InstallState::Cancelled) => self.set_phase(Phase::Ready(offer)),
            Ok(InstallState::Failed(why)) | Err(why) => {
                self.update_prefs(|p| p.auto_failed = Some(offer.key()));
                self.set_phase(Phase::Failed { offer: Some(offer), why: Failure::Install(why) });
            }
        }
    }

    /// Removes every download except `keep`, and the staged record unless kept.
    fn clear_downloads(&self, keep: Option<&str>) {
        let Ok(entries) = fs::read_dir(&self.dir) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "prefs.json" || name == changelog::FILE || Some(name.as_str()) == keep {
                continue;
            }
            // The unfinished download of the kept build, to resume.
            if let Some(k) = keep {
                let (part, meta) = part_paths(Path::new(k));
                if Path::new(&name) == part || Path::new(&name) == meta {
                    continue;
                }
            }
            if name == "staged.json" && keep.is_some() {
                continue;
            }
            let _ = fs::remove_file(e.path());
        }
    }
}

/// The learner-language lines Android shows once the new Kabar is in.
#[derive(Clone, Debug, Default)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub struct InstallTexts {
    pub lang: UiLanguage,
    pub done_title: String,
    pub done_body: String,
    /// "Kabar's update is ready: tap to install", if Android must ask
    /// while Kabar is in the background.
    pub ready_title: String,
}

#[cfg(target_os = "android")]
fn install_build(path: &Path, offer: &Offer, texts: &InstallTexts) -> Phase {
    use crate::platform::android;
    if !android::can_request_installs().unwrap_or(false) {
        let _ = android::open_install_permission();
        return Phase::NeedsPermission(offer.clone());
    }
    let path = path.to_string_lossy();
    match android::install_apk(
        &path,
        offer.asset.version_code.unwrap_or(0),
        &texts.done_title,
        &texts.done_body,
        &texts.ready_title,
    ) {
        Ok(()) => Phase::Installing(offer.clone()),
        Err(e) => Phase::Failed { offer: Some(offer.clone()), why: Failure::Install(e) },
    }
}

#[cfg(target_os = "macos")]
fn install_build(path: &Path, offer: &Offer, _texts: &InstallTexts) -> Phase {
    let Some(app) = bundle::running_bundle() else {
        return Phase::Failed {
            offer: Some(offer.clone()),
            why: Failure::Install("not running from Kabar.app".into()),
        };
    };
    match bundle::install_zip(path, &app, bundle::ditto_extract, |new| bundle::check_signature(new, &app)) {
        Ok(()) => Phase::Installed(offer.clone()),
        Err(e) => Phase::Failed { offer: Some(offer.clone()), why: Failure::Install(e) },
    }
}

#[cfg(not(any(target_os = "android", target_os = "macos")))]
fn install_build(_path: &Path, offer: &Offer, _texts: &InstallTexts) -> Phase {
    Phase::Failed { offer: Some(offer.clone()), why: Failure::Install("no installer on this system".into()) }
}

/// Mac, automatic mode: a verified download staged by the last session
/// goes in before the window opens, and Kabar reopens as the new version.
/// Called first thing in `main`.
#[cfg(target_os = "macos")]
pub fn apply_staged_on_launch() {
    let Ok(data) = crate::platform::data_dir() else { return };
    let dir = data.join("updates");
    let prefs = load_prefs(&dir);
    if prefs.mode != Mode::Auto {
        return;
    }
    let Some(offer) = load_staged(&dir) else { return };
    let info = BuildInfo::from_env();
    if info.key.is_none()
        || !is_newer(&info.installed(prefs.channel), &offer)
        || prefs.auto_failed.as_deref() == Some(&offer.key())
    {
        return;
    }
    let Some(app) = bundle::running_bundle() else { return };
    let path = dir.join(&offer.asset.file);
    let verified = fs::read(&path).is_ok_and(|bytes| {
        check_download(&offer.asset, bytes.len() as u64, &kabar_core::packs::sha256_hex(&bytes)).is_ok()
    });
    let result = if verified {
        bundle::install_zip(&path, &app, bundle::ditto_extract, |new| bundle::check_signature(new, &app))
    } else {
        Err("changed on disk".into())
    };
    match result {
        Ok(()) => {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(dir.join("staged.json"));
            bundle::relaunch(&app);
        }
        Err(e) => {
            crate::diag::report!("update {} not applied: {e}", offer.label());
            let mut prefs = prefs;
            prefs.auto_failed = Some(offer.key());
            let _ = save_prefs(&dir, &prefs);
        }
    }
}

/// Mac: quits and reopens the swapped-in Kabar.app.
#[cfg(target_os = "macos")]
pub fn restart() {
    if let Some(app) = bundle::running_bundle() {
        bundle::relaunch(&app);
    }
}

fn update_failure(e: UpdateError) -> Failure {
    match e {
        UpdateError::TooNew => Failure::TooNew,
        other => Failure::Verify(other.to_string()),
    }
}

/// Waits (in units of `retry_after`) before each retry of a download whose
/// connection dropped mid-file: 20 s, 1 min, 3 min in a real build.
const DOWNLOAD_RETRIES: [u32; 3] = [1, 3, 9];

/// A failed download try, and whether it got any new bytes (worth retrying
/// right away: the connection dropped rather than never opened).
#[derive(Debug)]
struct Fetch {
    why: Failure,
    progressed: bool,
}

impl Fetch {
    fn stopped(why: Failure) -> Self {
        Self { why, progressed: false }
    }
}

/// Which build a `.part` file belongs to, so only the same build resumes.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PartMeta {
    size: u64,
    sha256: String,
}

/// `<file>.part` and the `<file>.part.json` that says whose part it is.
fn part_paths(path: &Path) -> (PathBuf, PathBuf) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    (path.with_file_name(format!("{name}.part")), path.with_file_name(format!("{name}.part.json")))
}

fn read_part_meta(path: &Path) -> Option<PartMeta> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// The first byte of a 206 answer (`Content-Range: bytes 600-999/1000`).
fn range_start<B>(res: &ureq::http::Response<B>) -> Option<u64> {
    let v = res.headers().get("content-range")?.to_str().ok()?;
    v.strip_prefix("bytes ")?.split('-').next()?.trim().parse().ok()
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(timeout)).build().into()
}

fn get_small(url: &str) -> Result<Vec<u8>, Failure> {
    let mut res = agent(Duration::from_secs(30)).get(url).call().map_err(|_| Failure::Network)?;
    if res.status().as_u16() != 200 {
        return Err(Failure::Network);
    }
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(MAX_FEED_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::Network)?;
    if bytes.len() as u64 > MAX_FEED_BYTES {
        return Err(Failure::Verify("feed too large".into()));
    }
    Ok(bytes)
}

fn load_prefs(dir: &Path) -> Prefs {
    fs::read(dir.join("prefs.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_prefs(dir: &Path, prefs: &Prefs) -> Result<(), String> {
    write_json(dir, "prefs.json", prefs)
}

fn load_staged(dir: &Path) -> Option<Offer> {
    serde_json::from_slice(&fs::read(dir.join("staged.json")).ok()?).ok()
}

fn save_staged(dir: &Path, offer: &Offer) -> Result<(), String> {
    write_json(dir, "staged.json", offer)
}

fn write_json(dir: &Path, name: &str, value: &impl Serialize) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{name}.part"));
    fs::write(&tmp, serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join(name)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(version: &str, build: u64) -> Offer {
        Offer {
            version: version.into(),
            build,
            commit: None,
            notes: UpdateNotes::default(),
            asset: UpdateAsset {
                platform: UpdatePlatform::AndroidArm64,
                file: "kabar.apk".into(),
                size: 1,
                sha256: "00".repeat(32),
                version_code: None,
                build: None,
            },
            changes: Vec::new(),
        }
    }

    fn status(phase: Phase, mode: Mode) -> Status {
        Status {
            off: None,
            phase,
            prefs: Prefs { mode, ..Prefs::default() },
            channel: Channel::Alpha,
            preview: false,
            version: "1.0.0-alpha.1".into(),
            build: 0,
            content_build: 0,
            content_since_ms: None,
            content_waiting: None,
            content_check: None,
            packs_refreshed: 0,
            checking_all: false,
            history: Vec::new(),
        }
    }

    #[test]
    fn prefs_round_trip_and_default_to_automatic_alpha() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_prefs(dir.path()), Prefs::default());
        assert_eq!(Prefs::default().mode, Mode::Auto);
        let p = Prefs {
            mode: Mode::Notify,
            channel: Some(Channel::Beta),
            last_check_ms: Some(5),
            ..Prefs::default()
        };
        save_prefs(dir.path(), &p).unwrap();
        assert_eq!(load_prefs(dir.path()), p);
        let release =
            BuildInfo { version: Version::parse("1.0.0").unwrap(), build: 0, preview: false, key: None };
        assert_eq!(release.channel(None), Channel::Alpha);
        assert_eq!(release.channel(Some(Channel::Beta)), Channel::Beta);
        // A release build never follows the preview feed, whatever the file says.
        assert_eq!(release.channel(Some(Channel::Preview)), Channel::Alpha);
        let preview = BuildInfo { preview: true, ..release };
        assert_eq!(preview.channel(Some(Channel::Stable)), Channel::Preview);
    }

    #[test]
    fn the_banner_waits_for_something_to_do_and_can_be_closed() {
        let o = offer("1.0.0-alpha.2", 0);
        assert!(
            status(Phase::Available(o.clone()), Mode::Auto).banner().is_none(),
            "auto mode is still downloading"
        );
        assert!(status(Phase::Available(o.clone()), Mode::Notify).banner().is_some());
        assert!(status(Phase::Ready(o.clone()), Mode::Auto).banner().is_some());
        let mut s = status(Phase::Ready(o.clone()), Mode::Auto);
        s.prefs.dismissed = Some(o.key());
        assert!(s.banner().is_none());
        // A newer offer shows again.
        s.phase = Phase::Ready(offer("1.0.0-alpha.3", 0));
        assert!(s.banner().is_some());
    }

    #[test]
    fn offers_label_previews_by_build() {
        assert_eq!(offer("1.0.0-alpha.2", 0).label(), "1.0.0-alpha.2");
        assert_eq!(offer("0.1.0", 141).label(), "0.1.0 (141)");
        let me =
            Installed { version: Version::parse("0.1.0").unwrap(), build: 140, channel: Channel::Preview };
        assert!(is_newer(&me, &offer("0.1.0", 141)));
        assert!(!is_newer(&me, &offer("0.1.0", 140)));
    }

    #[test]
    fn staged_offer_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_staged(dir.path()).is_none());
        let o = offer("1.0.0-alpha.2", 0);
        save_staged(dir.path(), &o).unwrap();
        assert_eq!(load_staged(dir.path()), Some(o));
    }

    /// Serves `dir` over plain HTTP on localhost, one request per connection.
    fn serve(dir: PathBuf) -> String {
        serve_logged(dir).0
    }

    /// A tiny feed server: answers `Range: bytes=N-` with 206, and when
    /// `<file>.cut` holds a number, sends only that many bytes of each answer
    /// before hanging up (a dropped connection). Logs each request's path
    /// and range start.
    /// Each request's path and the byte its range starts at.
    type RequestLog = std::sync::Arc<Mutex<Vec<(String, u64)>>>;

    fn serve_logged(dir: PathBuf) -> (String, RequestLog) {
        use std::io::{BufRead as _, BufReader};
        let log = RequestLog::default();
        let seen = log.clone();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_owned();
                let mut from = 0u64;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                    let lower = h.to_ascii_lowercase();
                    if let Some(r) = lower.strip_prefix("range: bytes=") {
                        from = r.trim().trim_end_matches('-').parse().unwrap_or(0);
                    }
                }
                seen.lock().unwrap().push((path.clone(), from));
                let mut stream = reader.into_inner();
                let cut: Option<usize> = fs::read_to_string(dir.join(format!("{path}.cut")))
                    .ok()
                    .and_then(|c| c.trim().parse().ok());
                let _ = match fs::read(dir.join(&path)) {
                    Ok(body) if from as usize >= body.len() && from > 0 => stream.write_all(
                        b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                    Ok(body) => {
                        let total = body.len();
                        let rest = &body[from as usize..];
                        let head = if from > 0 {
                            format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                total - 1,
                                rest.len()
                            )
                        } else {
                            format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n")
                        };
                        let sent = &rest[..cut.unwrap_or(rest.len()).min(rest.len())];
                        stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(sent))
                    }
                    Err(_) => stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
            }
        });
        (url, log)
    }

    fn fixture_feed() -> tempfile::TempDir {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/update");
        let feed = tempfile::tempdir().unwrap();
        for name in [MANIFEST_FILE, SIGNATURE_FILE] {
            fs::copy(src.join(name), feed.path().join(name)).unwrap();
        }
        // The signed manifest's Android asset: these bytes (no .apk is tracked).
        fs::write(feed.path().join("kabar-1.0.0-alpha.2-android-arm64.apk"), b"fixture apk bytes").unwrap();
        feed
    }

    fn updater(feed: &str, version: &str) -> (&'static Updater, tempfile::TempDir) {
        let data = tempfile::tempdir().unwrap();
        (updater_in(data.path(), feed, version), data)
    }

    fn updater_in(data: &Path, feed: &str, version: &str) -> &'static Updater {
        let key = include_str!("../../../../fixtures/update/test-key.pub");
        let info = BuildInfo {
            version: Version::parse(version).unwrap(),
            build: 0,
            preview: false,
            key: Some(parse_public_key(key).unwrap()),
        };
        let mut u = Updater::new(
            data.join("updates"),
            info,
            Some(UpdatePlatform::AndroidArm64),
            Some(feed.to_owned()),
            None,
        );
        u.retry_after = Duration::from_millis(10);
        Box::leak(Box::new(u))
    }

    /// Android's job can start the process before any UI (#429): the app's
    /// start then takes the job's updater instead of opening a second one.
    /// The only test that touches the process-wide updater.
    #[test]
    fn the_job_and_the_app_share_one_updater() {
        let data = tempfile::tempdir().unwrap();
        let job = job_updater(data.path());
        assert!(get().is_some_and(|u| std::ptr::eq(u, job)), "the job's updater is the app's");
        init(data.path());
        init(data.path());
        assert!(std::ptr::eq(get().unwrap(), job), "init reuses it");
        assert!(std::ptr::eq(job_updater(data.path()), job));
        let local = OnceLock::new();
        assert!(std::ptr::eq(shared(&local, data.path()), shared(&local, data.path())));
    }

    #[test]
    fn whats_new_before_and_after_an_update() {
        let feed = fixture_feed();
        let url = serve(feed.path().to_path_buf());
        let data = tempfile::tempdir().unwrap();

        // A first install says nothing about what's new.
        let u = updater_in(data.path(), &url, "1.0.0-alpha.1");
        assert!(u.status().whats_new().is_empty());
        assert_eq!(u.status().prefs.last_run.as_deref(), Some("1.0.0-alpha.1+0"));

        // Before installing: the offer brings the builds after this one.
        u.check(false);
        let Phase::Available(o) = u.status().phase else { panic!("{:?}", u.status().phase) };
        assert_eq!(o.changes.iter().map(BuildChanges::key).collect::<Vec<_>>(), ["1.0.0-alpha.2+0"]);
        assert_eq!(u.status().history.len(), 2, "the whole feed history is kept");
        let ctx = NoticeContext { lang: UiLanguage::ArEg, ..NoticeContext::default() };
        u.set_notice_context(ctx);
        let n = notice(&u.status(), None, 0).unwrap();
        assert!(n.body.starts_with("• حلقات راديو جديدة\n• القارئ بقى أسرع\n• الصوت"), "{}", n.body);

        // The first launch of the new build opens "What's new" once.
        let u = updater_in(data.path(), &url, "1.0.0-alpha.2");
        let w = u.status().whats_new();
        assert_eq!(w.iter().map(BuildChanges::key).collect::<Vec<_>>(), ["1.0.0-alpha.2+0"]);
        u.close_whats_new();
        assert!(u.status().whats_new().is_empty());
        let u = updater_in(data.path(), &url, "1.0.0-alpha.2");
        assert!(u.status().whats_new().is_empty(), "seen stays seen");
    }

    #[test]
    fn a_content_bundle_comes_without_an_install() {
        use base64::Engine as _;
        use ed25519_dalek::Signer as _;
        let packed = miniz_oxide::deflate::compress_to_vec(b"cast: []\n", 10);
        let bundle = kabar_protocol::bundle::encode(&[("CAST".into(), packed)]);
        let feed = tempfile::tempdir().unwrap();
        let m = UpdateManifest {
            format: kabar_protocol::update::UPDATE_FORMAT,
            channel: Channel::Alpha,
            version: "1.0.0-alpha.1".into(),
            build: 0,
            commit: None,
            published_at: "2026-10-28T08:00:00Z".into(),
            notes: UpdateNotes::default(),
            assets: Vec::new(),
            history: Vec::new(),
            content: Some(kabar_protocol::update::ContentFile {
                file: "kabar-content.bundle".into(),
                size: bundle.len() as u64,
                sha256: kabar_core::packs::sha256_hex(&bundle),
                ui_build: 0,
            }),
        };
        let json = serde_json::to_vec(&m).unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x4b; 32]);
        let sig = base64::engine::general_purpose::STANDARD.encode(key.sign(&json).to_bytes());
        fs::write(feed.path().join(MANIFEST_FILE), &json).unwrap();
        fs::write(feed.path().join(SIGNATURE_FILE), sig).unwrap();
        fs::write(feed.path().join("kabar-content.bundle"), &bundle).unwrap();
        let url = serve(feed.path().to_path_buf());

        let (u, data) = updater(&url, "1.0.0-alpha.1");
        // "Check for updates" by hand: the feed, the content, then the packs.
        u.check_everything();
        assert_eq!(u.status().phase, Phase::UpToDate, "the same build: nothing to install");
        assert_eq!(u.status().content_waiting, Some(m.build));
        assert_eq!(u.status().content_check, Some(content::Refresh::Taken(m.build)));
        assert!(!u.status().checking_all, "the check by hand is over");
        assert_eq!(u.status().packs_refreshed, 0, "no audio packs on this device");
        let (entries, look) = content::load(data.path(), &u.info).unwrap();
        assert_eq!((entries.len(), look), (1, true), "the look fits the build it was made with");
        // Taken once; a newer app drops it.
        let again = updater_in(data.path(), &url, "1.0.0-alpha.1");
        again.check(false);
        assert_eq!(again.status().content_waiting, None);
        assert_eq!(
            again.status().content_check,
            Some(content::Refresh::Current),
            "nothing newer than what's in use"
        );
        let newer = BuildInfo { version: Version::parse("1.0.0-alpha.2").unwrap(), ..u.info.clone() };
        assert!(content::load(data.path(), &newer).is_none());
        assert!(content::load(data.path(), &u.info).is_none(), "and it is gone");
    }

    #[test]
    fn checks_downloads_and_verifies_from_a_signed_feed() {
        let feed = fixture_feed();
        let url = serve(feed.path().to_path_buf());

        let (u, _data) = updater(&url, "1.0.0-alpha.1");
        u.check(true);
        let Phase::Ready(offer) = u.status().phase else { panic!("{:?}", u.status().phase) };
        assert_eq!(offer.version, "1.0.0-alpha.2");
        assert_eq!(offer.asset.version_code, Some(10_000_102));
        assert!(u.verified_staged(&offer).is_ok());
        assert!(u.status().prefs.last_check_ms.is_some());
        // A fresh start finds the staged download again.
        let again = Updater::new(u.dir.clone(), u.info.clone(), u.platform, u.feed_override.clone(), None);
        assert_eq!(again.status().phase, Phase::Ready(offer.clone()));
        // Changed on disk after the download: refused at install time.
        fs::write(u.dir.join(&offer.asset.file), b"fixture apk bytez").unwrap();
        assert!(matches!(u.verified_staged(&offer), Err(Failure::Verify(_))));

        // Already on that version: nothing to do, and old downloads go.
        let (u, _data) = updater(&url, "1.0.0-alpha.2");
        u.check(true);
        assert_eq!(u.status().phase, Phase::UpToDate);
    }

    #[test]
    fn a_notice_goes_out_once_per_offer_and_step() {
        let noon = Some(LocalTime::hm(12, 0));
        let mut o = offer("1.0.0-alpha.2", 0);
        o.notes = UpdateNotes { en: "Javanese Sinetron, episode 12".into(), ..UpdateNotes::default() };
        let mut s = status(Phase::Available(o.clone()), Mode::Notify);
        s.prefs.notice = Some(NoticeContext { lang: UiLanguage::En, ..NoticeContext::default() });
        let n = notice(&s, noon, 0).expect("a new version is announced");
        assert_eq!(n.title, "A new Kabar is out · 1.0.0-alpha.2");
        assert!(n.body.starts_with("What's new: Javanese Sinetron, episode 12\n"), "{}", n.body);
        assert_eq!((n.action, n.button.as_str()), ("download", "Download"));
        // Sent once: the same offer at the same step stays quiet.
        s.prefs.notified = Some(n.key.clone());
        assert!(notice(&s, noon, 0).is_none());
        // Downloaded since: "ready to install" is a new step.
        s.phase = Phase::Ready(o.clone());
        let ready = notice(&s, noon, 0).unwrap();
        assert_eq!(ready.action, "install");
        assert_ne!(ready.key, n.key);
        // A closed banner, the switch, or updates off: nothing.
        let mut closed = s.clone();
        closed.prefs.dismissed = Some(o.key());
        assert!(notice(&closed, noon, 0).is_none());
        let mut quiet = s.clone();
        quiet.prefs.notify = false;
        assert!(notice(&quiet, noon, 0).is_none());
        let mut off = s.clone();
        off.prefs.mode = Mode::Off;
        assert!(notice(&off, noon, 0).is_none());
        // Nothing to say while checking or up to date.
        assert!(notice(&status(Phase::UpToDate, Mode::Auto), noon, 0).is_none());
    }

    #[test]
    fn notices_keep_quiet_hours_and_speak_the_learners_language() {
        let mut s = status(Phase::Ready(offer("0.1.0", 141)), Mode::Auto);
        let quiet = QuietHours { start: LocalTime::hm(22, 30), end: LocalTime::hm(8, 0) };
        s.prefs.notice =
            Some(NoticeContext { lang: UiLanguage::ArEg, quiet: Some(quiet), utc_offset_min: 120 });
        assert!(notice(&s, Some(LocalTime::hm(23, 15)), 0).is_none(), "inside quiet hours");
        assert!(notice(&s, Some(LocalTime::hm(7, 59)), 0).is_none());
        let n = notice(&s, Some(LocalTime::hm(8, 0)), 0).expect("quiet hours are over");
        assert!(n.title.starts_with("نسخة Kabar الجديدة جاهزة"), "{}", n.title);
        assert!(n.title.ends_with("0.1.0 (141)"));
        // No notes: the tap hint alone.
        assert_eq!(n.body, text::ut(UiLanguage::ArEg, text::U::NoticeTapReady));
        // Without a local time, the stored offset decides: 21:00 UTC is 23:00 in Cairo.
        assert!(notice(&s, None, 21 * 3600 * 1000).is_none());
        assert!(notice(&s, None, 12 * 3600 * 1000).is_some());
        // The prefs file keeps all of it, and old files default the switch on.
        let dir = tempfile::tempdir().unwrap();
        save_prefs(dir.path(), &s.prefs).unwrap();
        assert_eq!(load_prefs(dir.path()), s.prefs);
        fs::write(dir.path().join("prefs.json"), r#"{"mode":"notify"}"#).unwrap();
        assert!(load_prefs(dir.path()).notify);
    }

    #[test]
    fn the_background_check_downloads_on_wifi_and_announces_once() {
        let feed = fixture_feed();
        let url = serve(feed.path().to_path_buf());
        let noon = LocalTime::hm(12, 0);

        // Automatic mode on Wi-Fi: downloaded, then "ready to install".
        let (u, _data) = updater(&url, "1.0.0-alpha.1");
        let n = u.background(false, true, noon).expect("announced");
        assert_eq!(n.action, "install");
        assert!(matches!(u.status().phase, Phase::Ready(_)));
        // Not shown (Android said no): offered again at the next run.
        assert_eq!(u.status().prefs.notified, None, "marked only once shown");
        assert_eq!(u.background(false, true, noon).as_ref(), Some(&n));
        u.shown(&n.key);
        assert_eq!(u.status().prefs.notified.as_deref(), Some(n.key.as_str()));
        assert!(u.background(false, true, noon).is_none(), "once shown, never again");
        assert!(serde_json::to_string(&n).unwrap().contains(&format!("\"key\":\"{}\"", n.key)));

        // On mobile data: announced, not downloaded.
        let (u, _data) = updater(&url, "1.0.0-alpha.1");
        let n = u.background(true, true, noon).unwrap();
        assert_eq!(n.action, "download");
        assert!(matches!(u.status().phase, Phase::Available(_)));

        // Kabar in front (or notifications off): checked, kept for later.
        let (u, _data) = updater(&url, "1.0.0-alpha.1");
        assert!(u.background(true, false, noon).is_none());
        assert!(u.status().prefs.notified.is_none());
        assert!(u.background(true, true, noon).is_some(), "the next run sends it");

        // Up to date: nothing.
        let (u, _data) = updater(&url, "1.0.0-alpha.2");
        assert!(u.background(false, true, noon).is_none());
    }

    #[test]
    fn a_dropped_download_picks_up_where_it_stopped() {
        let apk = "kabar-1.0.0-alpha.2-android-arm64.apk";
        let feed = fixture_feed();
        let cut = feed.path().join(format!("{apk}.cut"));
        let (url, log) = serve_logged(feed.path().to_path_buf());
        let ranges = || -> Vec<u64> {
            let mut log = log.lock().unwrap();
            let r = log.iter().filter(|(p, _)| p == apk).map(|(_, from)| *from).collect();
            log.clear();
            r
        };

        // Every answer stops after 6 bytes: the 17-byte build comes in three
        // pieces, each try asking for the rest, and the whole is verified.
        fs::write(&cut, "6").unwrap();
        let (u, data) = updater(&url, "1.0.0-alpha.1");
        u.check(false);
        u.download();
        assert!(matches!(u.status().phase, Phase::Ready(_)), "{:?}", u.status().phase);
        assert_eq!(ranges(), [0, 6, 12]);
        let dir = data.path().join("updates");
        assert_eq!(fs::read(dir.join(apk)).unwrap(), b"fixture apk bytes");
        assert!(!dir.join(format!("{apk}.part")).exists());
        assert!(!dir.join(format!("{apk}.part.json")).exists());

        // Out of quick retries (2 bytes a try): the part stays, and the next
        // try, a tap or the next check later, continues it.
        fs::write(&cut, "2").unwrap();
        let (u, data) = updater(&url, "1.0.0-alpha.1");
        u.check(false);
        u.download();
        assert!(matches!(u.status().phase, Phase::Failed { why: Failure::Network, .. }));
        assert_eq!(ranges(), [0, 2, 4, 6]);
        let part = data.path().join("updates").join(format!("{apk}.part"));
        assert_eq!(fs::read(&part).unwrap(), b"fixture ");
        fs::remove_file(&cut).unwrap();
        u.download();
        assert!(matches!(u.status().phase, Phase::Ready(_)), "{:?}", u.status().phase);
        assert_eq!(ranges(), [8]);

        // A damaged part of this build: the resumed file fails the hash, so
        // it's fetched whole once more.
        let (u, data) = updater(&url, "1.0.0-alpha.1");
        u.check(false);
        let Phase::Available(o) = u.status().phase else { panic!() };
        let dir = data.path().join("updates");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{apk}.part")), b"FIXTURE ").unwrap();
        let meta = PartMeta { size: o.asset.size, sha256: o.asset.sha256.clone() };
        fs::write(dir.join(format!("{apk}.part.json")), serde_json::to_vec(&meta).unwrap()).unwrap();
        u.download();
        assert!(matches!(u.status().phase, Phase::Ready(_)), "{:?}", u.status().phase);
        assert_eq!(ranges(), [8, 0]);

        // A part left by another build is never continued: it starts over.
        let (u, data) = updater(&url, "1.0.0-alpha.1");
        let dir = data.path().join("updates");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{apk}.part")), b"old buil").unwrap();
        fs::write(dir.join(format!("{apk}.part.json")), r#"{"size":17,"sha256":"00"}"#).unwrap();
        u.check(false);
        u.download();
        assert!(matches!(u.status().phase, Phase::Ready(_)), "{:?}", u.status().phase);
        assert_eq!(ranges(), [0]);
    }

    #[test]
    fn a_swapped_build_or_manifest_is_never_offered() {
        // The build on the server isn't the one the manifest signed.
        let feed = fixture_feed();
        fs::write(feed.path().join("kabar-1.0.0-alpha.2-android-arm64.apk"), b"evil apk bytes!!!").unwrap();
        let url = serve(feed.path().to_path_buf());
        let (u, data) = updater(&url, "1.0.0-alpha.1");
        u.check(true);
        assert!(
            matches!(u.status().phase, Phase::Failed { why: Failure::Verify(_), .. }),
            "{:?}",
            u.status().phase
        );
        let mut left: Vec<_> =
            fs::read_dir(data.path().join("updates")).unwrap().flatten().map(|e| e.file_name()).collect();
        left.sort();
        // The signed manifest's history is kept; the build it didn't sign isn't.
        assert_eq!(left, ["history.json", "prefs.json"], "nothing unverified stays on disk");

        // The manifest changed after signing.
        let feed = fixture_feed();
        let json = fs::read_to_string(feed.path().join(MANIFEST_FILE)).unwrap();
        fs::write(feed.path().join(MANIFEST_FILE), json.replace("alpha.2\"", "alpha.9\"")).unwrap();
        let url = serve(feed.path().to_path_buf());
        let (u, _data) = updater(&url, "1.0.0-alpha.1");
        u.check(true);
        assert!(matches!(u.status().phase, Phase::Failed { why: Failure::Verify(_), .. }));

        // No connection.
        let (u, _data) = updater("http://127.0.0.1:9", "1.0.0-alpha.1");
        u.check(false);
        assert!(matches!(u.status().phase, Phase::Failed { why: Failure::Network, .. }));
    }
}
