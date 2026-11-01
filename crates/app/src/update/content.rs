//! Content bundles from the update feed (`docs/RELEASE.md` §5): lessons,
//! tests, stories, Radio, glosses, the Bridge, curricula and the
//! stylesheets, as one signed file (`kabar_protocol::bundle`). A check
//! downloads a newer one in the background; at the next launch the app reads
//! its content from it instead of the copy built in (`crate::embedded`), so a
//! content change needs no app install.
//!
//! ```text
//! <data>/content/content.<sha>.bundle   bundles, addressed by SHA-256
//! <data>/content/content.bundle         a pre-#513 install's bundle
//! <data>/content/content.json           the selected bundle (BundleInfo)
//! <data>/content/content.previous.json  the selection before the last download
//! <data>/content/content.good.json      the last bundle Kabar ran on calmly
//! <data>/content/content.starts.json    launches of an unconfirmed bundle
//! <data>/content/content.rejected.json  bundles that kept Kabar from starting
//! ```
//!
//! A bundle goes through these states: downloaded and verified (`refresh`),
//! selected (`content.json`), activated at launch (`load`), and confirmed
//! stable once Kabar has drawn its screens and run for [`STABLE_AFTER`] on it
//! (`confirm_running`). Only a confirmed bundle becomes last-known-good;
//! the good selector is written by nothing else, so a run of downloads can
//! never push it out. A bundle that was activated [`MAX_UNCONFIRMED_STARTS`]
//! times without being confirmed is rejected: launch falls back to the good
//! bundle, then the previous selection, then the built-in content, and the
//! rejected digest is never activated or downloaded again. The good bundle
//! itself is never counted, so a crash that isn't about content can't
//! reject it.
//!
//! A bundle is used only while it is at least as new as the running build
//! (an app update brings newer content built in, and the old bundle goes),
//! only when its hash still matches, and its stylesheets only on the exact
//! build they were made with.

use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::time::Duration;

use kabar_core::update::{Installed, Version, content_update};
use kabar_protocol::update::UpdateManifest;
use serde::{Deserialize, Serialize};

use super::{BuildInfo, agent};

const DIR: &str = "content";
const BUNDLE: &str = "content.bundle";
const INFO: &str = "content.json";
const BACKUP_INFO: &str = "content.previous.json";
const GOOD_INFO: &str = "content.good.json";
const STARTS: &str = "content.starts.json";
const REJECTED: &str = "content.rejected.json";
/// Bundles this app build could not read, so they aren't fetched again
/// until the app changes (`Unreadable`).
const UNREADABLE: &str = "content.unreadable.json";
/// Temporary name for the copy a legacy bundle gets when it is confirmed.
const GOOD_PART: &str = "content.good.bundle.part";

/// Launches of a bundle that never reached [`STABLE_AFTER`] before it is
/// treated as the reason Kabar can't start. Three keeps a user who closes
/// the app quickly once or twice from losing a good update.
pub(super) const MAX_UNCONFIRMED_STARTS: u8 = 3;
/// How long Kabar runs on a bundle, after its screens are drawn, before
/// the bundle counts as stable.
pub const STABLE_AFTER: Duration = Duration::from_secs(10);
/// Rejected digests remembered (oldest dropped first).
const MAX_REJECTED: usize = 16;

/// New downloads use a content-addressed filename. The metadata pointer
/// changes only after those bytes are fully written and verified, so a crash
/// mid-update leaves the previous selected bundle untouched.
fn bundle_path(dir: &Path, sha: &str) -> Option<std::path::PathBuf> {
    if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(dir.join(format!("content.{}.bundle", sha.to_ascii_lowercase())))
}

fn selected_path(dir: &Path, meta: &BundleInfo) -> std::path::PathBuf {
    // Existing installations used content.bundle; retain compatibility.
    bundle_path(dir, &meta.sha256).filter(|p| p.exists()).unwrap_or_else(|| dir.join(BUNDLE))
}

/// Where the bundle on disk came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct BundleInfo {
    pub version: String,
    /// The feed's build.
    pub build: u64,
    pub sha256: String,
    /// The app build its stylesheets fit.
    pub ui_build: u64,
    /// When it arrived on this device (ms since the epoch).
    #[serde(default)]
    pub fetched_ms: Option<u64>,
}

