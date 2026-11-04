//! Offline packs on this device (SPEC §7, §20; `docs/RUN.md` §12): which units'
//! audio is downloaded, downloading a unit's pack from the GitHub release,
//! checking every byte against its SHA-256, and finding the pre-generated
//! clip for a line the Lesson Player is about to say.
//!
//! Device-local, never synced (audio is a per-device cache):
//!
//! ```text
//! <data>/audio/<key>.mp3     clips, named by their content address
//! <data>/packs/<unit>.json   manifests of the installed packs
//! <data>/packs/index.json    the newest release index seen
//! <data>/packs/prefs.json    auto-download on/off
//! ```
//!
//! The lesson text itself is still built into the app (`build.rs`).

pub mod settings;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::Engine as _;
use kabar_core::packs::{self, audio_key, check_manifest, default_voice, sha256_hex};
use kabar_protocol::cloud::Voice;
use kabar_protocol::pack::{AUDIO_EXT, AUDIO_MIME, PACK_FORMAT, PackIndex, PackManifest, PackSummary};
use serde::{Deserialize, Serialize};

/// The index this build was made with (`content/packs/index.json`): lists
/// every unit even offline, before any release index was fetched.
const EMBEDDED_INDEX: &str = include_str!("../../../../content/packs/index.json");

/// Largest pack tar accepted (a unit is about 1–2 MB).
const MAX_PACK_BYTES: u64 = 64 * 1024 * 1024;

static STORE: OnceLock<PackStore> = OnceLock::new();
/// Packs being downloaded right now (so a prefetch and a tap don't race).
static IN_FLIGHT: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// Opens the pack store under the app's data directory. Called once at start.
pub fn init(data_dir: &Path) {
    let _ = STORE.set(PackStore::new(data_dir));
}

pub fn store() -> Option<&'static PackStore> {
    STORE.get()
}

/// The `packs` release of the Kabar repository (GitHub Releases: free, and
/// public repositories need no token to download).
pub const DEFAULT_PACKS_URL: &str = "https://github.com/MohamedRef/Kabar-app/releases/download/packs";

/// Where the packs are published: `KABAR_PACKS_URL` at run time or build
/// time (`.env`), else [`DEFAULT_PACKS_URL`]. An empty value turns
/// downloads off.
pub fn base_url() -> Option<String> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let url = std::env::var("KABAR_PACKS_URL")
        .ok()
        .or(option_env!("KABAR_PACKS_URL").map(String::from))
        .unwrap_or_else(|| DEFAULT_PACKS_URL.to_owned());
    let url = url.trim().trim_end_matches('/');
    (url.starts_with("https://") || url.starts_with("http://")).then(|| url.to_owned())
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

#[derive(Debug, PartialEq, Eq)]
pub struct PackStore {
    audio: PathBuf,
    packs: PathBuf,
}

