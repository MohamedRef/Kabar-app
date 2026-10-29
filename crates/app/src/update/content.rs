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

pub(super) fn load(data: &Path, info: &BuildInfo) -> Option<(Entries, bool)> {
    let dir = data.join(DIR);
    let Some(meta) = in_use(data, info) else {
        let _ = fs::remove_file(dir.join(BUNDLE));
        let _ = fs::remove_file(dir.join(INFO));
        return None;
    };
    let bytes = fs::read(dir.join(BUNDLE)).ok()?;
    if !kabar_core::packs::sha256_hex(&bytes).eq_ignore_ascii_case(&meta.sha256) {
        crate::diag::report!("content bundle changed on disk; using the built-in content");
        return None;
    }
    let entries = kabar_protocol::bundle::decode(&bytes)
        .map_err(|e| crate::diag::report!("content bundle doesn't open ({e}); using the built-in content"))
        .ok()?;
    let look = meta.order() == Some((info.version, meta.ui_build));
    Some((entries, look))
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
        let part = dir.join(format!("{BUNDLE}.part"));
        fs::write(&part, &bytes).map_err(|e| e.to_string())?;
        fs::rename(&part, dir.join(BUNDLE)).map_err(|e| e.to_string())?;
        let info = BundleInfo {
            version: m.version.clone(),
            build: m.build,
            sha256: file.sha256.clone(),
            ui_build: file.ui_build,
            fetched_ms: Some(super::now_ms()),
        };
        let json = serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?;
        fs::write(dir.join(INFO), json).map_err(|e| e.to_string())
    })();
    match result {
        Ok(()) => Refresh::Taken(m.build),
        Err(e) => {
            crate::diag::report!("content bundle {} not taken: {e}", file.file);
            Refresh::Failed
        }
    }
}
