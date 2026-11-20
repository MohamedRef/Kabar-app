//! Offline packs on this device (SPEC §7, §20; `docs/RUN.md` §13): which units'
//! audio is downloaded, downloading a unit's pack from the GitHub release,
//! checking every byte against its SHA-256, and finding the pre-generated
//! clip for a line the Lesson Player is about to say.
//!
//! Device-local, never synced (kept apart from `<data>/audio/`, which holds
//! Kabar Cloud's neural-voice cache and downloaded Radio episodes):
//!
//! ```text
//! <data>/packs/audio/<key>.mp3            clips, named by their content address
//! <data>/packs/downloads/<unit>.tar.part  an interrupted pack download (resumable)
//! <data>/packs/<unit>.json                manifests of the installed packs
//! <data>/packs/index.json                 the newest release index seen
//! <data>/packs/prefs.json                 auto-download on/off
//! ```
//!
//! The lesson text itself is still built into the app (`build.rs`).

mod fetch;
pub mod settings;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use base64::Engine as _;
use kabar_core::packs::{self, audio_key, check_manifest, default_voice, is_hex_key, is_unit_id, sha256_hex};
use kabar_protocol::cloud::Voice;
use kabar_protocol::pack::{AUDIO_EXT, AUDIO_MIME, PACK_FORMAT, PackIndex, PackManifest, PackSummary};
use serde::{Deserialize, Serialize};

use self::fetch::{TarFetch, fetch_tar, get_index};

/// The index this build was made with (`content/packs/index.json`): lists
/// every unit even offline, before any release index was fetched.
const EMBEDDED_INDEX: &str = include_str!("../../../../content/packs/index.json");

/// Largest pack tar accepted (a unit is about 1–5 MB).
#[cfg(not(target_arch = "wasm32"))]
pub(super) const MAX_PACK_BYTES: u64 = 64 * 1024 * 1024;

static STORE: OnceLock<PackStore> = OnceLock::new();
/// Packs being downloaded right now (so a prefetch and a tap don't race).
static IN_FLIGHT: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Opens the pack store under the app's data directory, moves any pack clips
/// left in `<data>/audio/` by an older build into `<data>/packs/audio/`, and
/// sweeps unowned clips and temp files. Called once at start.
pub fn init(data_dir: &Path) {
    let _ = STORE.set(PackStore::open(data_dir));
}

pub fn store() -> Option<&'static PackStore> {
    STORE.get()
}

/// The `packs` release of the Kabar repository (GitHub Releases: free, and
/// public repositories need no token to download).
pub const DEFAULT_PACKS_URL: &str = "https://github.com/MohamedRef/Kabar-app/releases/download/packs";
pub const NOT_CONFIGURED: &str = "no KABAR_PACKS_URL in this build";
pub const TOO_NEW: &str = "the packs need a newer Kabar";
pub(crate) const ALREADY_DOWNLOADING: &str = "already downloading";

/// Where the packs are published: `KABAR_PACKS_URL` at run time or build
/// time (`.env`), else [`DEFAULT_PACKS_URL`]. An empty or non-HTTPS value
/// turns downloads off (debug builds also accept loopback HTTP for tests).
pub fn base_url() -> Option<String> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let url = std::env::var("KABAR_PACKS_URL")
        .ok()
        .or(option_env!("KABAR_PACKS_URL").map(String::from))
        .unwrap_or_else(|| DEFAULT_PACKS_URL.to_owned());
    let url = url.trim().trim_end_matches('/');
    is_allowed_packs_url(url).then(|| url.to_owned())
}

