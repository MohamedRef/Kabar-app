//! The few files Kabar keeps outside its database (cloud voices, downloaded
//! episodes), on every target: plain files on the Mac and Android; on the web,
//! one IndexedDB entry (`file:<path>`) per file plus a slim `files.json`
//! index (`path -> size`) loaded at start-up so `size` stays synchronous.
//! Metadata `.json` files (such as downloaded episode cue lists) are preloaded
//! at boot, and audio files enter memory on their first [`read`] or [`write`],
//! so [`read_loaded`] finds them afterwards without bloating `files.json`.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One entry in the web build's `files.json` index. Legacy `inline` byte
/// arrays from older builds still deserialize so `boot` can migrate them out
/// to their own `file:<key>` entries, and are omitted when `files.json` is
/// saved back.
#[derive(Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct IndexEntry {
    size: u64,
    #[serde(default, skip_serializing)]
    inline: Option<Vec<u8>>,
}

/// The file's size, if it exists.
pub fn size(path: &Path) -> Option<u64> {
    imp::size(path)
}

/// The file's bytes.
pub async fn read(path: &Path) -> io::Result<Vec<u8>> {
    imp::read(path).await
}

/// The file's bytes without waiting: always on the Mac and Android; on the
/// web for `.json` metadata files and any file [`read`] or [`write`] touched
/// this session.
pub fn read_loaded(path: &Path) -> io::Result<Vec<u8>> {
    imp::read_loaded(path)
}

/// Replaces the file in one step, creating its folder.
pub async fn write(path: &Path, bytes: Vec<u8>) -> io::Result<()> {
    imp::write(path, bytes).await
}

pub fn remove(path: &Path) {
    imp::remove(path)
}

/// Files directly in `dir` with this extension, and their sizes.
pub fn list(dir: &Path, extension: &str) -> Vec<(PathBuf, u64)> {
    imp::list(dir, extension)
}

