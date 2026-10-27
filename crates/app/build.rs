//! Builds the offline content pack (`batna_core::pack`) from
//! `content/index.yaml` and every `content/<chapter>/LNN.yaml`, and embeds it
//! with `include_bytes!`, the same pattern as Kabar's build.rs (adapted from
//! kabar-app main: crates/app/build.rs). A new lecture file needs no code
//! change. A file that doesn't parse is left out with a warning; CI's
//! `batna-tools validate` and `packs build --check` fail on it.

use std::path::PathBuf;

use batna_core::content::{Index, Lecture};
use batna_core::pack::Pack;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let content = root.join("content");
    println!("cargo:rerun-if-changed={}", content.display());

    let index_path = content.join("index.yaml");
    println!("cargo:rerun-if-changed={}", index_path.display());
    let index: Index =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&index_path).expect("content/index.yaml"))
            .expect("content/index.yaml parses");

    let mut files = Vec::new();
    for dir in std::fs::read_dir(&content).expect("content/ exists").flatten() {
        if !dir.path().is_dir() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", dir.path().display());
        for f in std::fs::read_dir(dir.path()).unwrap().flatten() {
            let p = f.path();
            if p.extension().is_some_and(|e| e == "yaml") {
                println!("cargo:rerun-if-changed={}", p.display());
                files.push(p);
            }
        }
    }
    files.sort();

    let mut lectures = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        match serde_yaml_ng::from_str::<Lecture>(&text) {
            Ok(l) => lectures.push(l),
            Err(e) => println!("cargo:warning={}: left out of the app: {e}", f.display()),
        }
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let pack = out.join("content.pack");
    std::fs::write(&pack, Pack::new(index, lectures).encode()).unwrap();
    std::fs::write(
        out.join("embedded.rs"),
        format!("pub const PACK: &[u8] = include_bytes!({:?});\n", pack.display().to_string()),
    )
    .unwrap();
}
