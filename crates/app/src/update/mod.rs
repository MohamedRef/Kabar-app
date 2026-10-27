//! App updates over the air (`docs/RELEASE.md` §5): Kabar checks its update
//! feed on GitHub Releases, verifies the feed's Ed25519 signature and each
//! download's SHA-256, then installs it: PackageInstaller on Android, a
//! swapped Kabar.app on the Mac. Offline packs ride along: a check also
//! refreshes installed packs that have a newer version.
//!
//! Device-local, never synced:
//!
//! ```text
//! <data>/updates/prefs.json    automatic / notify / off, channel, last check
//! <data>/updates/staged.json   the verified download waiting to install
//! <data>/updates/<file>        that download (one at a time)
//! ```
//!
//! The work runs on its own thread; the UI follows [`Status`] through a
//! `tokio::sync::watch` channel.

pub mod bundle;
mod text;
pub mod view;

use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kabar_core::update::{
    Decision, Installed, UpdateError, VerifyingKey, Version, check_download, decide, parse_public_key,
    this_platform, verify_manifest,
};
use kabar_protocol::update::{
    Channel, MANIFEST_FILE, SIGNATURE_FILE, UpdateAsset, UpdateManifest, UpdateNotes, UpdatePlatform,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

/// Where the feeds live: one GitHub release per channel of the public repo
/// (free; no token needed to download).
const FEEDS_URL: &str = "https://github.com/MohamedRef/Kabar-app/releases/download";
/// The release public key (`just update-keygen` fills it once).
const RELEASE_KEY: &str = include_str!("../../update-key.pub");
/// How often a running Kabar looks for an update by itself.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
/// First automatic check after launch: late enough not to slow the start.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
}

impl Offer {
    fn from_manifest(m: &UpdateManifest, asset: UpdateAsset) -> Self {
        Self {
            version: m.version.clone(),
            build: m.build,
            commit: m.commit.clone(),
            notes: m.notes.clone(),
            asset,
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
}

impl Status {
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

static UPDATER: OnceLock<Updater> = OnceLock::new();

/// Opens the updater under the app's data directory and starts the
/// background checks. Called once at start.
pub fn init(data_dir: &Path) {
    let feed_override = dev_override("KABAR_UPDATE_URL");
    let platform =
        dev_override("KABAR_UPDATE_PLATFORM").and_then(|p| UpdatePlatform::parse(&p)).or_else(this_platform);
    let off = if cfg!(debug_assertions) && feed_override.is_none() {
        Some(Off::DevBuild)
    } else if cfg!(target_os = "macos") && bundle::running_bundle().is_none() && feed_override.is_none() {
        Some(Off::NotInBundle)
    } else {
        None
    };
    let updater = Updater::new(data_dir.join("updates"), BuildInfo::from_env(), platform, feed_override, off);
    let start = updater.status().off.is_none();
    if UPDATER.set(updater).is_err() || !start {
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
}

pub fn get() -> Option<&'static Updater> {
    UPDATER.get()
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
        let installed = info.installed(prefs.channel);
        let mut phase = Phase::Idle;
        if off.is_none()
            && let Some(offer) = load_staged(&dir)
            && is_newer(&installed, &offer)
            && dir.join(&offer.asset.file).metadata().is_ok_and(|m| m.len() == offer.asset.size)
        {
            phase = Phase::Ready(offer);
        }
        let status = Status { off, phase, channel: installed.channel, preview: info.preview, prefs };
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
    fn installed(&self) -> Installed {
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
    /// offline packs.
    fn tick(&'static self) {
        let s = self.status();
        if s.off.is_some() || s.prefs.mode == Mode::Off {
            return;
        }
        let due = s
            .prefs
            .last_check_ms
            .is_none_or(|t| now_ms().saturating_sub(t) >= CHECK_EVERY.as_millis() as u64);
        // Automatic downloads wait for Wi-Fi (a ~45 MB APK on mobile data,
        // up to every merge on the preview channel); the offer still shows.
        let auto = s.prefs.mode == Mode::Auto && !crate::platform::metered_network();
        if due {
            self.check(auto);
            crate::packs::update_installed();
        } else if auto && let Phase::Available(_) = s.phase {
            self.download();
        }
    }

    /// Looks at the feed; with `download`, fetches a newer build right away.
    pub fn check(&'static self, download: bool) {
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
        let result = self.fetch_offer();
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
    fn fetch_offer(&self) -> Result<Option<Offer>, Failure> {
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
                Ok(Some(Offer::from_manifest(&m, asset)))
            }
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
        match self.fetch_build(&format!("{base}/{}", offer.asset.file), &offer, &path) {
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
    /// SHA-256 against the signed manifest, then renames it into place.
    fn fetch_build(&self, url: &str, offer: &Offer, path: &Path) -> Result<(), Failure> {
        let mut res = agent(Duration::from_secs(30 * 60)).get(url).call().map_err(|_| Failure::Network)?;
        if res.status().as_u16() != 200 {
            return Err(Failure::Network);
        }
        let part = path.with_extension("part");
        let mut file = fs::File::create(&part).map_err(|e| Failure::Install(e.to_string()))?;
        let mut reader = res.body_mut().as_reader().take(offer.asset.size + 1);
        let mut hash = Sha256::new();
        let (mut done, mut shown) = (0u64, 0u64);
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|_| Failure::Network)?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| Failure::Install(e.to_string()))?;
            done += n as u64;
            if done - shown > offer.asset.size / 100 {
                shown = done;
                self.set_phase(Phase::Downloading { offer: offer.clone(), done });
            }
        }
        file.sync_all().map_err(|e| Failure::Install(e.to_string()))?;
        drop(file);
        let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if let Err(e) = check_download(&offer.asset, done, &hex) {
            let _ = fs::remove_file(&part);
            return Err(update_failure(e));
        }
        fs::rename(&part, path).map_err(|e| Failure::Install(e.to_string()))
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
        let next = install_build(&path, &offer, &texts);
        if let Phase::Installed(_) = next {
            self.clear_downloads(None);
        }
        self.set_phase(next);
    }

    pub fn set_mode(&self, mode: Mode) {
        self.update_prefs(|p| p.mode = mode);
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
            if name == "prefs.json" || Some(name.as_str()) == keep {
                continue;
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
            },
        }
    }

    fn status(phase: Phase, mode: Mode) -> Status {
        Status {
            off: None,
            phase,
            prefs: Prefs { mode, ..Prefs::default() },
            channel: Channel::Alpha,
            preview: false,
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
        use std::io::{BufRead as _, BufReader};
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
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                }
                let mut stream = reader.into_inner();
                let _ = match fs::read(dir.join(&path)) {
                    Ok(body) => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(&body))
                    }
                    Err(_) => stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
            }
        });
        url
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
        let key = include_str!("../../../../fixtures/update/test-key.pub");
        let info = BuildInfo {
            version: Version::parse(version).unwrap(),
            build: 0,
            preview: false,
            key: Some(parse_public_key(key).unwrap()),
        };
        let mut u = Updater::new(
            data.path().join("updates"),
            info,
            Some(UpdatePlatform::AndroidArm64),
            Some(feed.to_owned()),
            None,
        );
        u.retry_after = Duration::from_millis(10);
        (Box::leak(Box::new(u)), data)
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
        let left: Vec<_> =
            fs::read_dir(data.path().join("updates")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, ["prefs.json"], "nothing unverified stays on disk");

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