/// `dir` and everything in it.
pub fn remove_dir_all(dir: &Path) {
    imp::remove_dir_all(dir)
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

    pub fn size(path: &Path) -> Option<u64> {
        std::fs::metadata(path).ok().filter(|m| m.is_file()).map(|m| m.len())
    }

    pub async fn read(path: &Path) -> io::Result<Vec<u8>> {
        let path = path.to_path_buf();
        crate::platform::time::blocking(move || std::fs::read(path)).await
    }

    pub fn read_loaded(path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    pub async fn write(path: &Path, bytes: Vec<u8>) -> io::Result<()> {
        let path = path.to_path_buf();
        crate::platform::time::blocking(move || {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let name = path.file_name().map(|f| f.to_string_lossy()).unwrap_or_default();
            let tmp = path.with_file_name(format!(".{name}.{}.{n}.part", std::process::id()));
            std::fs::write(&tmp, bytes)?;
            if let Err(e) = std::fs::rename(&tmp, &path) {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
            Ok(())
        })
        .await
    }

    pub fn remove(path: &Path) {
        std::fs::remove_file(path).ok();
    }

    pub fn list(dir: &Path, extension: &str) -> Vec<(PathBuf, u64)> {
        let entries = std::fs::read_dir(dir).into_iter().flatten().flatten();
        entries
            .filter(|e| e.path().extension().is_some_and(|x| x == extension))
            .map(|e| (e.path(), e.metadata().map(|m| m.len()).unwrap_or(0)))
            .collect()
    }

    pub fn remove_dir_all(dir: &Path) {
        std::fs::remove_dir_all(dir).ok();
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::{BTreeMap, HashMap};
    use std::time::Duration;

    use super::*;
    use crate::platform::web::{idb_delete_key, idb_get_bytes, idb_get_checked, idb_put_bytes};

    const INDEX_KEY: &str = "files.json";

    thread_local! {
        static INDEX: RefCell<BTreeMap<String, IndexEntry>> = const { RefCell::new(BTreeMap::new()) };
        /// `.json` metadata files and files read or written this session.
        static LOADED: RefCell<HashMap<String, Vec<u8>>> = RefCell::new(HashMap::new());
        /// Set when `files.json` could not be read or parsed at boot, so a
        /// later write never clobbers the existing index and orphans files.
        static INDEX_UNREADABLE: Cell<bool> = const { Cell::new(false) };
        static SAVE_QUEUED: Cell<bool> = const { Cell::new(false) };
        static SAVE_AGAIN: Cell<bool> = const { Cell::new(false) };
    }

    fn key(path: &Path) -> String {
        path.to_string_lossy().trim_start_matches('/').to_string()
    }

    fn content_key(key: &str) -> String {
        format!("file:{key}")
    }

    fn not_found() -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, "not in this browser")
    }

    /// Before the app starts (from `platform::web::boot`).
    pub async fn boot() {
        let bytes = match idb_get_checked(INDEX_KEY).await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return,
            Err(e) => {
                INDEX_UNREADABLE.with(|u| u.set(true));
                crate::diag::report!("reading the browser's file list failed: {e}");
                return;
            }
        };
        let mut parsed: BTreeMap<String, IndexEntry> = match serde_json::from_slice(&bytes) {
            Ok(index) => index,
            Err(e) => {
                INDEX_UNREADABLE.with(|u| u.set(true));
                crate::diag::report!("the browser's file list is unreadable: {e}");
                return;
            }
        };
        let mut migrated = false;
        let mut preload_json = Vec::new();
        for (k, entry) in &mut parsed {
            if let Some(inline) = entry.inline.take() {
                if idb_put_bytes(&content_key(k), &inline).await.is_ok() {
                    migrated = true;
                }
                LOADED.with(|l| l.borrow_mut().insert(k.clone(), inline));
            } else if k.ends_with(".json") {
                preload_json.push(k.clone());
            }
        }
        INDEX.with(|i| *i.borrow_mut() = parsed);
        for k in preload_json {
            if let Some(data) = idb_get_bytes(&content_key(&k)).await {
                LOADED.with(|l| l.borrow_mut().insert(k, data));
            }
        }
        if migrated {
            save_index();
        }
    }

    fn save_index() {
        if INDEX_UNREADABLE.with(Cell::get) {
            return;
        }
        if SAVE_QUEUED.with(Cell::get) {
            SAVE_AGAIN.with(|a| a.set(true));
            return;
        }
        SAVE_QUEUED.with(|q| q.set(true));
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                SAVE_AGAIN.with(|a| a.set(false));
                // Coalesce bursts of writes or deletes in the same tick into one save.
                crate::platform::time::sleep(Duration::ZERO).await;
                let json = INDEX.with(|i| serde_json::to_vec(&*i.borrow())).unwrap_or_default();
                if let Err(e) = idb_put_bytes(INDEX_KEY, &json).await {
                    crate::diag::report!("saving the browser's file list failed: {e}");
                }
                if !SAVE_AGAIN.with(Cell::get) {
                    break;
                }
            }
            SAVE_QUEUED.with(|q| q.set(false));
        });
    }

    pub fn size(path: &Path) -> Option<u64> {
        INDEX.with(|i| i.borrow().get(&key(path)).map(|e| e.size))
    }

    pub fn read_loaded(path: &Path) -> io::Result<Vec<u8>> {
        let key = key(path);
        LOADED.with(|l| l.borrow().get(&key).cloned()).ok_or_else(not_found)
    }

    pub async fn read(path: &Path) -> io::Result<Vec<u8>> {
        if size(path).is_none() {
            return Err(not_found());
        }
        if let Ok(bytes) = read_loaded(path) {
            return Ok(bytes);
        }
        let key = key(path);
        let bytes = idb_get_bytes(&content_key(&key)).await.ok_or_else(not_found)?;
        LOADED.with(|l| l.borrow_mut().insert(key, bytes.clone()));
        Ok(bytes)
    }

    pub async fn write(path: &Path, bytes: Vec<u8>) -> io::Result<()> {
        let key = key(path);
        let size = bytes.len() as u64;
        idb_put_bytes(&content_key(&key), &bytes).await.map_err(io::Error::other)?;
        LOADED.with(|l| l.borrow_mut().insert(key.clone(), bytes));
        INDEX.with(|i| i.borrow_mut().insert(key, IndexEntry { size, inline: None }));
        save_index();
        Ok(())
    }

    fn forget(keys: Vec<String>) {
        if keys.is_empty() {
            return;
        }
        for key in keys {
            let existed = INDEX.with(|i| i.borrow_mut().remove(&key).is_some());
            LOADED.with(|l| l.borrow_mut().remove(&key));
            if existed {
                let content = content_key(&key);
                wasm_bindgen_futures::spawn_local(async move { idb_delete_key(&content).await });
            }
        }
        save_index();
    }

    pub fn remove(path: &Path) {
        forget(vec![key(path)]);
    }

    fn under(dir: &Path) -> Vec<String> {
        let prefix = format!("{}/", key(dir));
        INDEX.with(|i| i.borrow().keys().filter(|k| k.starts_with(&prefix)).cloned().collect())
    }

    pub fn list(dir: &Path, extension: &str) -> Vec<(PathBuf, u64)> {
        let prefix = format!("{}/", key(dir));
        INDEX.with(|i| {
            i.borrow()
                .iter()
                .filter(|(k, _)| {
                    k.strip_prefix(&prefix).is_some_and(|name| {
                        !name.contains('/') && Path::new(name).extension().is_some_and(|x| x == extension)
                    })
                })
                .map(|(k, e)| (PathBuf::from(k), e.size))
                .collect()
        })
    }

    pub fn remove_dir_all(dir: &Path) {
        forget(under(dir));
    }
}

#[cfg(target_arch = "wasm32")]
pub use imp::boot;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn web_index_reads_legacy_inline_bytes_and_saves_only_sizes() {
        let legacy = r#"{"audio/clip.mp3":{"size":3,"inline":[1,2,3]}}"#;
        let parsed: BTreeMap<String, IndexEntry> = serde_json::from_str(legacy).unwrap();
        assert_eq!(parsed["audio/clip.mp3"].size, 3);
        assert_eq!(parsed["audio/clip.mp3"].inline.as_deref(), Some(&[1, 2, 3][..]));

        let slim = serde_json::to_string(&parsed).unwrap();
        assert_eq!(slim, r#"{"audio/clip.mp3":{"size":3}}"#, "no inline byte arrays written back");
    }

    #[test]
    fn native_files_write_atomically_list_and_clean_up() {
        let dir = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let a = dir.path().join("episodes/one.mp3");
        let b = dir.path().join("episodes/one.json");
        rt.block_on(write(&a, vec![10, 20, 30])).unwrap();
        rt.block_on(write(&b, br#"[0.0]"#.to_vec())).unwrap();
        assert_eq!(size(&a), Some(3));
        assert_eq!(read_loaded(&a).unwrap(), [10, 20, 30]);
        assert_eq!(rt.block_on(read(&b)).unwrap(), br#"[0.0]"#);
        assert_eq!(list(&dir.path().join("episodes"), "mp3"), [(a.clone(), 3)]);
        remove(&a);
        assert_eq!(size(&a), None);
        remove_dir_all(&dir.path().join("episodes"));
        assert_eq!(size(&b), None);
    }
}
