//! Azure neural voices through Kabar Cloud's TTS proxy, cached forever on
//! the device (spec §20). Each file is the MP3 for one [`TtsRequest`], named
//! by the SHA-256 of the request, so the same line in the same voice is only
//! ever fetched once, and downloaded episodes play offline.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use kabar_protocol::cloud::{TtsRequest, TtsSegment, Voice};
use sha2::{Digest, Sha256};

use crate::cloud::link::{CloudLink, Unreachable, off_thread};

#[derive(Clone)]
pub struct AudioCache {
    dir: PathBuf,
    link: CloudLink,
}

/// What a set of downloaded files takes on disk.
#[cfg_attr(not(test), allow(dead_code))] // Offline downloads: Kabar Radio and Sinetron (PLAN §16 B).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub files: u64,
    pub bytes: u64,
}

impl AudioCache {
    /// `dir` is created on first write.
    pub fn new(dir: PathBuf, link: CloudLink) -> Self {
        Self { dir, link }
    }

    pub fn path(&self, req: &TtsRequest) -> PathBuf {
        self.dir.join(format!("{}.mp3", key(req)))
    }

    /// The file, if this request was fetched before.
    pub fn cached(&self, req: &TtsRequest) -> Option<PathBuf> {
        let path = self.path(req);
        path.is_file().then_some(path)
    }

    /// The cached file, or fetches it once from Kabar Cloud.
    pub async fn fetch(&self, req: &TtsRequest) -> Result<PathBuf, Unreachable> {
        if let Some(path) = self.cached(req) {
            return Ok(path);
        }
        let bytes = self.link.tts(req.clone()).await?;
        let (dir, path) = (self.dir.clone(), self.path(req));
        off_thread(move || write_atomically(&dir, &path, &bytes))
            .await
            .map_err(|e| Unreachable::Server(format!("saving audio: {e}")))?;
        Ok(self.path(req))
    }

    /// Fetches every request that isn't cached yet, in order, and reports
    /// progress as `(done, total)`. Stops at the first failure: the files
    /// already saved stay, so a retry resumes.
    #[cfg_attr(not(test), allow(dead_code))] // Offline downloads: Kabar Radio and Sinetron (PLAN §16 B).
    pub async fn download(
        &self,
        reqs: &[TtsRequest],
        mut progress: impl FnMut(usize, usize),
    ) -> Result<(), Unreachable> {
        for (i, req) in reqs.iter().enumerate() {
            self.fetch(req).await?;
            progress(i + 1, reqs.len());
        }
        Ok(())
    }

    /// How much of `reqs` is on the device.
    #[cfg_attr(not(test), allow(dead_code))] // Offline downloads: Kabar Radio and Sinetron (PLAN §16 B).
    pub fn usage(&self, reqs: &[TtsRequest]) -> Usage {
        reqs.iter().filter_map(|r| self.cached(r)).fold(Usage::default(), |u, p| Usage {
            files: u.files + 1,
            bytes: u.bytes + std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
        })
    }

    /// Frees the space `reqs` take. Lines shared with other episodes go too;
    /// they come back on the next play.
    #[cfg_attr(not(test), allow(dead_code))] // Offline downloads: Kabar Radio and Sinetron (PLAN §16 B).
    pub fn remove(&self, reqs: &[TtsRequest]) {
        for path in reqs.iter().filter_map(|r| self.cached(r)) {
            std::fs::remove_file(path).ok();
        }
    }
}

/// The default voice for lesson and review lines in `language` ("id", "jv").
pub fn default_voice(language: &str) -> Voice {
    if language == "jv" { Voice::JvSiti } else { Voice::IdGadis }
}

/// From a component: the neural voice for `text` as a playable URL, when it
/// is on the device or Kabar Cloud is reachable right now. `None` means use
/// the device's own voice; it never waits on a network that is down.
pub fn neural(
    text: &str,
    language: &str,
) -> Option<impl std::future::Future<Output = Option<String>> + use<>> {
    use dioxus::prelude::{ReadableExt, try_consume_context};
    let cache = try_consume_context::<AudioCache>()?;
    let req = line(default_voice(language), text, 0, 0);
    let live = try_consume_context::<crate::account::Cloud>().is_some_and(|c| {
        matches!(c.status.peek().sync, crate::cloud::SyncState::Live | crate::cloud::SyncState::CatchingUp)
    });
    if cache.cached(&req).is_none() && !live {
        return None;
    }
    Some(async move {
        match cache.fetch(&req).await {
            Ok(path) => data_url(&path).ok(),
            Err(e) => {
                eprintln!("kabar: neural voice: {e}");
                None
            }
        }
    })
}

