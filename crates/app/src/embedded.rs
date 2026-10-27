//! Content files built into the app, compressed by `build.rs`: `LESSONS`,
//! `TESTS`, `STORIES`, `RADIO`, `PERSONAS` and `GRAMMAR` (id → file), the
//! gloss batches, and the Bridge, frequency list, speech levels and curricula.
//! Each file is unpacked only when it's used, and the text is dropped once
//! it's parsed, so the app neither carries ~25 MB of raw text in its binary
//! nor keeps it in memory.

include!(concat!(env!("OUT_DIR"), "/content.rs"));

/// The text of one packed file.
pub fn unpack(packed: &[u8]) -> String {
    // build.rs packed valid UTF-8, so neither can fail short of a broken build.
    let bytes = miniz_oxide::inflate::decompress_to_vec(packed).expect("embedded content is valid DEFLATE");
    String::from_utf8(bytes).expect("embedded content is UTF-8")
}

/// The text of `id` in a table, if this build ships it.
pub fn find(table: &[(&str, &[u8])], id: &str) -> Option<String> {
    table.iter().find(|(file, _)| *file == id).map(|(_, packed)| unpack(packed))
}

/// Every file of a table, unpacked: `(id, text)`.
pub fn all(table: &'static [(&'static str, &'static [u8])]) -> impl Iterator<Item = (&'static str, String)> {
    table.iter().map(|(id, packed)| (*id, unpack(packed)))
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
        assert_eq!(unpack(CURRICULUM_ID), disk("curriculum/indonesian.yaml"));
        assert_eq!(unpack(BRIDGE_ID), disk("content/bridge/id.jsonl"));
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
