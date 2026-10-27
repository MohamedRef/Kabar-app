//! `batna-tools packs build [--out DIR] [--check]`: the offline content pack
//! (`batna_core::pack`), the same bytes `crates/app/build.rs` embeds.
//!
//! - `build` writes `dist/packs/batna-content.pack` and prints what is in it.
//! - `--check` (CI) writes nothing. It fails when a lecture file doesn't
//!   parse (the app would leave it out) or the pack doesn't round-trip.
//!
//! Also home of [`load`], which the Anki exporter shares.

use std::path::{Path, PathBuf};

use batna_core::content::{Index, Lecture};
use batna_core::pack::Pack;

/// `content/index.yaml` and every written lecture, in index order.
pub fn load(root: &Path) -> Result<(Index, Vec<Lecture>), Vec<String>> {
    let content = root.join("content");
    let index: Index = crate::read_yaml(&content.join("index.yaml")).map_err(|e| vec![e])?;
    let mut lectures = Vec::new();
    let mut problems = Vec::new();
    for entry in &index.lectures {
        let path = crate::lecture_path(&content, &entry.id);
        if path.exists() {
            match crate::read_yaml::<Lecture>(&path) {
                Ok(l) => lectures.push(l),
                Err(e) => problems.push(e),
            }
        }
    }
    if problems.is_empty() { Ok((index, lectures)) } else { Err(problems) }
}

pub fn run(root: &Path, args: &[String]) -> Result<String, Vec<String>> {
    if args.first().map(String::as_str) != Some("build") {
        return Err(vec!["usage: batna-tools packs build [--out DIR] [--check]".into()]);
    }
    let check = args.iter().any(|a| a == "--check");
    let out = crate::flag(args, "--out").map(PathBuf::from).unwrap_or_else(|| root.join("dist/packs"));

    let (index, lectures) = load(root)?;
    let pack = Pack::new(index, lectures);
    let bytes = pack.encode();
    match Pack::decode(&bytes) {
        Ok(back) if back == pack => {}
        Ok(_) => return Err(vec!["the pack doesn't round-trip: some content changes when packed".into()]),
        Err(e) => return Err(vec![format!("the pack doesn't decode: {e}")]),
    }
    let summary = format!(
        "{} lecture(s), {} items, {:.1} KB packed",
        pack.lectures.len(),
        pack.item_count(),
        bytes.len() as f64 / 1024.0
    );
    if check {
        return Ok(format!("pack OK: {summary}"));
    }
    let path = out.join("batna-content.pack");
    std::fs::create_dir_all(&out).map_err(|e| vec![format!("{}: {e}", out.display())])?;
    std::fs::write(&path, &bytes).map_err(|e| vec![format!("{}: {e}", path.display())])?;
    Ok(format!("{}: {summary}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_content_packs() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = run(&root, &["build".into(), "--check".into()]).unwrap();
        assert!(report.starts_with("pack OK"), "{report}");
    }

    #[test]
    fn a_broken_lecture_fails_the_check() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("content/hb")).unwrap();
        std::fs::write(
            dir.path().join("content/index.yaml"),
            "schema: batna.content/v1\nlectures:\n  - { id: hb.L01, title: Liver Cirrhosis, pages: [1, 22], chunk: 10 }\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("content/hb/L01.yaml"), "schema: batna.content/v1\nlecture: [")
            .unwrap();
        let problems = run(dir.path(), &["build".into(), "--check".into()]).unwrap_err();
        assert!(problems[0].contains("L01.yaml"), "{problems:?}");
    }
}