impl PackStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { audio: data_dir.join("audio"), packs: data_dir.join("packs") }
    }

    fn clip_path(&self, key: &str) -> PathBuf {
        self.audio.join(format!("{key}.{AUDIO_EXT}"))
    }

    fn manifest_path(&self, unit: &str) -> PathBuf {
        self.packs.join(format!("{unit}.json"))
    }

    /// The pre-generated clip for `text` in `voice`, if a downloaded pack has it.
    pub fn clip(&self, voice: Voice, text: &str) -> Option<PathBuf> {
        let path = self.clip_path(&audio_key(voice, text));
        path.is_file().then_some(path)
    }

    /// Manifests of the installed packs.
    pub fn installed(&self) -> Vec<PackManifest> {
        let Ok(entries) = fs::read_dir(&self.packs) else { return Vec::new() };
        let mut out: Vec<PackManifest> = entries
            .flatten()
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.ends_with(".json") && name != "index.json" && name != "prefs.json"
            })
            .filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok())
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Pack id → installed version.
    pub fn installed_versions(&self) -> BTreeMap<String, u32> {
        self.installed().into_iter().map(|m| (m.id, m.version)).collect()
    }

    /// Bytes of all cached clips.
    pub fn storage_bytes(&self) -> u64 {
        let Ok(entries) = fs::read_dir(&self.audio) else { return 0 };
        entries.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
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

    pub fn auto(&self) -> bool {
        fs::read(self.packs.join("prefs.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Prefs>(&b).ok())
            .is_none_or(|p| p.auto)
    }

    pub fn set_auto(&self, auto: bool) -> Result<(), String> {
        fs::create_dir_all(&self.packs).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec(&Prefs { auto }).map_err(|e| e.to_string())?;
        fs::write(self.packs.join("prefs.json"), json).map_err(|e| e.to_string())
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
    /// manifest hash, and only then installs it.
    pub fn install(&self, expected: &PackSummary, tar: &[u8]) -> Result<PackManifest, String> {
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
        Ok(manifest)
    }

    /// Removes a pack, and every clip no other installed pack shares.
    pub fn remove(&self, unit: &str) -> Result<(), String> {
        match fs::remove_file(self.manifest_path(unit)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let keep = self.installed();
        let cached = self.cached_keys();
        for key in packs::orphans(cached.iter().map(String::as_str), &keep) {
            let _ = fs::remove_file(self.clip_path(&key));
        }
        Ok(())
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("part");
    fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn embedded_index() -> PackIndex {
    serde_json::from_str(EMBEDDED_INDEX).unwrap_or_default()
}

// --- network (blocking; call from a worker thread) ------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into()
}

/// The web build fetches packs in a later phase (docs/WEB.md); [`base_url`]
/// is `None` there, so nothing asks.
#[cfg(target_arch = "wasm32")]
fn get(_url: &str) -> Result<Vec<u8>, String> {
    Err(NOT_CONFIGURED.into())
}

#[cfg(not(target_arch = "wasm32"))]
fn get(url: &str) -> Result<Vec<u8>, String> {
    let mut res = agent().get(url).call().map_err(|e| format!("network: {e}"))?;
    let status = res.status().as_u16();
    if status != 200 {
        return Err(format!("{url}: HTTP {status}"));
    }
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(MAX_PACK_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("network: {e}"))?;
    if bytes.len() as u64 > MAX_PACK_BYTES {
        return Err("pack too large".into());
    }
    Ok(bytes)
}

/// Fetches the `index.json` of the release at `base` and remembers it.
fn refresh_index_from(store: &PackStore, base: &str) -> Result<PackIndex, String> {
    let index: PackIndex =
        serde_json::from_slice(&get(&format!("{base}/index.json"))?).map_err(|e| format!("index: {e}"))?;
    if index.format > PACK_FORMAT {
        return Err("the packs need a newer Kabar".into());
    }
    store.save_index(&index)?;
    Ok(index)
}

pub const NOT_CONFIGURED: &str = "no KABAR_PACKS_URL in this build";

/// Downloads and installs one pack (by id, from the newest index) from the
/// release at `base` (`None`: not configured).
fn download_from(store: &PackStore, base: Option<&str>, id: &str) -> Result<PackManifest, String> {
    {
        let mut busy = IN_FLIGHT.lock().map_err(|e| e.to_string())?;
        if !busy.insert(id.to_owned()) {
            return Err("already downloading".into());
        }
    }
    let result = (|| {
        let base = base.ok_or(NOT_CONFIGURED)?;
        let index = store.index();
        let row = index.get(id).ok_or("no such pack")?;
        if !row.downloadable() {
            return Err("this unit's audio isn't published yet".to_owned());
        }
        let tar = get(&format!("{base}/{}", row.file))?;
        store.install(row, &tar)
    })();
    if let Ok(mut busy) = IN_FLIGHT.lock() {
        busy.remove(id);
    }
    result
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
/// download leaves the list; a failed one stays until retried.
static JOBS: Mutex<Vec<(String, Job)>> = Mutex::new(Vec::new());
static WORKER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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

/// Queues a pack for `store` (again, after a failure) and makes sure the
/// queue runs, downloading from the release at `base`.
fn enqueue_in(store: &'static PackStore, base: Option<String>, id: &str) {
    queue(&JOBS, id);
    if !WORKER.swap(true, std::sync::atomic::Ordering::SeqCst) {
        let started = std::thread::Builder::new().name("kabar-packs".into()).spawn(move || {
            run_queue(&JOBS, |id| download_from(store, base.as_deref(), id).map(drop));
            WORKER.store(false, std::sync::atomic::Ordering::SeqCst);
            // Asked for while the worker was finishing: run again.
            if JOBS.lock().is_ok_and(|j| j.iter().any(|(_, s)| *s == Job::Queued)) {
                enqueue_again(store, base);
            }
        });
        if started.is_err() {
            WORKER.store(false, std::sync::atomic::Ordering::SeqCst);
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
    let _ = std::thread::Builder::new().name("kabar-packs".into()).spawn(move || {
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
pub(crate) mod tests {
    use kabar_core::packs::{PackInput, build_manifest, refresh_totals, summary};
    use kabar_protocol::lesson::LessonContent;

    use super::*;

    fn lesson() -> LessonContent {
        let yaml = crate::lesson::lesson_content("bipa1.u01.l01").expect("bipa1.u01.l01 ships");
        kabar_core::lesson::parse_lesson(&yaml).unwrap()
    }

    fn tar_of(m: &PackManifest, clips: &[Vec<u8>]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        let mut add = |name: &str, data: &[u8]| {
            let mut h = tar::Header::new_ustar();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            b.append_data(&mut h, name, data).unwrap();
        };
        add("manifest.json", &serde_json::to_vec(m).unwrap());
        for (a, bytes) in m.audio.iter().zip(clips) {
            add(&format!("audio/{}.mp3", a.key), bytes);
        }
        b.into_inner().unwrap()
    }

    /// A manifest with fake clips, its index row and its tar (the layout
    /// `kabar-tools packs bundle` writes).
    pub(super) fn pack() -> (PackManifest, Vec<Vec<u8>>, PackSummary, Vec<u8>) {
        pack_v(1)
    }

    /// [`pack`] at `version`.
    pub(super) fn pack_v(version: u32) -> (PackManifest, Vec<Vec<u8>>, PackSummary, Vec<u8>) {
        let l = lesson();
        let mut m = build_manifest(
            "id",
            "bipa1.u01",
            &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "x".into(), lesson: &l }],
            None,
        );
        m.version = version;
        let clips: Vec<Vec<u8>> = (0..m.audio.len()).map(|i| format!("clip {i}").into_bytes()).collect();
        for (a, bytes) in m.audio.iter_mut().zip(&clips) {
            a.sha256 = Some(sha256_hex(bytes));
            a.size = Some(bytes.len() as u64);
        }
        refresh_totals(&mut m);
        let tar = tar_of(&m, &clips);
        let mut row = summary(&m, None);
        row.file_sha256 = Some(sha256_hex(&tar));
        row.file_size = Some(tar.len() as u64);
        (m, clips, row, tar)
    }

    #[test]
    fn install_find_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let store = PackStore::new(dir.path());
        let (m, _, row, tar) = pack();
        assert!(store.installed().is_empty());

        store.install(&row, &tar).unwrap();
        assert_eq!(store.installed_versions().get("id/bipa1.u01"), Some(&1));
        let first = &m.audio[0];
        let found = store.clip(first.voice, &format!("  {} ", first.text)).expect("clip");
        assert_eq!(fs::read(found).unwrap(), b"clip 0");
        assert!(store.storage_bytes() > 0);
        // The lesson's narrator lines use the default voice.
        let l = lesson();
        let narrated = &l.slides.iter().flat_map(|s| &s.examples).next().unwrap().text;
        assert!(store.clip(default_voice("id"), narrated).is_some());

        store.remove("bipa1.u01").unwrap();
        assert!(store.installed().is_empty());
        assert_eq!(store.storage_bytes(), 0);
    }

    #[test]
    fn damaged_downloads_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = PackStore::new(dir.path());
        let (m, mut clips, row, mut tar) = pack();
        // Not the tar the index announced.
        let at = tar.len() / 2;
        tar[at] ^= 1;
        assert!(store.install(&row, &tar).unwrap_err().contains("hash"));
        // The announced tar, but a clip that doesn't match its manifest hash.
        clips[0] = b"tampered".to_vec();
        let bad = tar_of(&m, &clips);
        let row = PackSummary { file_sha256: Some(sha256_hex(&bad)), ..row };
        assert!(store.install(&row, &bad).unwrap_err().contains("damaged"));
        // A manifest that isn't the one the index lists.
        let (_, clips, row, _) = pack();
        let mut other = m.clone();
        other.version = 7;
        let t = tar_of(&other, &clips);
        let row = PackSummary { file_sha256: Some(sha256_hex(&t)), ..row };
        assert!(store.install(&row, &t).is_err());
        assert!(store.installed().is_empty());
        assert_eq!(store.storage_bytes(), 0);
    }

    #[test]
    fn the_queue_runs_in_order_and_keeps_failures_for_a_retry() {
        let jobs = Mutex::new(Vec::new());
        for id in ["a", "b", "c", "a"] {
            queue(&jobs, id);
        }
        let mut order = Vec::new();
        run_queue(&jobs, |id| {
            order.push(id.to_owned());
            if id == "b" { Err("network".into()) } else { Ok(()) }
        });
        assert_eq!(order, ["a", "b", "c"], "one at a time, asked twice runs once");
        assert_eq!(*jobs.lock().unwrap(), [("b".to_owned(), Job::Failed)]);
        queue(&jobs, "b");
        assert_eq!(*jobs.lock().unwrap(), [("b".to_owned(), Job::Queued)], "retry");
        run_queue(&jobs, |_| Ok(()));
        assert!(jobs.lock().unwrap().is_empty());
    }

    #[test]
    fn auto_download_defaults_on() {
        let dir = tempfile::tempdir().unwrap();
        let store = PackStore::new(dir.path());
        assert!(store.auto());
        store.set_auto(false).unwrap();
        assert!(!store.auto());
        // Prefs saved before the switch existed (or damaged) mean on.
        fs::write(dir.path().join("packs/prefs.json"), "{}").unwrap();
        assert!(store.auto());
        fs::write(dir.path().join("packs/prefs.json"), "not json").unwrap();
        assert!(store.auto());
    }

    #[test]
    fn without_a_pack_store_nothing_is_fetched() {
        // The app opens its store at start; before that (and in these
        // tests) prefetching and updating quietly do nothing.
        assert!(store().is_none());
        assert_eq!(update_installed(true), 0);
        prefetch_for("bipa1.u01.l01");
        assert_eq!(clip_url("Selamat pagi", "id", None), None);
        assert!(jobs().is_empty() || jobs().values().all(|j| *j != Job::Queued));
    }

    #[test]
    fn embedded_index_lists_every_unit() {
        let index = embedded_index();
        assert!(index.packs.iter().any(|p| p.id == "id/bipa1.u01"));
        assert!(index.packs.iter().any(|p| p.id == "jv/j1.u01"));
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(PackStore::new(dir.path()).index(), index);
    }

    // ------------------------------------------------------- over the network

    /// Serves `files` over plain HTTP on localhost (404 for anything else),
    /// and logs each path asked for.
    pub(super) fn serve(files: BTreeMap<String, Vec<u8>>) -> (String, std::sync::Arc<Mutex<Vec<String>>>) {
        use std::io::{BufRead as _, BufReader, Write as _};
        let log = std::sync::Arc::new(Mutex::new(Vec::new()));
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
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                }
                seen.lock().unwrap().push(path.clone());
                let mut stream = reader.into_inner();
                let _ = match files.get(&path) {
                    Some(body) => stream
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            )
                            .as_bytes(),
                        )
                        .and_then(|()| stream.write_all(body)),
                    None => stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
            }
        });
        (url, log)
    }

    /// Downloads share `IN_FLIGHT` and the job queue: tests that download
    /// take turns, so one never sees another's "already downloading".
    pub(super) fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static TURN: Mutex<()> = Mutex::new(());
        TURN.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn index_of(rows: Vec<PackSummary>) -> PackIndex {
        PackIndex { format: PACK_FORMAT, packs: rows }
    }

    /// A release holding `rows` as its index and each `(file, tar)`.
    pub(super) fn release(
        rows: Vec<PackSummary>,
        tars: &[(&str, &[u8])],
    ) -> (String, std::sync::Arc<Mutex<Vec<String>>>) {
        let mut files = BTreeMap::new();
        files.insert("index.json".to_owned(), serde_json::to_vec(&index_of(rows)).unwrap());
        for (name, tar) in tars {
            files.insert((*name).to_owned(), tar.to_vec());
        }
        serve(files)
    }

    fn store_in(dir: &tempfile::TempDir) -> PackStore {
        PackStore::new(dir.path())
    }

    #[test]
    fn a_fetched_index_is_remembered_and_wins_over_the_built_in_one() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let (_, _, row, tar) = pack();
        let (url, log) = release(vec![row.clone()], &[(&row.file, &tar)]);
        let index = refresh_index_from(&store, &url).unwrap();
        assert_eq!(index.packs, vec![row.clone()]);
        assert_eq!(store.index(), index, "the release's index, not the built-in one");
        assert_eq!(*log.lock().unwrap(), ["index.json"]);
        // A fetched index with nothing in it never hides the built-in units.
        let (empty, _) = release(Vec::new(), &[]);
        refresh_index_from(&store, &empty).unwrap();
        assert_eq!(store.index(), embedded_index());
    }

    #[test]
    fn index_failures_say_what_went_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        // Nothing published there.
        let (url, _) = serve(BTreeMap::new());
        assert!(refresh_index_from(&store, &url).unwrap_err().ends_with("HTTP 404"));
        // Not an index.
        let mut files = BTreeMap::new();
        files.insert("index.json".to_owned(), b"<html>".to_vec());
        let (url, _) = serve(files);
        assert!(refresh_index_from(&store, &url).unwrap_err().starts_with("index:"));
        // Made for a newer app: refused, and not remembered.
        let newer = PackIndex { format: PACK_FORMAT + 1, packs: vec![pack().2] };
        let mut files = BTreeMap::new();
        files.insert("index.json".to_owned(), serde_json::to_vec(&newer).unwrap());
        let (url, _) = serve(files);
        assert_eq!(refresh_index_from(&store, &url).unwrap_err(), "the packs need a newer Kabar");
        assert_eq!(store.index(), embedded_index());
        // Nobody listening at all.
        let closed = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}", l.local_addr().unwrap())
        };
        assert!(refresh_index_from(&store, &closed).unwrap_err().starts_with("network:"));
    }

    #[test]
    fn a_download_is_checked_then_installed_and_plays() {
        let _turn = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let (m, clips, row, tar) = pack();
        let (url, log) = release(vec![row.clone()], &[(&row.file, &tar)]);
        refresh_index_from(&store, &url).unwrap();
        let installed = download_from(&store, Some(&url), &row.id).unwrap();
        assert_eq!(installed, m);
        assert_eq!(store.installed_versions().get(&row.id), Some(&1));
        assert_eq!(log.lock().unwrap().last().map(String::as_str), Some(row.file.as_str()));
        // The Lesson Player gets the clip as a data URL for <audio>.
        let first = &m.audio[0];
        let url = clip_url_in(&store, &first.text, "id", Some(first.voice)).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&clips[0]);
        assert_eq!(url, format!("data:{AUDIO_MIME};base64,{b64}"));
        assert_eq!(clip_url_in(&store, "a line no pack has", "id", None), None);
        // Nothing is left running.
        assert!(!jobs().contains_key(&row.id));
    }

    #[test]
    fn downloads_that_cannot_happen_are_refused_plainly() {
        let _turn = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let (_, _, row, tar) = pack();
        let (url, _) = release(vec![row.clone()], &[(&row.file, &tar)]);
        // A build without a packs address.
        assert_eq!(download_from(&store, None, &row.id).unwrap_err(), NOT_CONFIGURED);
        // A unit the index doesn't list.
        refresh_index_from(&store, &url).unwrap();
        assert_eq!(download_from(&store, Some(&url), "id/nope").unwrap_err(), "no such pack");
        // Listed, but its audio isn't bundled yet.
        let waiting = PackSummary { file_sha256: None, ..row.clone() };
        let (url2, log) = release(vec![waiting], &[]);
        refresh_index_from(&store, &url2).unwrap();
        assert_eq!(
            download_from(&store, Some(&url2), &row.id).unwrap_err(),
            "this unit's audio isn't published yet"
        );
        assert_eq!(*log.lock().unwrap(), ["index.json"], "no tar asked for");
        // Announced but missing from the release.
        let (url3, _) = release(vec![row.clone()], &[]);
        refresh_index_from(&store, &url3).unwrap();
        let err = download_from(&store, Some(&url3), &row.id).unwrap_err();
        assert!(err.ends_with("HTTP 404"), "{err}");
        // The same pack already on its way (a prefetch and a tap).
        IN_FLIGHT.lock().unwrap().insert(row.id.clone());
        assert_eq!(jobs().get(&row.id), Some(&Job::Downloading), "shown as downloading");
        assert_eq!(download_from(&store, Some(&url), &row.id).unwrap_err(), "already downloading");
        IN_FLIGHT.lock().unwrap().remove(&row.id);
        assert!(store.installed().is_empty());
    }

    #[test]
    fn opening_a_lesson_fetches_its_unit_and_updates_bring_new_versions() {
        let _turn = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let (_, _, row, tar) = pack();
        let (url, log) = release(vec![row.clone()], &[(&row.file, &tar)]);
        prefetch_from(&store, &url, "bipa1.u01.l02");
        assert_eq!(store.installed_versions().get(&row.id), Some(&1));
        assert_eq!(*log.lock().unwrap(), ["index.json".to_owned(), row.file.clone()]);
        // Installed and current: nothing more to fetch or update.
        prefetch_from(&store, &url, "bipa1.u01.l03");
        assert_eq!(log.lock().unwrap().len(), 3, "only the index again");
        assert_eq!(update_installed_from(&store, &url), 0);

        // Version 2 on the release: the update re-downloads it.
        let (_, _, row2, tar2) = pack_v(2);
        let (url2, _) = release(vec![row2.clone()], &[(&row2.file, &tar2)]);
        assert_eq!(update_installed_from(&store, &url2), 1);
        assert_eq!(store.installed_versions().get(&row.id), Some(&2));
        // An index that can't be read updates nothing.
        let (gone, _) = serve(BTreeMap::new());
        assert_eq!(update_installed_from(&store, &gone), 0);
        // A broken tar on the release: counted as not updated, old pack kept.
        let (_, _, row3, _) = pack_v(3);
        let (url3, _) = release(vec![row3.clone()], &[(&row3.file, b"not a tar")]);
        assert_eq!(update_installed_from(&store, &url3), 0);
        assert_eq!(store.installed_versions().get(&row.id), Some(&2));
        // With no index at all, prefetch falls back to the one it has.
        prefetch_from(&store, &gone, "bipa1.u01.l01");
        assert_eq!(store.installed_versions().get(&row.id), Some(&2));
    }

    #[test]
    fn packs_with_odd_contents_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let (m, clips, row, _) = pack();
        let signed = |tar: Vec<u8>| (PackSummary { file_sha256: Some(sha256_hex(&tar)), ..row.clone() }, tar);
        // No manifest in the tar.
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_ustar();
        h.set_size(1);
        h.set_mode(0o644);
        b.append_data(&mut h, "readme.txt", &b"x"[..]).unwrap();
        let (r, t) = signed(b.into_inner().unwrap());
        assert_eq!(store.install(&r, &t).unwrap_err(), "the pack has no manifest");
        // A clip the manifest doesn't list.
        let tar = {
            let mut b = tar::Builder::new(Vec::new());
            let mut add = |name: &str, data: &[u8]| {
                let mut h = tar::Header::new_ustar();
                h.set_size(data.len() as u64);
                h.set_mode(0o644);
                b.append_data(&mut h, name, data).unwrap();
            };
            add("manifest.json", &serde_json::to_vec(&m).unwrap());
            add("audio/stray.mp3", b"stray");
            b.into_inner().unwrap()
        };
        let (r, t) = signed(tar);
        assert!(store.install(&r, &t).unwrap_err().contains("isn't in the manifest"));
        // A clip the manifest lists but the tar lacks.
        let short: Vec<Vec<u8>> = clips[..clips.len() - 1].to_vec();
        let (r, t) = signed(tar_of(&m, &short));
        assert!(store.install(&r, &t).unwrap_err().ends_with("is missing"));
        assert!(store.installed().is_empty());
    }

    #[test]
    fn removing_one_pack_keeps_clips_another_still_needs() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let (m, _, row, tar) = pack();
        store.install(&row, &tar).unwrap();
        // A second unit's manifest sharing the first clip.
        let mut other = m.clone();
        other.id = "id/bipa1.u02".into();
        other.unit = "bipa1.u02".into();
        other.audio.truncate(1);
        fs::write(store.manifest_path("bipa1.u02"), serde_json::to_vec(&other).unwrap()).unwrap();
        store.remove("bipa1.u01").unwrap();
        assert!(store.clip(m.audio[0].voice, &m.audio[0].text).is_some(), "still needed by bipa1.u02");
        assert!(m.audio[1..].iter().all(|a| store.clip(a.voice, &a.text).is_none()));
        // Removing what isn't there is fine.
        store.remove("bipa9.u09").unwrap();
        // The release address: plain https by default.
        if std::env::var_os("KABAR_PACKS_URL").is_none() && option_env!("KABAR_PACKS_URL").is_none() {
            assert_eq!(base_url().as_deref(), Some(DEFAULT_PACKS_URL));
        }
    }
}
