//! The study content built into the app: build.rs packs it
//! (`batna_core::pack`) and this unpacks it once, on first use.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use batna_core::content::{Index, Lecture};
use batna_core::pack::Pack;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

pub struct Library {
    pub index: Index,
    /// Written lectures, by id (`hb.L01`).
    pub lectures: BTreeMap<String, Lecture>,
}

impl Library {
    pub fn get() -> &'static Library {
        static LIB: OnceLock<Library> = OnceLock::new();
        LIB.get_or_init(|| {
            // build.rs wrote the pack from types it just parsed, so this can't fail.
            let Pack { index, lectures } = Pack::decode(embedded::PACK).expect("embedded content pack");
            let lectures = lectures.into_iter().map(|l| (l.lecture.clone(), l)).collect();
            Library { index, lectures }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_file_parses() {
        let lib = Library::get();
        assert_eq!(lib.index.lectures.len(), 20);
        let written = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/hb"))
            .unwrap()
            .flatten()
            .filter(|f| f.path().extension().is_some_and(|e| e == "yaml"))
            .count();
        assert_eq!(lib.lectures.len(), written, "every lecture file made it into the pack");
    }
}
