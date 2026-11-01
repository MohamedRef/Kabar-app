//! Content bundles from the update feed (`docs/RELEASE.md` §5): lessons,
//! tests, stories, Radio, glosses, the Bridge, curricula and the
//! stylesheets, as one signed file (`kabar_protocol::bundle`). A check
//! downloads a newer one in the background; at the next launch the app reads
//! its content from it instead of the copy built in (`crate::embedded`), so a
//! content change needs no app install.
//!
//! ```text
//! <data>/content/content.bundle  the bundle in use from the next launch
//! <data>/content/content.json    where it came from (BundleInfo)
//! ```
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

/// Reads content from the downloaded bundle from now on, when there is one
/// for this build; a stale one (the app was updated since) is removed.
/// Called at launch, before anything reads content.
pub fn use_installed(data: &Path) {
    let info = BuildInfo::from_env();
    if let Some((entries, look)) = load(data, &info) {
        crate::embedded::use_bundle(entries, look);
    }
}

type Entries = Vec<(String, Vec<u8>)>;

/// Verify both the selected file and the matching metadata before activating it.
fn validated(dir: &Path, info: &BuildInfo, selector: &str) -> Option<(Entries, bool)> {
    let meta: BundleInfo = serde_json::from_slice(&fs::read(dir.join(selector)).ok()?).ok()?;
    if meta.order()? < (info.version, info.build) {
        return None;
    }
    let bytes = fs::read(selected_path(dir, &meta)).ok()?;
    if !kabar_core::packs::sha256_hex(&bytes).eq_ignore_ascii_case(&meta.sha256) {
        return None;
    }
    let entries = kabar_protocol::bundle::decode(&bytes).ok()?;
    let look = meta.order() == Some((info.version, meta.ui_build));
    Some((entries, look))
}

/// Write selectors through a temporary file, never exposing partial JSON.
fn commit_selector(dir: &Path, selector: &str, json: &[u8]) -> Result<(), String> {
    let temp = dir.join(format!("{selector}.part"));
    fs::write(&temp, json).map_err(|e| e.to_string())?;
    fs::rename(&temp, dir.join(selector)).map_err(|e| e.to_string())
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
    let (Some(current), Some(previous)) = (read(INFO), read(BACKUP_INFO)) else {
        return;
    };
    // A syntactically valid selector may still have an invalid digest. Fail
    // closed rather than deleting a possible recovery candidate.
    let mut retained = Vec::new();
    for meta in [current, previous].into_iter().flatten() {
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
        let old_part =
            matches!(name, "content.bundle.part" | "content.json.part" | "content.previous.json.part");
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

pub(super) fn load(data: &Path, info: &BuildInfo) -> Option<(Entries, bool)> {
    let dir = data.join(DIR);
    if let Some(bundle) = validated(&dir, info, INFO) {
        cleanup_unreferenced(&dir);
        return Some(bundle);
    }
    if let Some(bundle) = validated(&dir, info, BACKUP_INFO) {
        // Restore only an independently verified previous selection.
        if let Ok(previous) = fs::read(dir.join(BACKUP_INFO))
            && let Err(e) = commit_selector(&dir, INFO, &previous)
        {
            crate::diag::report!("content selector recovery failed: {e}");
        }
        crate::diag::report!("recovered the previous verified content bundle");
        cleanup_unreferenced(&dir);
        return Some(bundle);
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

/// Downloads the feed's bundle when it is newer than the one in use, checks
/// it, and keeps it for the next launch. True when one was taken. Failures
/// are only reported: the content in use keeps working.
pub(super) fn refresh(
    data: &Path,
    me: &Installed,
    info: &BuildInfo,
    m: &UpdateManifest,
    base: &str,
) -> Refresh {
    let current = in_use(data, info).and_then(|i| i.order());
    let file = match content_update(me, m, current) {
        Ok(Some(f)) => f.clone(),
        Ok(None) => return Refresh::Current,
        Err(e) => {
            crate::diag::report!("content bundle: {e}");
            return Refresh::Failed;
        }
    };
    let result = (|| -> Result<(), String> {
        if file.size > kabar_protocol::bundle::MAX_BYTES {
            return Err(format!("{} bytes is too large", file.size));
        }
        let mut res = agent(Duration::from_secs(10 * 60))
            .get(&format!("{base}/{}", file.file))
            .call()
            .map_err(|e| e.to_string())?;
        if res.status().as_u16() != 200 {
            return Err(format!("HTTP {}", res.status()));
        }
        let mut bytes = Vec::new();
        res.body_mut().as_reader().take(file.size + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() as u64 != file.size
            || !kabar_core::packs::sha256_hex(&bytes).eq_ignore_ascii_case(&file.sha256)
        {
            return Err("doesn't match the signed manifest".into());
        }
        // Every file must open, or none is used.
        for (key, packed) in kabar_protocol::bundle::decode(&bytes).map_err(|e| e.to_string())? {
            let text = miniz_oxide::inflate::decompress_to_vec(&packed)
                .map_err(|_| format!("{key} doesn't unpack"))?;
            std::str::from_utf8(&text).map_err(|_| format!("{key} isn't UTF-8"))?;
        }
        let dir = data.join(DIR);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let target = bundle_path(&dir, &file.sha256).ok_or("invalid bundle digest")?;
        let part = dir.join(format!("{BUNDLE}.part"));
        fs::write(&part, &bytes).map_err(|e| e.to_string())?;
        fs::rename(&part, &target).map_err(|e| e.to_string())?;
        let next = BundleInfo {
            version: m.version.clone(),
            build: m.build,
            sha256: file.sha256.clone(),
            ui_build: file.ui_build,
            fetched_ms: Some(super::now_ms()),
        };
        let json = serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?;
        // The previous selection must be recoverable before changing the
        // active pointer. A corrupt active selection never replaces a good backup.
        if validated(&dir, info, INFO).is_some() {
            let old = fs::read(dir.join(INFO)).map_err(|e| e.to_string())?;
            commit_selector(&dir, BACKUP_INFO, &old)?;
        }
        // Commit the active pointer only after its bundle and fallback are safe.
        commit_selector(&dir, INFO, &json)
    })();
    match result {
        Ok(()) => Refresh::Taken(m.build),
        Err(e) => {
            crate::diag::report!("content bundle {} not taken: {e}", file.file);
            Refresh::Failed
        }
    }
}
