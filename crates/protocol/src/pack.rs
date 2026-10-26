//! Offline content packs (SPEC §7, §18, §20; PLAN §7–8): one pack per
//! curriculum unit, so a unit's lessons and audio work with no network.
//!
//! A pack is described by a [`PackManifest`] (committed under
//! `content/packs/<lang>/<unit>.json` by `kabar-tools packs build`) and
//! shipped as `<unit>.tar` on a GitHub release:
//!
//! ```text
//! manifest.json          the PackManifest, with every audio entry's sha256/size
//! audio/<key>.mp3        one clip per AudioEntry, named by its key
//! ```
//!
//! [`PackIndex`] (`index.json` next to the tars) lists every pack, its
//! version and the tar's hash, so a device can tell what is new without
//! downloading anything else.
//!
//! Audio is content-addressed twice: an entry's `key` is the SHA-256 of what
//! was asked for (voice, normalized text, SSML parameters; see
//! `kabar_core::packs::audio_key`), so the same line in two units is one
//! clip; its `sha256` is the hash of the MP3 bytes, which the device checks
//! before trusting a download.

use serde::{Deserialize, Serialize};

use crate::cloud::Voice;

/// Bumped when the manifest or tar layout changes incompatibly. A device
/// ignores packs with a newer format (and keeps its old ones).
pub const PACK_FORMAT: u32 = 1;
/// Versions the audio-key recipe; part of every key.
pub const AUDIO_KEY_VERSION: &str = "kabar.audio/v1";
/// Azure output format of every pre-generated clip (same as the TTS proxy).
pub const AUDIO_OUTPUT_FORMAT: &str = "audio-24khz-48kbitrate-mono-mp3";
pub const AUDIO_MIME: &str = "audio/mpeg";
/// File extension of a clip in the tar and the device cache.
pub const AUDIO_EXT: &str = "mp3";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackManifest {
    /// [`PACK_FORMAT`] when written.
    pub format: u32,
    /// `<language>/<unit>`, e.g. `id/bipa1.u03`.
    pub id: String,
    /// `id` or `jv`.
    pub language: String,
    /// Curriculum unit id, e.g. `bipa1.u03`.
    pub unit: String,
    /// Starts at 1; bumped whenever `content_hash` changes.
    pub version: u32,
    /// SHA-256 over the lessons' hashes and the audio keys: same content,
    /// same hash, so rebuilding unchanged content never bumps the version.
    pub content_hash: String,
    pub lessons: Vec<PackLesson>,
    pub audio: Vec<AudioEntry>,
    pub totals: PackTotals,
}

/// A lesson in the pack. Its YAML is still built into the app; the hash lets
/// a later version deliver newer lesson text through the pack (delta sync).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackLesson {
    pub id: String,
    /// SHA-256 of the lesson's YAML file.
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioEntry {
    /// SHA-256 of (key version, output format, voice, normalized text,
    /// SSML parameters): the clip's name in the tar and the device cache.
    pub key: String,
    /// The target-language text, normalized.
    pub text: String,
    pub voice: Voice,
    /// Billable characters (what Azure counts against the free quota).
    pub chars: u32,
    /// SHA-256 of the MP3 bytes; `None` until the clip is synthesized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Size of the MP3 in bytes; `None` until synthesized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// [`AUDIO_MIME`].
    pub mime: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackTotals {
    pub lessons: u32,
    pub clips: u32,
    /// Billable characters of all clips (to plan the monthly TTS quota).
    pub chars: u64,
    /// Bytes of the clips synthesized so far.
    pub audio_bytes: u64,
    /// Clips not synthesized yet.
    pub missing: u32,
}

/// `index.json`: every pack the release offers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PackIndex {
    pub format: u32,
    pub packs: Vec<PackSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackSummary {
    pub id: String,
    pub language: String,
    pub unit: String,
    /// The unit's town or station on the Journey Map (e.g. `Bandung`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<String>,
    pub version: u32,
    pub content_hash: String,
    pub lessons: u32,
    pub clips: u32,
    pub chars: u64,
    pub audio_bytes: u64,
    /// Clips not synthesized yet: a pack with missing clips isn't bundled.
    pub missing: u32,
    /// File name of the tar on the release, e.g. `bipa1.u03.tar`.
    pub file: String,
    /// SHA-256 and size of the tar, once `packs bundle` built it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
}

impl PackSummary {
    /// The tar exists on the release and can be downloaded.
    pub fn downloadable(&self) -> bool {
        self.file_sha256.is_some() && self.missing == 0 && self.clips > 0
    }
}

impl PackIndex {
    pub fn get(&self, id: &str) -> Option<&PackSummary> {
        self.packs.iter().find(|p| p.id == id)
    }
}

/// `<language>/<unit>`.
pub fn pack_id(language: &str, unit: &str) -> String {
    format!("{language}/{unit}")
}

/// Name of a pack's tar on the release.
pub fn pack_file(unit: &str) -> String {
    format!("{unit}.tar")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsynthesized_entries_omit_hashes() {
        let e = AudioEntry {
            key: "k".into(),
            text: "Apa kabar?".into(),
            voice: Voice::IdGadis,
            chars: 10,
            sha256: None,
            size: None,
            mime: AUDIO_MIME.into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("sha256") && !json.contains("size"));
        assert!(json.contains(r#""voice":"id_gadis""#));
        assert_eq!(serde_json::from_str::<AudioEntry>(&json).unwrap(), e);
    }

    #[test]
    fn only_bundled_complete_packs_download() {
        let mut s = PackSummary {
            id: pack_id("id", "bipa1.u01"),
            language: "id".into(),
            unit: "bipa1.u01".into(),
            stop: None,
            version: 1,
            content_hash: "h".into(),
            lessons: 5,
            clips: 3,
            chars: 30,
            audio_bytes: 0,
            missing: 0,
            file: pack_file("bipa1.u01"),
            file_sha256: None,
            file_size: None,
        };
        assert_eq!(s.id, "id/bipa1.u01");
        assert_eq!(s.file, "bipa1.u01.tar");
        assert!(!s.downloadable());
        s.file_sha256 = Some("x".into());
        assert!(s.downloadable());
        s.missing = 1;
        assert!(!s.downloadable());
    }
}