/// What a check did about the content bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    /// Nothing newer than what's here (in use or waiting).
    Current,
    /// This feed build's bundle is downloaded; it's used from the next launch.
    Taken(u64),
    /// A newer one exists but didn't come through; the next check tries again.
    Failed,
}

impl BundleInfo {
    fn order(&self) -> Option<(Version, u64)> {
        Version::parse(&self.version).map(|v| (v, self.build))
    }
}

/// The bundle on disk, when it is at least as new as this build.
pub(super) fn in_use(data: &Path, info: &BuildInfo) -> Option<BundleInfo> {
    let meta: BundleInfo = serde_json::from_slice(&fs::read(data.join(DIR).join(INFO)).ok()?).ok()?;
    (meta.order()? >= (info.version, info.build)).then_some(meta)
}

/// The bundle activated by this process, for [`confirm_running`].
static ACTIVE: std::sync::OnceLock<(std::path::PathBuf, BundleInfo)> = std::sync::OnceLock::new();

/// Reads content from the downloaded bundle from now on, when there is one
/// for this build; a stale one (the app was updated since) is removed.
/// Called at launch, before anything reads content.
pub fn use_installed(data: &Path) {
    let info = BuildInfo::from_env();
    if let Some((meta, entries, look)) = activate(data, &info) {
        let _ = ACTIVE.set((data.to_path_buf(), meta));
        crate::embedded::use_bundle(entries, look);
    }
}

/// Kabar has drawn its screens and run [`STABLE_AFTER`] on the bundle it
/// activated at launch: that bundle becomes last-known-good. Reads and
/// may copy the bundle, so call it off the UI thread. Runs once.
pub fn confirm_running() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Some((data, meta)) = ACTIVE.get() else { return };
    if let Err(e) = confirm_stable(data, meta) {
        crate::diag::report!("content bundle {} not confirmed: {e}", meta.build);
    }
}

type Entries = Vec<(String, Vec<u8>)>;

fn read_meta(dir: &Path, selector: &str) -> Option<BundleInfo> {
    serde_json::from_slice(&fs::read(dir.join(selector)).ok()?).ok()
}

fn same_sha(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Verify both the selected file and the matching metadata before activating
/// it: the selector parses, fits this build, isn't rejected, and its bundle
/// is whole and decodes.
fn validated(dir: &Path, info: &BuildInfo, selector: &str) -> Option<(BundleInfo, Entries, bool)> {
    let meta = read_meta(dir, selector)?;
    if meta.order()? < (info.version, info.build) || is_rejected(dir, &meta.sha256) {
        return None;
    }
    let bytes = fs::read(selected_path(dir, &meta)).ok()?;
    if bytes.len() as u64 > kabar_protocol::bundle::MAX_BYTES
        || !same_sha(&kabar_core::packs::sha256_hex(&bytes), &meta.sha256)
    {
        return None;
    }
    let entries = kabar_protocol::bundle::decode(&bytes).ok()?;
    // Cheap enough for every launch: the full check ran when it was taken.
    let issues = kabar_core::bundle_check::check_keys(entries.iter().map(|(k, _)| k.as_str()));
    if let Some(first) = issues.first() {
        crate::diag::report!("content bundle {} not used: {first}", meta.build);
        return None;
    }
    let look = meta.order() == Some((info.version, meta.ui_build));
    Some((meta, entries, look))
}

/// Unpacks every file of a bundle as text, within
/// `kabar_protocol::bundle::MAX_FILE_UNPACKED` each and `MAX_UNPACKED` in all.
pub(super) fn unpack_all(entries: &[(String, Vec<u8>)]) -> Result<Vec<(String, String)>, String> {
    use kabar_protocol::bundle::{MAX_FILE_UNPACKED, MAX_UNPACKED};
    let mut total = 0u64;
    let mut files = Vec::with_capacity(entries.len());
    for (key, packed) in entries {
        let raw = miniz_oxide::inflate::decompress_to_vec_with_limit(packed, MAX_FILE_UNPACKED)
            .map_err(|_| format!("{key} doesn't unpack (or is over {MAX_FILE_UNPACKED} bytes)"))?;
        total += raw.len() as u64;
        if total > MAX_UNPACKED {
            return Err(format!("the files unpack to over {MAX_UNPACKED} bytes"));
        }
        let text = String::from_utf8(raw).map_err(|_| format!("{key} isn't UTF-8"))?;
        files.push((key.clone(), text));
    }
    Ok(files)
}

/// Writes `bytes` to `path` through `temp`, flushed to disk before the
/// rename, so a crash leaves either the old file or the whole new one.
fn write_atomic(dir: &Path, temp: &str, path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    let temp = dir.join(temp);
    let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(bytes).and_then(|()| file.sync_all()).map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&temp, path).map_err(|e| e.to_string())?;
    // Make the rename itself durable where the platform allows it.
    #[cfg(unix)]
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Write selectors through a temporary file, never exposing partial JSON.
fn commit_selector(dir: &Path, selector: &str, json: &[u8]) -> Result<(), String> {
    write_atomic(dir, &format!("{selector}.part"), &dir.join(selector), json)
}

/// Launches of one unconfirmed bundle.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Starts {
    sha256: String,
    count: u8,
}

