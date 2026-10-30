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

mod service;

mod model;
pub(crate) use model::notes_in;
use model::*;
pub use model::{Failure, Mode, Notice, NoticeContext, Off, Offer, Phase, Prefs, Status, notice};

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
mod tests;
