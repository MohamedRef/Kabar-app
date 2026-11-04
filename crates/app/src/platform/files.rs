//! The few files Kabar keeps outside its database (cloud voices, downloaded
//! episodes), on every target: plain files on the Mac and Android; on the web,
//! IndexedDB entries named by path, with an index (path and size, and the
//! bytes of small files) loaded at start-up so `size` and small reads stay
//! synchronous. A large file's bytes are read into memory on its first
//! [`read`], so [`read_loaded`] finds them afterwards.

use std::io;
use std::path::{Path, PathBuf};

/// The file's size, if it exists.
pub fn size(path: &Path) -> Option<u64> {
    imp::size(path)
}

/// The file's bytes.
pub async fn read(path: &Path) -> io::Result<Vec<u8>> {
    imp::read(path).await
}

/// The file's bytes without waiting: always on the Mac and Android; on the
/// web for small files and ones [`read`] or [`write`] touched this session.
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
    use super::*;

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
            let tmp = path.with_extension("part");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(&tmp, &path)
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
    use std::cell::RefCell;
    use std::collections::{BTreeMap, HashMap};

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::platform::web::{idb_delete_key, idb_get_bytes, idb_put_bytes};

    /// Files this small live in the index itself.
    const INLINE_BYTES: usize = 16 * 1024;
    const INDEX_KEY: &str = "files.json";

    #[derive(Default, Serialize, Deserialize)]
    struct Entry {
        size: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inline: Option<Vec<u8>>,
    }

    thread_local! {
        static INDEX: RefCell<BTreeMap<String, Entry>> = const { RefCell::new(BTreeMap::new()) };
        /// Large files read or written this session.
        static LOADED: RefCell<HashMap<String, Vec<u8>>> = RefCell::new(HashMap::new());
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
        let Some(bytes) = idb_get_bytes(INDEX_KEY).await else { return };
        match serde_json::from_slice(&bytes) {
            Ok(index) => INDEX.with(|i| *i.borrow_mut() = index),
            Err(e) => crate::diag::report!("the browser's file list is unreadable: {e}"),
        }
    }

    fn save_index() {
        let json = INDEX.with(|i| serde_json::to_vec(&*i.borrow())).unwrap_or_default();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = idb_put_bytes(INDEX_KEY, &json).await {
                crate::diag::report!("saving the browser's file list failed: {e}");
            }
        });
    }

    pub fn size(path: &Path) -> Option<u64> {
        INDEX.with(|i| i.borrow().get(&key(path)).map(|e| e.size))
    }

    pub fn read_loaded(path: &Path) -> io::Result<Vec<u8>> {
        let key = key(path);
        if let Some(bytes) = INDEX.with(|i| i.borrow().get(&key).and_then(|e| e.inline.clone())) {
            return Ok(bytes);
        }
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
        let inline = if bytes.len() <= INLINE_BYTES {
            Some(bytes)
        } else {
            idb_put_bytes(&content_key(&key), &bytes).await.map_err(io::Error::other)?;
            LOADED.with(|l| l.borrow_mut().insert(key.clone(), bytes));
            None
        };
        INDEX.with(|i| i.borrow_mut().insert(key, Entry { size, inline }));
        save_index();
        Ok(())
    }

    fn forget(keys: Vec<String>) {
        if keys.is_empty() {
            return;
        }
        for key in &keys {
            let inline = INDEX.with(|i| i.borrow_mut().remove(key)).is_some_and(|e| e.inline.is_some());
            LOADED.with(|l| l.borrow_mut().remove(key));
            if !inline {
                let content = content_key(key);
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
