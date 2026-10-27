//! batna-tools: content checks for Batna.
//!
//! ```text
//! batna-tools validate [--root DIR]   check content/ against the rules (SPLIT.md §4–5)
//! batna-tools anki [--lecture hb.L01] [--verdicts FILE] [--out DIR]   Anki deck (docs/ANKI.md)
//! batna-tools packs build [--out DIR] [--check]   the offline content pack
//! batna-tools verdicts FILE [--approve]   fix list from the app's ✎/✗ export
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use batna_core::content::{Index, Lecture, SCHEMA};
use batna_core::validate::{self, Context};

mod anki;
mod packs;
mod verdicts;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = match args.iter().position(|a| a == "--root") {
        Some(i) => match args.get(i + 1) {
            Some(r) => PathBuf::from(r),
            None => return usage(),
        },
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    };
    match args.first().map(String::as_str) {
        Some("validate") => match validate_all(&root) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(problems) => {
                for p in &problems {
                    eprintln!("{p}");
                }
                eprintln!("{} problem(s)", problems.len());
                ExitCode::FAILURE
            }
        },
        Some("anki") => report(anki::run(&root, &args[1..])),
        Some("packs") => report(packs::run(&root, &args[1..])),
        Some("verdicts") => report(verdicts::run(&root, &args[1..])),
        _ => usage(),
    }
}

fn report(result: Result<String, Vec<String>>) -> ExitCode {
    match result {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(problems) => {
            for p in &problems {
                eprintln!("{p}");
            }
            ExitCode::FAILURE
        }
    }
}

/// The value after `name`, if given.
fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: batna-tools <validate | anki | packs build | verdicts FILE> [--root DIR] (see the top of crates/tools/src/main.rs)"
    );
    ExitCode::from(2)
}

/// Checks `content/index.yaml` and every `content/<chapter>/LNN.yaml`.
/// Returns a one-line summary, or every problem found.
pub fn validate_all(root: &Path) -> Result<String, Vec<String>> {
    let mut problems = Vec::new();
    let content = root.join("content");
    let index: Index = match read_yaml(&content.join("index.yaml")) {
        Ok(i) => i,
        Err(e) => return Err(vec![e]),
    };
    if index.schema != SCHEMA {
        problems.push(format!("content/index.yaml: schema must be {SCHEMA}"));
    }
    let approved = read_approved(&content.join("approved.txt"));

    let mut items = 0;
    let mut files = 0;
    let mut listed = BTreeSet::new();
    for entry in &index.lectures {
        listed.insert(entry.id.clone());
        let path = lecture_path(&content, &entry.id);
        if !path.exists() {
            continue; // not written yet
        }
        files += 1;
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let lec: Lecture = match serde_yaml_ng::from_str(&text) {
            Ok(l) => l,
            Err(e) => {
                problems.push(format!("{}: {e}", rel(root, &path)));
                continue;
            }
        };
        if lec.lecture != entry.id {
            problems.push(format!(
                "{}: lecture is {}, the file name says {}",
                rel(root, &path),
                lec.lecture,
                entry.id
            ));
        }
        if lec.book.pages != entry.pages {
            problems.push(format!(
                "{}: book pages {:?} differ from content/index.yaml {:?}",
                rel(root, &path),
                lec.book.pages,
                entry.pages
            ));
        }
        let cx = Context { raw_keys: raw_keys(&text), approved: Some(&approved) };
        for p in validate::lecture(&lec, &cx) {
            problems.push(format!("{}: {p}", rel(root, &path)));
        }
        items += lec.items.len();
    }
    // Lecture files that the index doesn't list.
    if let Ok(dirs) = std::fs::read_dir(&content) {
        for dir in dirs.flatten().filter(|d| d.path().is_dir()) {
            let chapter = dir.file_name().to_string_lossy().to_string();
            for f in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
                let name = f.file_name().to_string_lossy().to_string();
                if let Some(stem) = name.strip_suffix(".yaml") {
                    let id = format!("{chapter}.{stem}");
                    if !listed.contains(&id) {
                        problems.push(format!("content/{chapter}/{name}: not listed in content/index.yaml"));
                    }
                }
            }
        }
    }
    if problems.is_empty() {
        Ok(format!("content OK: {files} of {} lectures written, {items} items", index.lectures.len()))
    } else {
        Err(problems)
    }
}

/// `hb.L01` → `content/hb/L01.yaml`.
fn lecture_path(content: &Path, id: &str) -> PathBuf {
    let (chapter, num) = id.split_once('.').unwrap_or((id, ""));
    content.join(chapter).join(format!("{num}.yaml"))
}

fn read_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_yaml_ng::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// One item id per line; `#` starts a comment. Missing file = nothing approved.
fn read_approved(path: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// The keys written on each item, by id, so typos like `anwser:` are caught.
fn raw_keys(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(doc) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(text) else {
        return out;
    };
    let Some(items) = doc.get("items").and_then(|i| i.as_sequence()) else {
        return out;
    };
    for item in items {
        let Some(map) = item.as_mapping() else {
            continue;
        };
        let Some(id) = map.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let keys = map.keys().filter_map(|k| k.as_str().map(String::from)).collect();
        out.insert(id.to_string(), keys);
    }
    out
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn repo_content_is_valid() {
        if let Err(problems) = validate_all(&root()) {
            panic!("{problems:#?}");
        }
    }

    #[test]
    fn a_tree_with_problems_is_reported() {
        let dir = std::env::temp_dir().join(format!("batna-validate-{}", std::process::id()));
        let hb = dir.join("content/hb");
        std::fs::create_dir_all(&hb).unwrap();
        std::fs::write(
            dir.join("content/index.yaml"),
            "schema: batna.content/v1\nlectures:\n  - { id: hb.L01, title: Liver Cirrhosis, pages: [1, 22], chunk: 10 }\n",
        )
        .unwrap();
        let sample = std::fs::read_to_string(root().join("fixtures/content/sample.yaml")).unwrap();
        std::fs::write(
            hb.join("L01.yaml"),
            sample.replace("front: Anatomical", "fornt: x\n    front: Anatomical"),
        )
        .unwrap();
        std::fs::write(hb.join("L99.yaml"), "").unwrap();
        let problems = validate_all(&dir).unwrap_err();
        assert!(problems.iter().any(|p| p.contains("unknown key `fornt`")), "{problems:#?}");
        assert!(problems.iter().any(|p| p.contains("L99.yaml: not listed")), "{problems:#?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
