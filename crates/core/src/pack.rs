//! The offline content pack: `content/index.yaml` plus every written lecture,
//! as one DEFLATE-compressed JSON blob. `crates/app/build.rs` builds it from
//! the YAML and embeds it with `include_bytes!`; `batna-tools packs build`
//! writes the same bytes to `dist/packs/` and `--check`s them in CI.
//!
//! JSON rather than YAML so the app parses it fast on the phone, and rather
//! than a binary format because items use `#[serde(flatten)]` and untagged
//! enums, which only self-describing formats support.

use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec_with_limit;
use serde::{Deserialize, Serialize};

use crate::content::{Index, Lecture};

/// The first bytes of every pack. Bump the digit when the layout changes.
pub const MAGIC: &[u8; 8] = b"BATNAPK1";

/// Highest miniz level: slower to pack (only when content changes), not to unpack.
const LEVEL: u8 = 10;

/// An unpacked pack can't be bigger than this (the whole chapter is a few MB).
const MAX_UNPACKED: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    pub index: Index,
    /// Written lectures, in index order.
    pub lectures: Vec<Lecture>,
}

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("not a Batna content pack (bad header)")]
    Magic,
    #[error("the pack is corrupt: {0}")]
    Inflate(String),
    #[error("the pack's JSON doesn't match the content types: {0}")]
    Json(#[from] serde_json::Error),
}

impl Pack {
    /// Orders the lectures as the index lists them; lectures it doesn't list go last.
    pub fn new(index: Index, mut lectures: Vec<Lecture>) -> Self {
        let pos = |id: &str| index.lectures.iter().position(|e| e.id == id).unwrap_or(usize::MAX);
        lectures.sort_by(|a, b| pos(&a.lecture).cmp(&pos(&b.lecture)).then(a.lecture.cmp(&b.lecture)));
        Pack { index, lectures }
    }

    pub fn encode(&self) -> Vec<u8> {
        let json = serde_json::to_vec(self).expect("content types always serialize");
        let mut out = MAGIC.to_vec();
        out.extend(compress_to_vec(&json, LEVEL));
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PackError> {
        let body = bytes.strip_prefix(MAGIC.as_slice()).ok_or(PackError::Magic)?;
        let json = decompress_to_vec_with_limit(body, MAX_UNPACKED)
            .map_err(|e| PackError::Inflate(format!("{e:?}")))?;
        Ok(serde_json::from_slice(&json)?)
    }

    pub fn item_count(&self) -> usize {
        self.lectures.iter().map(|l| l.items.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Lecture {
        let text = include_str!("../../../fixtures/content/sample.yaml");
        serde_yaml_ng::from_str(text).unwrap()
    }

    fn index() -> Index {
        serde_yaml_ng::from_str(
            "schema: batna.content/v1\nlectures:\n  - { id: hb.L01, title: Liver Cirrhosis, pages: [1, 22], chunk: 10 }\n  - { id: hb.L02, title: Two, pages: [23, 36], chunk: 10 }\n",
        )
        .unwrap()
    }

    #[test]
    fn a_pack_round_trips_and_keeps_index_order() {
        let mut second = sample();
        second.lecture = "hb.L02".into();
        let pack = Pack::new(index(), vec![second, sample()]);
        assert_eq!(pack.lectures[0].lecture, "hb.L01");
        let bytes = pack.encode();
        assert!(bytes.starts_with(MAGIC));
        let back = Pack::decode(&bytes).unwrap();
        assert_eq!(back, pack);
        assert_eq!(back.item_count(), 18);
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(matches!(Pack::decode(b"PK\x03\x04"), Err(PackError::Magic)));
        let mut bad = MAGIC.to_vec();
        bad.extend([1, 2, 3]);
        assert!(matches!(Pack::decode(&bad), Err(PackError::Inflate(_))));
    }
}