/// One line in one voice.
pub fn line(voice: Voice, text: &str, pitch_percent: i8, rate_percent: i8) -> TtsRequest {
    TtsRequest { segments: vec![TtsSegment { voice, text: text.trim().into(), pitch_percent }], rate_percent }
}

/// Stable file key: the request's canonical JSON, hashed.
pub fn key(req: &TtsRequest) -> String {
    let json = serde_json::to_vec(req).unwrap_or_default();
    Sha256::digest(&json).iter().map(|b| format!("{b:02x}")).collect()
}

/// A `data:` URL the WebView's `<audio>` can play without file access.
pub fn data_url(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(format!("data:audio/mpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// Joins MP3 files into one track. Every file comes from the same Azure
/// output format (constant bitrate, same sample rate, no ID3 tags), so the
/// frames simply follow each other.
#[cfg_attr(not(test), allow(dead_code))] // Offline downloads: Kabar Radio and Sinetron (PLAN §16 B).
pub fn join(parts: &[PathBuf], out: &Path) -> std::io::Result<()> {
    let mut joined = Vec::new();
    for part in parts {
        joined.extend(std::fs::read(part)?);
    }
    let dir = out.parent().unwrap_or(Path::new("."));
    write_atomically(dir, out, &joined)
}

fn write_atomically(dir: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_follow_voice_pitch_rate_and_text() {
        let a = line(Voice::IdArdi, "Selamat siang!", 0, 0);
        assert_eq!(key(&a), key(&line(Voice::IdArdi, " Selamat siang! ", 0, 0)), "trimmed");
        for other in [
            line(Voice::IdGadis, "Selamat siang!", 0, 0),
            line(Voice::IdArdi, "Selamat siang!", -10, 0),
            line(Voice::IdArdi, "Selamat siang!", 0, -20),
            line(Voice::IdArdi, "Selamat pagi!", 0, 0),
        ] {
            assert_ne!(key(&a), key(&other));
        }
        assert_eq!(key(&a).len(), 64);
    }

    #[test]
    fn offline_plays_what_was_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path().join("audio"), CloudLink::default());
        let (hit, miss) =
            (line(Voice::JvSiti, "Kowe wis mangan?", 0, 0), line(Voice::JvDimas, "Durung.", 0, 0));
        write_atomically(&dir.path().join("audio"), &cache.path(&hit), b"ID3-free mp3").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        assert_eq!(runtime.block_on(cache.fetch(&hit)).unwrap(), cache.path(&hit));
        assert_eq!(runtime.block_on(cache.fetch(&miss)), Err(Unreachable::NotConfigured));
        let mut seen = Vec::new();
        runtime
            .block_on(cache.download(std::slice::from_ref(&hit), |done, total| seen.push((done, total))))
            .unwrap();
        assert_eq!(seen, [(1, 1)]);
        let both = [hit.clone(), miss];
        assert!(runtime.block_on(cache.download(&both, |_, _| {})).is_err(), "stops at the missing line");
        assert_eq!(cache.usage(&both), Usage { files: 1, bytes: 12 });
        assert!(data_url(&cache.path(&hit)).unwrap().starts_with("data:audio/mpeg;base64,"));
        cache.remove(&both);
        assert_eq!(cache.usage(&both), Usage::default());
    }

    #[test]
    fn episodes_join_into_one_track() {
        let dir = tempfile::tempdir().unwrap();
        let parts: Vec<PathBuf> = ["a", "b"].iter().map(|n| dir.path().join(format!("{n}.mp3"))).collect();
        std::fs::write(&parts[0], [1, 2]).unwrap();
        std::fs::write(&parts[1], [3]).unwrap();
        let out = dir.path().join("episodes/radio.bipa1.u01.mp3");
        join(&parts, &out).unwrap();
        assert_eq!(std::fs::read(out).unwrap(), [1, 2, 3]);
    }
}
