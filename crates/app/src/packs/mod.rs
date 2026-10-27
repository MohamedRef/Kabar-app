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

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into()
}

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

/// Fetches the release's `index.json` and remembers it.
pub fn refresh_index(store: &PackStore) -> Result<PackIndex, String> {
    let base = base_url().ok_or(NOT_CONFIGURED)?;
    let index: PackIndex =
        serde_json::from_slice(&get(&format!("{base}/index.json"))?).map_err(|e| format!("index: {e}"))?;
    if index.format > PACK_FORMAT {
        return Err("the packs need a newer Kabar".into());
    }
    store.save_index(&index)?;
    Ok(index)
}

pub const NOT_CONFIGURED: &str = "no KABAR_PACKS_URL in this build";

/// Downloads and installs one pack (by id, from the newest index).
pub fn download(store: &PackStore, id: &str) -> Result<PackManifest, String> {
    {
        let mut busy = IN_FLIGHT.lock().map_err(|e| e.to_string())?;
        if !busy.insert(id.to_owned()) {
            return Err("already downloading".into());
        }
    }
    let result = (|| {
        let base = base_url().ok_or(NOT_CONFIGURED)?;
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

pub fn downloading(id: &str) -> bool {
    IN_FLIGHT.lock().map(|b| b.contains(id)).unwrap_or(false)
}

/// Just in time (SPEC §20): when a lesson opens, fetch its unit's pack and
/// the next unit's in the background, if auto-download is on. Failures are
/// quiet: the lesson plays with the device's own voice meanwhile.
pub fn prefetch_for(lesson_id: &str) {
    let Some(store) = store() else { return };
    if base_url().is_none() || !store.auto() {
        return;
    }
    let lesson_id = lesson_id.to_owned();
    let _ = std::thread::Builder::new().name("kabar-packs".into()).spawn(move || {
        let index = refresh_index(store).unwrap_or_else(|_| store.index());
        for id in packs::prefetch(&index, &lesson_id, &store.installed_versions()) {
            if let Err(e) = download(store, &id) {
                eprintln!("kabar: prefetching {id} failed: {e}");
            }
        }
    });
}

/// Over the air with the app update check: refreshes the index and
/// re-downloads every installed pack that has a newer version, if
/// auto-download is on. Blocking; called from the updater's thread.
pub fn update_installed() {
    let Some(store) = store() else { return };
    if base_url().is_none() || !store.auto() {
        return;
    }
    let Ok(index) = refresh_index(store) else { return };
    for id in packs::outdated(&index, &store.installed_versions()) {
        if let Err(e) = download(store, &id) {
            eprintln!("kabar: updating pack {id} failed: {e}");
        }
    }
}

/// A `data:` URL of the pre-generated clip for a line, for the WebView's
/// `<audio>` (works the same on the Mac and Android, no file server needed).
/// `voice` is the dialogue speaker's; otherwise the language's narrator.
pub fn clip_url(text: &str, language: &str, voice: Option<Voice>) -> Option<String> {
    let path = store()?.clip(voice.unwrap_or_else(|| default_voice(language)), text)?;
    let bytes = fs::read(path).ok()?;
    Some(format!("data:{AUDIO_MIME};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
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
    fn pack() -> (PackManifest, Vec<Vec<u8>>, PackSummary, Vec<u8>) {
        let l = lesson();
        let mut m = build_manifest(
            "id",
            "bipa1.u01",
            &[PackInput { id: "bipa1.u01.l01", yaml_sha256: "x".into(), lesson: &l }],
            None,
        );
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
    fn auto_download_defaults_on() {
        let dir = tempfile::tempdir().unwrap();
        let store = PackStore::new(dir.path());
        assert!(store.auto());
        store.set_auto(false).unwrap();
        assert!(!store.auto());
    }

    #[test]
    fn embedded_index_lists_every_unit() {
        let index = embedded_index();
        assert!(index.packs.iter().any(|p| p.id == "id/bipa1.u01"));
        assert!(index.packs.iter().any(|p| p.id == "jv/j1.u01"));
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(PackStore::new(dir.path()).index(), index);
    }
}