pub fn is_allowed_packs_url(url: &str) -> bool {
    url.starts_with("https://")
        || (cfg!(debug_assertions)
            && (url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")))
}

#[derive(Default, Serialize, Deserialize)]
struct Prefs {
    /// Fetch the current and next unit when a lesson opens.
    #[serde(default = "yes")]
    auto: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug)]
pub struct PackStore {
    audio: PathBuf,
    packs: PathBuf,
    /// Serializes `install`, `remove` and orphan sweeps so a `remove` can
    /// never delete clips of an `install` whose manifest isn't written yet.
    ops: Mutex<()>,
}

impl PartialEq for PackStore {
    fn eq(&self, other: &Self) -> bool {
        self.audio == other.audio && self.packs == other.packs
    }
}

impl Eq for PackStore {}

impl PackStore {
    pub fn new(data_dir: &Path) -> Self {
        let packs = data_dir.join("packs");
        Self { audio: packs.join("audio"), packs, ops: Mutex::new(()) }
    }

    /// Opens the store at start-up, migrating pack clips from `<data>/audio/`
    /// (older builds shared that folder with `AudioCache`) and sweeping stray
    /// `.part` files and unowned pack clips.
    pub fn open(data_dir: &Path) -> Self {
        let store = Self::new(data_dir);
        store.migrate_legacy_audio(&data_dir.join("audio"));
        store.sweep();
        store
    }

    fn lock_ops(&self) -> std::sync::MutexGuard<'_, ()> {
        self.ops.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn clip_path(&self, key: &str) -> PathBuf {
        self.audio.join(format!("{key}.{AUDIO_EXT}"))
    }

    fn manifest_path(&self, unit: &str) -> PathBuf {
        self.packs.join(format!("{unit}.json"))
    }

    pub(super) fn downloads_dir(&self) -> PathBuf {
        self.packs.join("downloads")
    }

    pub(super) fn part_paths(&self, unit: &str) -> (PathBuf, PathBuf) {
        let dir = self.downloads_dir();
        (dir.join(format!("{unit}.tar.part")), dir.join(format!("{unit}.tar.part.json")))
    }

    /// Moves only the clips listed by installed pack manifests from
    /// `<data>/audio/` into `<data>/packs/audio/`, leaving every cloud-voice
    /// and Radio file in `<data>/audio/` untouched.
    fn migrate_legacy_audio(&self, legacy_audio: &Path) {
        let _guard = self.lock_ops();
        let installed = self.installed_unlocked();
        if installed.is_empty() || !legacy_audio.is_dir() {
            return;
        }
        let keys: BTreeSet<&str> = installed
            .iter()
            .flat_map(|m| m.audio.iter().map(|a| a.key.as_str()))
            .filter(|k| is_hex_key(k))
            .collect();
        for key in keys {
            let dst = self.clip_path(key);
            let src = legacy_audio.join(format!("{key}.{AUDIO_EXT}"));
            if dst.is_file() || !src.is_file() {
                continue;
            }
            if fs::create_dir_all(&self.audio).is_err() {
                return;
            }
            if fs::rename(&src, &dst).is_err()
                && let Ok(bytes) = fs::read(&src)
                && write_atomic(&dst, &bytes).is_ok()
            {
                let _ = fs::remove_file(&src);
            }
        }
    }

    /// The pre-generated clip for `text` in `voice`, if a downloaded pack has it.
    pub fn clip(&self, voice: Voice, text: &str) -> Option<PathBuf> {
        let path = self.clip_path(&audio_key(voice, text));
        path.is_file().then_some(path)
    }

    /// Manifests of the installed packs.
    pub fn installed(&self) -> Vec<PackManifest> {
        let _guard = self.lock_ops();
        self.installed_unlocked()
    }

    fn installed_unlocked(&self) -> Vec<PackManifest> {
        let Ok(entries) = fs::read_dir(&self.packs) else { return Vec::new() };
        let mut out: Vec<PackManifest> = entries
            .flatten()
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.ends_with(".json") && name != "index.json" && name != "prefs.json"
            })
            .filter_map(|e| serde_json::from_slice::<PackManifest>(&fs::read(e.path()).ok()?).ok())
            .filter(|m| is_unit_id(&m.unit))
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Pack id → installed version.
    pub fn installed_versions(&self) -> BTreeMap<String, u32> {
        self.installed().into_iter().map(|m| (m.id, m.version)).collect()
    }

    /// Bytes of all cached pack clips (`.mp3` files in `<data>/packs/audio/`).
    pub fn storage_bytes(&self) -> u64 {
        let Ok(entries) = fs::read_dir(&self.audio) else { return 0 };
        entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == AUDIO_EXT))
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum()
    }

    fn cached_keys(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.audio) else { return Vec::new() };
        entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                Some(name.strip_suffix(&format!(".{AUDIO_EXT}"))?.to_owned())
            })
            .collect()
    }

    /// Deletes unowned clips in `<data>/packs/audio/` and leftover `.part`
    /// temp files from interrupted writes.
    pub fn sweep(&self) {
        let _guard = self.lock_ops();
        self.sweep_unlocked(&self.installed_unlocked());
    }

    fn sweep_unlocked(&self, keep: &[PackManifest]) {
        for dir in [&self.audio, &self.packs] {
            if let Ok(entries) = fs::read_dir(dir) {
                for e in entries.flatten() {
                    if e.file_name().to_string_lossy().ends_with(".part") {
                        let _ = fs::remove_file(e.path());
                    }
                }
            }
        }
        let cached = self.cached_keys();
        for key in packs::orphans(cached.iter().map(String::as_str), keep) {
            let _ = fs::remove_file(self.clip_path(&key));
        }
    }

    pub fn auto(&self) -> bool {
        fs::read(self.packs.join("prefs.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Prefs>(&b).ok())
            .is_none_or(|p| p.auto)
    }

    pub fn set_auto(&self, auto: bool) -> Result<(), String> {
        fs::create_dir_all(&self.packs).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec(&Prefs { auto }).map_err(|e| e.to_string())?;
        write_atomic(&self.packs.join("prefs.json"), &json)
    }

    /// The newest index: the last one fetched from the release, else the
    /// one built into the app.
    pub fn index(&self) -> PackIndex {
        let fetched = fs::read(self.packs.join("index.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<PackIndex>(&b).ok())
            .filter(|i| i.format <= PACK_FORMAT && !i.packs.is_empty());
        fetched.unwrap_or_else(embedded_index)
    }

    fn save_index(&self, index: &PackIndex) -> Result<(), String> {
        fs::create_dir_all(&self.packs).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?;
        write_atomic(&self.packs.join("index.json"), &json)
    }

    /// Checks a downloaded tar against its index row, every clip against its
    /// manifest hash, and only then installs it and sweeps replaced clips.
    pub fn install(&self, expected: &PackSummary, tar: &[u8]) -> Result<PackManifest, String> {
        if !is_unit_id(&expected.unit) {
            return Err("bad unit id".into());
        }
        if expected.file_sha256.as_deref() != Some(sha256_hex(tar).as_str()) {
            return Err("the download doesn't match the index (hash)".into());
        }
        let mut archive = tar::Archive::new(tar);
        let mut manifest: Option<PackManifest> = None;
        let mut clips: Vec<(String, Vec<u8>)> = Vec::new();
        for entry in archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let name = entry.path().map_err(|e| e.to_string())?.to_string_lossy().into_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            if name == "manifest.json" {
                let m: PackManifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                check_manifest(&m, expected)?;
                manifest = Some(m);
            } else if let Some(key) =
                name.strip_prefix("audio/").and_then(|n| n.strip_suffix(&format!(".{AUDIO_EXT}")))
            {
                clips.push((key.to_owned(), bytes));
            }
        }
        let manifest = manifest.ok_or("the pack has no manifest")?;
        let wanted: BTreeMap<&str, &str> =
            manifest.audio.iter().filter_map(|a| Some((a.key.as_str(), a.sha256.as_deref()?))).collect();
        let mut got = BTreeSet::new();
        for (key, bytes) in &clips {
            match wanted.get(key.as_str()) {
                Some(sha) if *sha == sha256_hex(bytes) => {
                    got.insert(key.as_str());
                }
                Some(_) => return Err(format!("clip {key} is damaged")),
                None => return Err(format!("clip {key} isn't in the manifest")),
            }
        }
        let _guard = self.lock_ops();
        if let Some(missing) = wanted.keys().find(|k| !got.contains(*k) && !self.clip_path(k).is_file()) {
            return Err(format!("clip {missing} is missing"));
        }
        fs::create_dir_all(&self.audio).map_err(|e| e.to_string())?;
        for (key, bytes) in &clips {
            write_atomic(&self.clip_path(key), bytes)?;
        }
        fs::create_dir_all(&self.packs).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
        write_atomic(&self.manifest_path(&manifest.unit), &json)?;
        let (part, meta) = self.part_paths(&manifest.unit);
        let _ = fs::remove_file(part);
        let _ = fs::remove_file(meta);
        self.sweep_unlocked(&self.installed_unlocked());
        Ok(manifest)
    }

    /// Removes a pack, and every clip no other installed pack shares.
    pub fn remove(&self, unit: &str) -> Result<(), String> {
        if !is_unit_id(unit) {
            return Err("bad unit id".into());
        }
        let _guard = self.lock_ops();
        match fs::remove_file(self.manifest_path(unit)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let (part, meta) = self.part_paths(unit);
        let _ = fs::remove_file(part);
        let _ = fs::remove_file(meta);
        self.sweep_unlocked(&self.installed_unlocked());
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn process_tag() -> u32 {
    std::process::id()
}

#[cfg(target_arch = "wasm32")]
fn process_tag() -> u32 {
    0
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|f| f.to_string_lossy()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.{n}.part", process_tag()));
    fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{}: {e}", path.display()));
    }
    Ok(())
}

pub fn embedded_index() -> PackIndex {
    serde_json::from_str(EMBEDDED_INDEX).unwrap_or_default()
}

/// Fetches the `index.json` of the release at `base` and remembers it.
fn refresh_index_from(store: &PackStore, base: &str) -> Result<PackIndex, String> {
    let index: PackIndex = serde_json::from_slice(&get_index(&format!("{base}/index.json"))?)
        .map_err(|e| format!("index: {e}"))?;
    if index.format > PACK_FORMAT {
        return Err(TOO_NEW.into());
    }
    store.save_index(&index)?;
    Ok(index)
}

/// Downloads and installs one pack (by id, from the newest index) from the
/// release at `base` (`None`: not configured). If a re-uploaded `<unit>.tar`
/// mismatches the saved index, refreshes `index.json` once and retries.
fn download_from(store: &PackStore, base: Option<&str>, id: &str) -> Result<PackManifest, String> {
    {
        let mut busy = IN_FLIGHT.lock().map_err(|e| e.to_string())?;
        if !busy.insert(id.to_owned()) {
            return Err(ALREADY_DOWNLOADING.into());
        }
    }
    let result = download_locked(store, base, id);
    if let Ok(mut busy) = IN_FLIGHT.lock() {
        busy.remove(id);
    }
    if result.is_ok()
        && let Ok(mut jobs) = JOBS.lock()
    {
        jobs.retain(|(j, _)| j != id);
    }
    result
}

fn download_locked(store: &PackStore, base: Option<&str>, id: &str) -> Result<PackManifest, String> {
    let base = base.ok_or(NOT_CONFIGURED)?;
    let index = store.index();
    let row = index.get(id).ok_or("no such pack")?;
    if !row.downloadable() {
        return Err("this unit's audio isn't published yet".to_owned());
    }
    let first = match fetch_tar(store, &format!("{base}/{}", row.file), row)? {
        TarFetch::Verified(tar) => match store.install(row, &tar) {
            Ok(m) => return Ok(m),
            Err(e) if e.contains("manifest") => (tar, e),
            Err(e) => return Err(e),
        },
        TarFetch::HashMismatch(tar) => (tar, "the download doesn't match the index (hash)".into()),
    };
    let (tar, first_err) = first;
    let Ok(fresh_index) = refresh_index_from(store, base) else { return Err(first_err) };
    let Some(fresh) = fresh_index.get(id).filter(|f| f.downloadable() && *f != row) else {
        return Err(first_err);
    };
    if fresh.file_sha256.as_deref() == Some(sha256_hex(&tar).as_str()) {
        return store.install(fresh, &tar);
    }
    match fetch_tar(store, &format!("{base}/{}", fresh.file), fresh)? {
        TarFetch::Verified(tar2) => store.install(fresh, &tar2),
        TarFetch::HashMismatch(_) => Err("the download doesn't match the index (hash)".into()),
    }
}

/// Where a pack the learner asked for stands. Downloads the learner starts
/// run one at a time, in the order asked, so a slow connection finishes one
/// unit before starting the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Job {
    Queued,
    Downloading,
    /// The download or the check failed; the row offers to try again.
    Failed,
}

/// Packs asked for and not yet installed, in the order asked. A finished
/// download leaves the list; a failed one stays until retried or installed.
static JOBS: Mutex<Vec<(String, Job)>> = Mutex::new(Vec::new());
static WORKER: AtomicBool = AtomicBool::new(false);

/// Every pack the learner asked for that isn't installed yet.
pub fn jobs() -> BTreeMap<String, Job> {
    let mut out: BTreeMap<String, Job> = JOBS.lock().map(|j| j.iter().cloned().collect()).unwrap_or_default();
    // A prefetch or update running in the background shows too.
    if let Ok(busy) = IN_FLIGHT.lock() {
        for id in busy.iter() {
            out.insert(id.clone(), Job::Downloading);
        }
    }
    out
}

/// [`jobs`] pruned against `store` so a stale `Failed` entry never lingers
/// after a pack was installed at the index's version by another path.
pub fn jobs_for(store: &PackStore) -> BTreeMap<String, Job> {
    let installed = store.installed_versions();
    let index = store.index();
    let is_current = |id: &str| {
        let Some(&have) = installed.get(id) else { return false };
        index.get(id).is_none_or(|row| have >= row.version)
    };
    if let Ok(mut jobs) = JOBS.lock() {
        jobs.retain(|(id, state)| !(*state == Job::Failed && is_current(id)));
    }
    jobs()
}

fn is_in_flight(id: &str) -> bool {
    IN_FLIGHT.lock().is_ok_and(|b| b.contains(id))
}

#[cfg(not(target_arch = "wasm32"))]
fn wait_in_flight(id: &str) {
    while is_in_flight(id) {
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(target_arch = "wasm32")]
fn wait_in_flight(_id: &str) {}

/// Queues a pack for `store` (again, after a failure) and makes sure the
/// queue runs, downloading from the release at `base`.
fn enqueue_in(store: &'static PackStore, base: Option<String>, id: &str) {
    queue(&JOBS, id);
    if !WORKER.swap(true, Ordering::SeqCst) {
        let started = crate::platform::time::spawn("kabar-packs", move || {
            run_queue(&JOBS, |id| {
                if is_in_flight(id) {
                    wait_in_flight(id);
                    let have = store.installed_versions().get(id).copied();
                    if have.is_some_and(|v| store.index().get(id).is_none_or(|r| v >= r.version)) {
                        return Ok(());
                    }
                }
                download_from(store, base.as_deref(), id).map(drop)
            });
            WORKER.store(false, Ordering::SeqCst);
            // Asked for while the worker was finishing: run again.
            if JOBS.lock().is_ok_and(|j| j.iter().any(|(_, s)| *s == Job::Queued)) {
                enqueue_again(store, base);
            }
        });
        if started.is_err() {
            WORKER.store(false, Ordering::SeqCst);
        }
    }
}

fn enqueue_again(store: &'static PackStore, base: Option<String>) {
    let next =
        JOBS.lock().ok().and_then(|j| j.iter().find(|(_, s)| *s == Job::Queued).map(|(id, _)| id.clone()));
    if let Some(id) = next {
        enqueue_in(store, base, &id);
    }
}

fn queue(jobs: &Mutex<Vec<(String, Job)>>, id: &str) {
    let Ok(mut jobs) = jobs.lock() else { return };
    match jobs.iter_mut().find(|(j, _)| j == id) {
        Some((_, state @ Job::Failed)) => *state = Job::Queued,
        Some(_) => {}
        None => jobs.push((id.to_owned(), Job::Queued)),
    }
}

/// Runs queued jobs one by one until none is left.
fn run_queue(jobs: &Mutex<Vec<(String, Job)>>, mut fetch: impl FnMut(&str) -> Result<(), String>) {
    loop {
        let next = {
            let Ok(mut jobs) = jobs.lock() else { return };
            let Some((id, state)) = jobs.iter_mut().find(|(_, s)| *s == Job::Queued) else { return };
            *state = Job::Downloading;
            id.clone()
        };
        let result = fetch(&next);
        let Ok(mut jobs) = jobs.lock() else { return };
        match result {
            Ok(()) => jobs.retain(|(id, _)| *id != next),
            Err(e) if e == ALREADY_DOWNLOADING => jobs.retain(|(id, _)| *id != next),
            Err(e) => {
                crate::diag::report!("pack {next}: {e}");
                if let Some((_, state)) = jobs.iter_mut().find(|(id, _)| *id == next) {
                    *state = Job::Failed;
                }
            }
        }
    }
}

/// Just in time (SPEC §20): when a lesson opens, fetch its unit's pack and
/// the next unit's in the background, if auto-download is on and the
/// connection isn't metered (like app updates). Failures are quiet: the
/// lesson plays with the device's own voice meanwhile.
pub fn prefetch_for(lesson_id: &str) {
    let Some(store) = store() else { return };
    let Some(base) = base_url() else { return };
    if !store.auto() {
        return;
    }
    let lesson_id = lesson_id.to_owned();
    let _ = crate::platform::time::spawn("kabar-packs", move || {
        if crate::platform::metered_network() {
            return;
        }
        prefetch_from(store, &base, &lesson_id);
    });
}

/// The packs of `lesson_id`'s unit and the next one, from `base`, after a
/// fresh look at the index (the one already known when that fails).
fn prefetch_from(store: &PackStore, base: &str, lesson_id: &str) {
    let index = refresh_index_from(store, base).unwrap_or_else(|_| store.index());
    for id in packs::prefetch(&index, lesson_id, &store.installed_versions()) {
        if let Err(e) = download_from(store, Some(base), &id) {
            crate::diag::report!("prefetching {id} failed: {e}");
        }
    }
}

/// Over the air with the app update check: refreshes the index and
/// re-downloads every installed pack that has a newer version, when the
/// connection isn't metered and either auto-download is on or the learner
/// asked (`by_hand`, "Check for updates"). Returns how many packs it
/// refreshed. Blocking; called from the updater's thread.
pub fn update_installed(by_hand: bool) -> usize {
    let Some(store) = store() else { return 0 };
    let Some(base) = base_url() else { return 0 };
    if !(by_hand || store.auto()) || crate::platform::metered_network() {
        return 0;
    }
    update_installed_from(store, &base)
}

/// Re-downloads every installed pack the release at `base` has newer.
fn update_installed_from(store: &PackStore, base: &str) -> usize {
    let Ok(index) = refresh_index_from(store, base) else { return 0 };
    let mut done = 0;
    for id in packs::outdated(&index, &store.installed_versions()) {
        match download_from(store, Some(base), &id) {
            Ok(_) => done += 1,
            Err(e) => crate::diag::report!("updating pack {id} failed: {e}"),
        }
    }
    done
}

/// A `data:` URL of the pre-generated clip for a line, for the WebView's
/// `<audio>` (works the same on the Mac and Android, no file server needed).
/// `voice` is the dialogue speaker's; otherwise the language's narrator.
pub fn clip_url(text: &str, language: &str, voice: Option<Voice>) -> Option<String> {
    clip_url_in(store()?, text, language, voice)
}

fn clip_url_in(store: &PackStore, text: &str, language: &str, voice: Option<Voice>) -> Option<String> {
    let path = store.clip(voice.unwrap_or_else(|| default_voice(language)), text)?;
    let bytes = fs::read(path).ok()?;
    Some(format!("data:{AUDIO_MIME};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[cfg(test)]
pub(crate) mod tests;