fn starts_of(dir: &Path, sha: &str) -> u8 {
    fs::read(dir.join(STARTS))
        .ok()
        .and_then(|b| serde_json::from_slice::<Starts>(&b).ok())
        .filter(|s| same_sha(&s.sha256, sha))
        .map_or(0, |s| s.count)
}

fn rejected(dir: &Path) -> Vec<String> {
    fs::read(dir.join(REJECTED)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub(super) fn is_rejected(dir: &Path, sha: &str) -> bool {
    rejected(dir).iter().any(|r| same_sha(r, sha))
}

fn reject(dir: &Path, sha: &str) {
    let mut list = rejected(dir);
    if list.iter().any(|r| same_sha(r, sha)) {
        return;
    }
    list.push(sha.to_ascii_lowercase());
    let excess = list.len().saturating_sub(MAX_REJECTED);
    list.drain(..excess);
    let result = serde_json::to_vec(&list)
        .map_err(|e| e.to_string())
        .and_then(|json| commit_selector(dir, REJECTED, &json));
    if let Err(e) = result {
        crate::diag::report!("content bundle rejection not saved: {e}");
    }
}

/// A bundle that downloaded whole but failed this build's checks: content
/// made for a newer app, or broken. Unlike a rejection it is tied to the
/// app build that couldn't read it, so an app update tries it again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Unreadable {
    sha256: String,
    /// `version+build` of the app that refused it.
    app: String,
}

fn app_key(info: &BuildInfo) -> String {
    format!("{}+{}", info.version, info.build)
}

fn unreadable(dir: &Path) -> Vec<Unreadable> {
    fs::read(dir.join(UNREADABLE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn is_unreadable(dir: &Path, info: &BuildInfo, sha: &str) -> bool {
    let app = app_key(info);
    unreadable(dir).iter().any(|u| u.app == app && same_sha(&u.sha256, sha))
}

fn mark_unreadable(dir: &Path, info: &BuildInfo, sha: &str) {
    let app = app_key(info);
    // Entries from earlier app builds no longer apply.
    let mut list: Vec<_> = unreadable(dir).into_iter().filter(|u| u.app == app).collect();
    if list.iter().any(|u| same_sha(&u.sha256, sha)) {
        return;
    }
    list.push(Unreadable { sha256: sha.to_ascii_lowercase(), app });
    let excess = list.len().saturating_sub(MAX_REJECTED);
    list.drain(..excess);
    let result = fs::create_dir_all(dir)
        .map_err(|e| e.to_string())
        .and_then(|()| serde_json::to_vec(&list).map_err(|e| e.to_string()))
        .and_then(|json| commit_selector(dir, UNREADABLE, &json));
    if let Err(e) = result {
        crate::diag::report!("unreadable content bundle not remembered: {e}");
    }
}

/// The feed's bundle when this device already holds exactly those bytes
/// (a feed that changed only the app, or went back to earlier content), and
/// whether they sit at the bundle's own content-addressed path.
fn on_disk(dir: &Path, file: &kabar_protocol::update::ContentFile) -> Option<(Vec<u8>, bool)> {
    let target = bundle_path(dir, &file.sha256)?;
    let legacy = read_meta(dir, INFO).filter(|m| same_sha(&m.sha256, &file.sha256)).map(|_| dir.join(BUNDLE));
    [(target, true)].into_iter().chain(legacy.map(|p| (p, false))).find_map(|(path, own)| {
        let bytes = fs::read(path).ok()?;
        (bytes.len() as u64 == file.size && same_sha(&kabar_core::packs::sha256_hex(&bytes), &file.sha256))
            .then_some((bytes, own))
    })
}

/// Counts this launch against `meta` unless it is the confirmed bundle.
fn count_start(dir: &Path, meta: &BundleInfo) {
    let good = read_meta(dir, GOOD_INFO);
    if good.is_some_and(|g| same_sha(&g.sha256, &meta.sha256)) {
        if starts_of(dir, &meta.sha256) > 0 {
            let _ = fs::remove_file(dir.join(STARTS));
        }
        return;
    }
    let next = Starts {
        sha256: meta.sha256.to_ascii_lowercase(),
        count: starts_of(dir, &meta.sha256).saturating_add(1),
    };
    let result = serde_json::to_vec(&next)
        .map_err(|e| e.to_string())
        .and_then(|json| commit_selector(dir, STARTS, &json));
    if let Err(e) = result {
        crate::diag::report!("content start count not saved: {e}");
    }
}

/// Makes `meta` (the bundle this process activated and ran on) the
/// last-known-good one. A legacy `content.bundle` gets its own
/// content-addressed copy first, so the good selector never depends on a
/// file a later launch might remove.
pub(super) fn confirm_stable(data: &Path, meta: &BundleInfo) -> Result<(), String> {
    let dir = data.join(DIR);
    if is_rejected(&dir, &meta.sha256) {
        return Ok(());
    }
    let target = bundle_path(&dir, &meta.sha256).ok_or("invalid bundle digest")?;
    let intact = |path: &Path| {
        fs::read(path).is_ok_and(|b| same_sha(&kabar_core::packs::sha256_hex(&b), &meta.sha256))
    };
    let already = read_meta(&dir, GOOD_INFO).as_ref() == Some(meta) && intact(&target);
    if !already {
        if !intact(&target) {
            let bytes = fs::read(selected_path(&dir, meta)).map_err(|e| e.to_string())?;
            if !same_sha(&kabar_core::packs::sha256_hex(&bytes), &meta.sha256) {
                return Err("the bundle in use changed on disk".into());
            }
            write_atomic(&dir, GOOD_PART, &target, &bytes)?;
        }
        // The pointer moves last: until then the old good bundle stays
        // referenced, and cleanup keeps it.
        let json = serde_json::to_vec_pretty(meta).map_err(|e| e.to_string())?;
        commit_selector(&dir, GOOD_INFO, &json)?;
    }
    if starts_of(&dir, &meta.sha256) > 0 {
        let _ = fs::remove_file(dir.join(STARTS));
    }
    Ok(())
}

/// Reclaim only unreferenced SHA-addressed bundles and interrupted writes.
/// A missing or unreadable selector disables deletion: its bundle might still
/// be recoverable. Never touch legacy content.bundle or media pack storage.
fn cleanup_unreferenced(dir: &Path) {
    let read = |name: &str| -> Option<Option<BundleInfo>> {
        match fs::read(dir.join(name)) {
            Ok(bytes) => serde_json::from_slice(&bytes).ok().map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(None),
            Err(_) => None,
        }
    };
    let (Some(current), Some(previous), Some(good)) = (read(INFO), read(BACKUP_INFO), read(GOOD_INFO)) else {
        return;
    };
    // A syntactically valid selector may still have an invalid digest. Fail
    // closed rather than deleting a possible recovery candidate.
    let mut retained = Vec::new();
    for meta in [current, previous, good].into_iter().flatten() {
        let Some(path) = bundle_path(dir, &meta.sha256) else { return };
        retained.push(path);
    }
    let Ok(files) = fs::read_dir(dir) else { return };
    for item in files.flatten() {
        if !item.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let path = item.path();
        let Some(name) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        let old_part = matches!(
            name,
            "content.bundle.part"
                | "content.json.part"
                | "content.previous.json.part"
                | "content.good.json.part"
                | "content.starts.json.part"
                | "content.rejected.json.part"
                | "content.unreadable.json.part"
                | GOOD_PART
        );
        let orphan = name
            .strip_prefix("content.")
            .and_then(|s| s.strip_suffix(".bundle"))
            .and_then(|sha| bundle_path(dir, sha))
            .is_some_and(|candidate| candidate == path && !retained.contains(&candidate));
        if old_part || orphan {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
pub(super) fn load(data: &Path, info: &BuildInfo) -> Option<(Entries, bool)> {
    activate(data, info).map(|(_, entries, look)| (entries, look))
}

/// Picks the bundle this launch reads: the selected one, unless it is
/// damaged or kept Kabar from starting; then the last-known-good one; then
/// the previous selection; else none (the built-in content). Counts the
/// launch against the bundle it picks.
fn activate(data: &Path, info: &BuildInfo) -> Option<(BundleInfo, Entries, bool)> {
    let dir = data.join(DIR);
    if let Some(current) = read_meta(&dir, INFO)
        && !read_meta(&dir, GOOD_INFO).is_some_and(|g| same_sha(&g.sha256, &current.sha256))
        && starts_of(&dir, &current.sha256) >= MAX_UNCONFIRMED_STARTS
    {
        crate::diag::report!(
            "content bundle {} didn't let Kabar start {MAX_UNCONFIRMED_STARTS} times; rejected",
            current.build
        );
        reject(&dir, &current.sha256);
        let _ = fs::remove_file(dir.join(STARTS));
    }
    for selector in [INFO, GOOD_INFO, BACKUP_INFO] {
        let Some((meta, entries, look)) = validated(&dir, info, selector) else { continue };
        if selector != INFO {
            // Restore only an independently verified selection.
            match serde_json::to_vec_pretty(&meta) {
                Ok(json) => {
                    if let Err(e) = commit_selector(&dir, INFO, &json) {
                        crate::diag::report!("content selector recovery failed: {e}");
                    }
                }
                Err(e) => crate::diag::report!("content selector recovery failed: {e}"),
            }
            crate::diag::report!("recovered content bundle {} from {selector}", meta.build);
        }
        count_start(&dir, &meta);
        cleanup_unreferenced(&dir);
        return Some((meta, entries, look));
    }
    if in_use(data, info).is_none() {
        // Preserve the existing stale-content behavior for app upgrades.
        let _ = fs::remove_file(dir.join(BUNDLE));
        let _ = fs::remove_file(dir.join(INFO));
    } else {
        crate::diag::report!("content bundle unavailable; using the built-in content");
    }
    None
}

/// Whether a feed bundle with digest `sha` is exactly the content this
/// app has built in: CI builds the bundle before the APK and bakes its
/// digest in as `KABAR_BUILTIN_CONTENT`. Only while no downloaded bundle is
/// in use, so a feed going back to this content still replaces a newer one.
fn built_in(builtin: Option<&str>, bundle_in_use: bool, sha: &str) -> bool {
    !bundle_in_use && builtin.is_some_and(|b| same_sha(b.trim(), sha))
}

/// Fetches the feed's bundle and checks it against the signed manifest row.
fn download(base: &str, file: &kabar_protocol::update::ContentFile) -> Result<Vec<u8>, String> {
    let mut res = agent(Duration::from_secs(10 * 60))
        .get(&format!("{base}/{}", file.file))
        .call()
        .map_err(|e| e.to_string())?;
    if res.status().as_u16() != 200 {
        return Err(format!("HTTP {}", res.status()));
    }
    let mut bytes = Vec::new();
    res.body_mut().as_reader().take(file.size + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 != file.size || !same_sha(&kabar_core::packs::sha256_hex(&bytes), &file.sha256) {
        return Err("doesn't match the signed manifest".into());
    }
    Ok(bytes)
}

/// Downloads the feed's bundle when it is newer than the one in use, checks
/// it, and keeps it for the next launch. True when one was taken. Failures
/// are only reported: the content in use keeps working. Bytes the device
/// already holds aren't downloaded again (a feed that changed only the app
/// carries the same bundle under a new build), and a bundle this app build
/// couldn't read isn't fetched again until the app changes.
pub(super) fn refresh(
    data: &Path,
    me: &Installed,
    info: &BuildInfo,
    m: &UpdateManifest,
    base: &str,
) -> Refresh {
    let current = in_use(data, info).and_then(|i| i.order());
    let file = match content_update(me, m, current) {
        Ok(Some(f)) if is_rejected(&data.join(DIR), &f.sha256) => {
            // It kept Kabar from starting here; wait for a newer one.
            crate::diag::report!("content bundle {} was rejected on this device; skipped", f.file);
            return Refresh::Current;
        }
        Ok(Some(f)) if is_unreadable(&data.join(DIR), info, &f.sha256) => {
            // This app build couldn't read it; an app update tries again.
            return Refresh::Current;
        }
        Ok(Some(f)) => f.clone(),
        Ok(None) => return Refresh::Current,
        Err(e) => {
            crate::diag::report!("content bundle: {e}");
            return Refresh::Failed;
        }
    };
    // The app runs on its built-in content and the feed's bundle is that
    // same content (the build that made this app): nothing to download.
    if built_in(option_env!("KABAR_BUILTIN_CONTENT"), in_use(data, info).is_some(), &file.sha256) {
        return Refresh::Current;
    }
    let dir = data.join(DIR);
    // The same bytes as the bundle in use: only the build number moves.
    let unchanged = in_use(data, info).is_some_and(|i| same_sha(&i.sha256, &file.sha256));
    // Err((true, _)): the file itself can't be used by this build, and is
    // remembered; Err((false, _)): try again next time.
    let result = (|| -> Result<(), (bool, String)> {
        let net = |e: String| (false, e);
        if file.size > kabar_protocol::bundle::MAX_BYTES {
            return Err(net(format!("{} bytes is too large", file.size)));
        }
        let held = on_disk(&dir, &file);
        let own_copy = held.as_ref().is_some_and(|(_, own)| *own);
        let bytes = match held {
            Some((bytes, _)) => bytes,
            None => download(base, &file).map_err(net)?,
        };
        // Every file must open within the size limits and pass the same
        // checks `kabar-tools update bundle` runs before publishing, or
        // none is used.
        let bad = |e: String| (true, e);
        let entries = kabar_protocol::bundle::decode(&bytes).map_err(|e| bad(e.to_string()))?;
        let files = unpack_all(&entries).map_err(bad)?;
        let issues = kabar_core::bundle_check::check_files(&files);
        if let Some(first) = issues.first() {
            return Err(bad(format!("{} content problem(s), first {first}", issues.len())));
        }
        drop(files);
        fs::create_dir_all(&dir).map_err(|e| net(e.to_string()))?;
        let target = bundle_path(&dir, &file.sha256).ok_or_else(|| net("invalid bundle digest".into()))?;
        if !own_copy {
            write_atomic(&dir, &format!("{BUNDLE}.part"), &target, &bytes).map_err(net)?;
        }
        let next = BundleInfo {
            version: m.version.clone(),
            build: m.build,
            sha256: file.sha256.clone(),
            ui_build: file.ui_build,
            fetched_ms: Some(super::now_ms()),
        };
        let json = serde_json::to_vec_pretty(&next).map_err(|e| net(e.to_string()))?;
        // The previous selection must be recoverable before changing the
        // active pointer. A corrupt active selection never replaces a good backup.
        if !unchanged && validated(&dir, info, INFO).is_some() {
            let old = fs::read(dir.join(INFO)).map_err(|e| net(e.to_string()))?;
            commit_selector(&dir, BACKUP_INFO, &old).map_err(net)?;
        }
        // Commit the active pointer only after its bundle and fallback are safe.
        commit_selector(&dir, INFO, &json).map_err(net)
    })();
    match result {
        Ok(()) if unchanged => Refresh::Current,
        Ok(()) => Refresh::Taken(m.build),
        Err((bad, e)) => {
            crate::diag::report!("content bundle {} not taken: {e}", file.file);
            if bad {
                mark_unreadable(&dir, info, &file.sha256);
            }
            Refresh::Failed
        }
    }
}

#[cfg(test)]
mod built_in_tests {
    use super::built_in;

    #[test]
    fn the_feed_bundle_of_the_running_build_is_not_downloaded() {
        let sha = "ab".repeat(32);
        assert!(built_in(Some(&sha), false, &sha.to_uppercase()), "the same content");
        assert!(
            !built_in(Some(&sha), true, &sha),
            "a downloaded bundle is in use: the feed may roll it back"
        );
        assert!(!built_in(Some(&"cd".repeat(32)), false, &sha), "other content");
        assert!(!built_in(None, false, &sha), "a local build doesn't know its digest");
    }
}
