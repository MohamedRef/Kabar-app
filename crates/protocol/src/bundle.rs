//! The content bundle: everything the app builds in from `content/` and
//! `curriculum/` (lessons, tests, stories, Radio, personas, grammar, glosses,
//! the Bridge, the frequency list, speech levels, the cast and both
//! curricula), plus its stylesheets, as one file on the update feed. A
//! device that runs the app build the bundle came from, or an older one,
//! reads its content from the newest bundle instead of the copy built in, so
//! a content or look change needs no app install (`docs/RELEASE.md` §5).
//!
//! Layout: [`MAGIC`], then one record per file: the key's length (u32 LE),
//! the key (UTF-8), the data's length (u32 LE), the data. The data is the
//! file DEFLATE-compressed (raw, miniz level 10), exactly as `build.rs`
//! embeds it, so the app treats both alike. Keys are those of
//! [`source_files`] (`LESSONS/bipa1.u01.l01`, `BRIDGE_ID`) and
//! [`CSS_PREFIX`]`<path under crates/app>` for stylesheets.

include!("bundle_sources.rs");

/// The first bytes of every bundle. Bump the digit when the layout changes.
pub const MAGIC: &[u8; 8] = b"KABARCB1";

/// A bundle is never bigger than this (the whole course is ~8 MB packed).
pub const MAX_BYTES: u64 = 96 * 1024 * 1024;

/// No file in a bundle unpacks to more than this (the largest is ~4 MB).
pub const MAX_FILE_UNPACKED: usize = 16 * 1024 * 1024;

/// All of a bundle's files together never unpack to more than this
/// (the whole course is ~30 MB), so a hostile or broken bundle can't
/// exhaust a phone's memory.
pub const MAX_UNPACKED: u64 = 256 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum BundleError {
    /// Not a Kabar content bundle, or a layout this build can't read.
    Magic,
    /// Cut short or garbled.
    Corrupt,
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BundleError::Magic => "not a Kabar content bundle",
            BundleError::Corrupt => "the content bundle is corrupt",
        })
    }
}

impl std::error::Error for BundleError {}

pub fn encode(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    for (key, data) in entries {
        for part in [key.as_bytes(), data.as_slice()] {
            out.extend((part.len() as u32).to_le_bytes());
            out.extend(part);
        }
    }
    out
}

pub fn decode(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, BundleError> {
    let mut rest = bytes.strip_prefix(MAGIC.as_slice()).ok_or(BundleError::Magic)?;
    let take = |rest: &mut &[u8]| -> Result<Vec<u8>, BundleError> {
        let (len, tail) = rest.split_first_chunk::<4>().ok_or(BundleError::Corrupt)?;
        let len = u32::from_le_bytes(*len) as usize;
        let part = tail.get(..len).ok_or(BundleError::Corrupt)?.to_vec();
        *rest = &tail[len..];
        Ok(part)
    };
    let mut out = Vec::new();
    while !rest.is_empty() {
        let key = String::from_utf8(take(&mut rest)?).map_err(|_| BundleError::Corrupt)?;
        let data = take(&mut rest)?;
        out.push((key, data));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_round_trips_and_rejects_garbage() {
        let entries =
            vec![("LESSONS/bipa1.u01.l01".to_string(), vec![1, 2, 3]), ("CAST".to_string(), Vec::new())];
        let bytes = encode(&entries);
        assert_eq!(decode(&bytes).unwrap(), entries);
        assert_eq!(decode(b"PK\x03\x04"), Err(BundleError::Magic));
        assert_eq!(decode(&bytes[..bytes.len() - 1]), Err(BundleError::Corrupt));
    }

    #[test]
    fn the_repo_sources_are_all_there() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for s in SOURCES {
            let files = source_files(&root, *s);
            assert!(!files.is_empty() && files.iter().all(|(_, p)| p.is_file()), "{}", s.name());
        }
        let lessons = source_files(&root, SOURCES[0]);
        assert!(lessons.iter().any(|(k, _)| k == "LESSONS/bipa1.u01.l01"));
    }
}
