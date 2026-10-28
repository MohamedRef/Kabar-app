//! Content files built into the app, compressed by `build.rs`: the tables
//! `LESSONS`, `TESTS`, `STORIES`, `RADIO`, `PERSONAS`, `GRAMMAR` and the gloss
//! batches (id → file), and the Bridge, frequency list, speech levels, cast
//! and curricula. Each file is unpacked only when it's used, and the text is
//! dropped once it's parsed, so the app neither carries ~25 MB of raw text in
//! its binary nor keeps it in memory.
//!
//! A content bundle from the update feed (`kabar_protocol::bundle`) replaces
//! all of it when it is at least as new as this build: [`use_bundle`] runs at
//! launch, before anything reads content, so a content change reaches the
//! learner with no app install. Its stylesheets ([`Css`]) apply only to the
//! exact build they were made with.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// Files of one kind, by id (a lesson id, a gloss batch's file stem).
#[derive(Clone, Copy)]
pub struct Table {
    pub name: &'static str,
    pub rows: &'static [(&'static str, &'static [u8])],
}

/// One built-in file.
#[derive(Clone, Copy)]
pub struct Single {
    pub name: &'static str,
    pub packed: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/content.rs"));

/// The update feed's bundle in use, by key (`LESSONS/<id>`, `BRIDGE_ID`,
/// `css/<path>`); every file still DEFLATE-packed.
struct Bundle {
    files: BTreeMap<String, Vec<u8>>,
    /// Its stylesheets were made with this very build.
    look: bool,
}

static BUNDLE: OnceLock<Bundle> = OnceLock::new();

/// Reads content from `entries` (a verified bundle) from now on. Call once,
/// at launch, before anything is read; later calls are ignored.
pub fn use_bundle(entries: Vec<(String, Vec<u8>)>, look: bool) {
    let _ = BUNDLE.set(Bundle { files: entries.into_iter().collect(), look });
}

fn bundle() -> Option<&'static Bundle> {
    BUNDLE.get()
}

/// The text of one packed file.
pub fn unpack(packed: &[u8]) -> String {
    // build.rs packed valid UTF-8, so neither can fail short of a broken build.
    let bytes = miniz_oxide::inflate::decompress_to_vec(packed).expect("embedded content is valid DEFLATE");
    String::from_utf8(bytes).expect("embedded content is UTF-8")
}

/// A bundle's file, or None when it doesn't unpack (the built-in copy is
/// used instead; `kabar-tools bundle build` checks every file, so this
/// takes a broken download).
fn unpack_bundled(packed: &[u8]) -> Option<String> {
    let bytes = miniz_oxide::inflate::decompress_to_vec(packed).ok()?;
    String::from_utf8(bytes).ok()
}

/// A table's rows in use: the bundle's when it has any, else the built-in ones.
pub fn rows(table: Table) -> Vec<(&'static str, &'static [u8])> {
    if let Some(b) = bundle() {
        let prefix = format!("{}/", table.name);
        let rows: Vec<_> = b
            .files
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| (&k[prefix.len()..], v.as_slice()))
            .collect();
        if !rows.is_empty() {
            return rows;
        }
    }
    table.rows.to_vec()
}

/// The text of a single file.
pub fn text(file: Single) -> String {
    bundle()
        .and_then(|b| b.files.get(file.name))
        .and_then(|p| unpack_bundled(p))
        .unwrap_or_else(|| unpack(file.packed))
}

/// The text of `id` in a table, if this build (or its bundle) ships it.
pub fn find(table: Table, id: &str) -> Option<String> {
    rows(table).into_iter().find(|(file, _)| *file == id).map(|(_, packed)| unpack_row(table, id, packed))
}

/// Every file of a table, unpacked: `(id, text)`.
pub fn all(table: Table) -> impl Iterator<Item = (&'static str, String)> {
    rows(table).into_iter().map(move |(id, packed)| (id, unpack_row(table, id, packed)))
}

/// A row's text; a bundled row that doesn't unpack falls back to the
/// built-in copy, or to nothing.
fn unpack_row(table: Table, id: &str, packed: &'static [u8]) -> String {
    if table.rows.iter().any(|(_, p)| std::ptr::eq(*p, packed)) {
        return unpack(packed);
    }
    unpack_bundled(packed).unwrap_or_else(|| {
        table.rows.iter().find(|(f, _)| *f == id).map(|(_, p)| unpack(p)).unwrap_or_default()
    })
}

/// One stylesheet: its path under `crates/app` and the built-in text.
#[derive(Clone, Copy)]
pub struct Css {
    pub path: &'static str,
    pub text: &'static str,
}

/// `css!("assets/main.css")`: a stylesheet under `crates/app`.
macro_rules! css {
    ($path:literal) => {
        $crate::embedded::Css {
            path: $path,
            text: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path)),
        }
    };
}
pub(crate) use css;

impl Css {
    /// The text in use: the bundle's copy when the bundle's look was made
    /// with this build, else the built-in one.
    pub fn text(self) -> &'static str {
        let Some(b) = bundle().filter(|b| b.look) else { return self.text };
        static TEXT: OnceLock<Mutex<BTreeMap<&'static str, &'static str>>> = OnceLock::new();
        let mut cache = TEXT.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
        cache.entry(self.path).or_insert_with(|| {
            let key = format!("{}{}", kabar_protocol::bundle::CSS_PREFIX, self.path);
            b.files.get(&key).and_then(|p| unpack_bundled(p)).map_or(self.text, |t| t.leak())
        })
    }
}

/// A `data:` URL of `bytes`.
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    use base64::Engine as _;
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// The `data:` URL of an embedded file (art, fonts), encoded the first time
/// it's shown and kept, so pictures nobody opens cost no memory.
pub fn cached_data_url(mime: &'static str, bytes: &'static [u8]) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};
    // Keyed by address: each embedded file is one static.
    static URLS: LazyLock<Mutex<HashMap<usize, &'static str>>> = LazyLock::new(Default::default);
    let mut urls = URLS.lock().unwrap_or_else(|e| e.into_inner());
    urls.entry(bytes.as_ptr() as usize).or_insert_with(|| data_url(mime, bytes).leak())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_files_on_disk() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let disk = |p: &str| std::fs::read_to_string(format!("{root}/{p}")).unwrap();
        assert_eq!(text(CURRICULUM_ID), disk("curriculum/indonesian.yaml"));
        assert_eq!(text(BRIDGE_ID), disk("content/bridge/id.jsonl"));
        assert_eq!(find(LESSONS, "bipa1.u01.l01").unwrap(), disk("content/lessons/id/bipa1.u01.l01.yaml"));
        assert!(find(LESSONS, "bipa8.u01.l01").is_none());
    }

    #[test]
    fn data_urls_are_encoded_once() {
        static PNG: &[u8] = b"\x89PNG";
        let a = cached_data_url("image/png", PNG);
        assert_eq!(a, "data:image/png;base64,iVBORw==");
        assert!(std::ptr::eq(a, cached_data_url("image/png", PNG)));
    }
}
