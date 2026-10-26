//! Azure neural voices through Kabar Cloud's TTS proxy, cached forever on
//! the device (spec §20). Each file is the MP3 for one [`TtsRequest`], named
//! by the SHA-256 of the request, so the same line in the same voice is only
//! ever fetched once, and downloaded episodes play offline.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use kabar_core::story::mp3_seconds;
use kabar_protocol::cloud::{TTS_MP3_KBPS, TtsRequest, TtsSegment, Voice};
use sha2::{Digest, Sha256};

use crate::cloud::link::{CloudLink, Unreachable, off_thread};

#[derive(Clone)]
pub struct AudioCache {
    dir: PathBuf,
    link: CloudLink,
}

/// What a set of downloaded files takes on disk.
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

    fn track_files(&self, id: &str) -> (PathBuf, PathBuf) {
        let dir = self.dir.join("episodes");
        (dir.join(format!("{id}.mp3")), dir.join(format!("{id}.json")))
    }

    /// A downloaded episode, if it's on the device.
    pub fn track(&self, id: &str) -> Option<Track> {
        let (mp3, starts) = self.track_files(id);
        let starts: Vec<f64> = serde_json::from_slice(&std::fs::read(starts).ok()?).ok()?;
        mp3.is_file().then_some(Track { path: mp3, starts })
    }

    /// Downloads an episode for offline: every line (progress as `(done,
    /// total)`), then one joined track plus where each line starts in it.
    /// The single lines are dropped afterwards; the track holds them.
    pub async fn download_track(
        &self,
        id: &str,
        reqs: &[TtsRequest],
        progress: impl FnMut(usize, usize),
    ) -> Result<Track, Unreachable> {
        self.download(reqs, progress).await?;
        let parts: Vec<PathBuf> = reqs.iter().map(|r| self.path(r)).collect();
        let (mp3, starts_file) = self.track_files(id);
        let track = off_thread(move || -> std::io::Result<Track> {
            let mut starts = Vec::with_capacity(parts.len());
            let mut at = 0.0;
            for part in &parts {
                starts.push(at);
                at += mp3_seconds(std::fs::metadata(part)?.len(), TTS_MP3_KBPS);
            }
            join(&parts, &mp3)?;
            let dir = mp3.parent().unwrap_or(Path::new("."));
            write_atomically(dir, &starts_file, &serde_json::to_vec(&starts)?)?;
            for part in &parts {
                std::fs::remove_file(part).ok();
            }
            Ok(Track { path: mp3, starts })
        })
        .await
        .map_err(|e| Unreachable::Server(format!("saving the episode: {e}")))?;
        Ok(track)
    }

    pub fn remove_track(&self, id: &str) {
        let (mp3, starts) = self.track_files(id);
        std::fs::remove_file(mp3).ok();
        std::fs::remove_file(starts).ok();
    }

    /// What all downloaded episodes take.
    pub fn downloads(&self) -> Usage {
        let entries = std::fs::read_dir(self.dir.join("episodes")).into_iter().flatten().flatten();
        entries.filter(|e| e.path().extension().is_some_and(|x| x == "mp3")).fold(Usage::default(), |u, e| {
            Usage { files: u.files + 1, bytes: u.bytes + e.metadata().map(|m| m.len()).unwrap_or(0) }
        })
    }

    /// "Free up space": every downloaded episode goes; they can be
    /// downloaded again.
    pub fn remove_downloads(&self) {
        std::fs::remove_dir_all(self.dir.join("episodes")).ok();
    }
}

/// A downloaded episode: one MP3, and the second each line starts at.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub path: PathBuf,
    pub starts: Vec<f64>,
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
        assert!(cache.cached(&both[0]).is_some() && cache.cached(&both[1]).is_none());
        assert!(data_url(&cache.path(&hit)).unwrap().starts_with("data:audio/mpeg;base64,"));
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

    #[test]
    fn a_downloaded_episode_is_one_track_with_line_starts() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("audio");
        let cache = AudioCache::new(audio.clone(), CloudLink::default());
        let lines = [line(Voice::IdGadis, "Selamat pagi!", 0, 0), line(Voice::IdArdi, "Pagi, Gadis!", 0, 0)];
        // One second and half a second of 48 kbit/s audio.
        write_atomically(&audio, &cache.path(&lines[0]), &[0; 6_000]).unwrap();
        write_atomically(&audio, &cache.path(&lines[1]), &[1; 3_000]).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let track = runtime.block_on(cache.download_track("radio.bipa1.u01", &lines, |_, _| {})).unwrap();
        assert_eq!(track.starts, [0.0, 1.0]);
        assert_eq!(std::fs::metadata(&track.path).unwrap().len(), 9_000);
        assert_eq!(cache.track("radio.bipa1.u01"), Some(track));
        assert!(lines.iter().all(|l| cache.cached(l).is_none()), "the single lines live in the track now");
        assert_eq!(cache.downloads(), Usage { files: 1, bytes: 9_000 });
        cache.remove_downloads();
        assert_eq!(cache.track("radio.bipa1.u01"), None);
        assert_eq!(cache.downloads(), Usage::default());
    }
}
