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
}
