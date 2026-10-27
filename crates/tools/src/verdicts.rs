//! Mohamed's ✓/✎/✗ export from the app (`batna.verdicts/v1`, chunk 3).
//!
//! `batna-tools verdicts FILE [--approve]` prints a fix list for each lecture
//! (✗ wrong and ✎ edited items, with his note and the fields he changed),
//! and with `--approve` adds his ✓ ids to `content/approved.txt`.
//!
//! The Anki exporter reads the same file (`anki --verdicts FILE`): ✗ items are
//! left out and ✎ items use his edited text.
//!
//! Mirrors `batna_core::review::verify` until chunk 3's types land there.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use batna_core::content::Item;
use serde::Deserialize;

pub const SCHEMA: &str = "batna.verdicts/v1";

#[derive(Debug, Clone, Deserialize)]
pub struct Verdicts {
    pub schema: String,
    #[serde(default)]
    pub exported: Option<String>,
    #[serde(default)]
    pub verdicts: Vec<VerdictEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VerdictEntry {
    pub id: String,
    pub verdict: Verdict,
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// The fields he overrode, keyed like the content YAML.
    #[serde(default)]
    pub edit: BTreeMap<String, FieldValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Approved,
    Edited,
    Wrong,
}

#[derive(Debug, Clone, PartialEq, Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum FieldValue {
    Text(String),
    List(Vec<String>),
}

impl Verdicts {
    pub fn read(path: &Path) -> Result<Self, String> {
        let v: Verdicts = crate::read_yaml(path)?;
        if v.schema != SCHEMA {
            return Err(format!("{}: schema is {:?}, expected {SCHEMA:?}", path.display(), v.schema));
        }
        Ok(v)
    }

    /// The latest verdict for each item (the export may hold several).
    pub fn latest(&self) -> BTreeMap<&str, &VerdictEntry> {
        let mut out: BTreeMap<&str, &VerdictEntry> = BTreeMap::new();
        for v in &self.verdicts {
            match out.get(v.id.as_str()) {
                Some(old) if old.at > v.at => {}
                _ => {
                    out.insert(&v.id, v);
                }
            }
        }
        out
    }
}

/// `item` with his edited fields applied. Fields that don't fit the item's
/// kind are reported, and the item is kept as written.
pub fn apply_edit(item: &Item, edit: &BTreeMap<String, FieldValue>) -> Result<Item, String> {
    let mut value = serde_json::to_value(item).map_err(|e| e.to_string())?;
    let map = value.as_object_mut().expect("an item serializes to a map");
    for (key, v) in edit {
        if matches!(key.as_str(), "id" | "kind" | "status") {
            return Err(format!("{}: the edit changes `{key}`, which only the YAML may change", item.id));
        }
        let kind = item.body.kind();
        if !batna_core::content::allowed_keys(kind).is_some_and(|k| k.contains(&key.as_str()))
            && key != "topic"
        {
            return Err(format!("{}: a {kind} has no field `{key}`", item.id));
        }
        map.insert(key.clone(), serde_json::to_value(v).map_err(|e| e.to_string())?);
    }
    serde_json::from_value(value)
        .map_err(|e| format!("{}: the edit doesn't fit a {}: {e}", item.id, item.body.kind()))
}

pub fn run(root: &Path, args: &[String]) -> Result<String, Vec<String>> {
    let Some(file) = args.first().filter(|a| !a.starts_with("--")) else {
        return Err(vec!["usage: batna-tools verdicts FILE [--approve]".into()]);
    };
    let verdicts = Verdicts::read(Path::new(file)).map_err(|e| vec![e])?;
    let (_, lectures) = crate::packs::load(root)?;
    let items: BTreeMap<&str, (&str, &Item)> = lectures
        .iter()
        .flat_map(|l| l.items.iter().map(move |i| (i.id.as_str(), (l.lecture.as_str(), i))))
        .collect();

    let latest = verdicts.latest();
    let mut fixes: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut approved = Vec::new();
    let mut unknown = Vec::new();
    for (id, v) in &latest {
        let Some((lecture, item)) = items.get(id) else {
            unknown.push(id.to_string());
            continue;
        };
        let mut line = match v.verdict {
            Verdict::Approved => {
                approved.push(id.to_string());
                continue;
            }
            Verdict::Wrong => format!("✗ {id} ({}, p. {}) marked wrong", item.body.kind(), item.src.page),
            Verdict::Edited => format!("✎ {id} ({}, p. {}) edited", item.body.kind(), item.src.page),
        };
        if let Some(note) = v.note.as_deref().filter(|n| !n.trim().is_empty()) {
            write!(line, "\n    note: {}", note.trim()).unwrap();
        }
        for (key, value) in &v.edit {
            let shown = match value {
                FieldValue::Text(t) => t.clone(),
                FieldValue::List(l) => l.join(" | "),
            };
            write!(line, "\n    {key} → {shown}").unwrap();
        }
        if v.verdict == Verdict::Edited
            && let Err(e) = apply_edit(item, &v.edit)
        {
            write!(line, "\n    (can't apply: {e})").unwrap();
        }
        fixes.entry(lecture).or_default().push(line);
    }

    let mut out = String::new();
    if let Some(when) = &verdicts.exported {
        writeln!(out, "From the app export of {when}\n").unwrap();
    }
    if fixes.is_empty() {
        out.push_str("No ✎ or ✗ items: nothing to fix.\n");
    }
    for (lecture, lines) in &fixes {
        writeln!(out, "## {lecture}: {} to fix", lines.len()).unwrap();
        for l in lines {
            writeln!(out, "{l}").unwrap();
        }
        out.push('\n');
    }
    if !unknown.is_empty() {
        writeln!(out, "Not in the content any more (renumbered or not merged yet): {}", unknown.join(", "))
            .unwrap();
    }
    if args.iter().any(|a| a == "--approve") {
        let added = add_approved(&root.join("content/approved.txt"), &approved).map_err(|e| vec![e])?;
        write!(out, "content/approved.txt: {added} new approval(s) of {} ✓", approved.len()).unwrap();
    } else {
        write!(out, "{} ✓ approved (add them to content/approved.txt with --approve)", approved.len())
            .unwrap();
    }
    Ok(out)
}

/// Appends the ids not already in the file; returns how many were new.
fn add_approved(path: &Path, ids: &[String]) -> Result<usize, String> {
    let have: BTreeSet<String> = crate::read_approved(path);
    let new: Vec<&String> = ids.iter().filter(|i| !have.contains(*i)).collect();
    if new.is_empty() {
        return Ok(0);
    }
    let mut text = std::fs::read_to_string(path).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for id in &new {
        writeln!(text, "{id}").unwrap();
    }
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(new.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORT: &str = r#"
schema: batna.verdicts/v1
exported: "2026-11-02T10:00:00Z"
verdicts:
  - { id: hb.L01.001, verdict: approved, at: "2026-11-02T09:00:00Z" }
  - { id: hb.L01.003, verdict: approved, at: "2026-11-01T09:00:00Z" }
  - { id: hb.L01.003, verdict: wrong, at: "2026-11-02T09:05:00Z", note: "B is 7-9, recount" }
  - { id: hb.L01.004, verdict: edited, at: "2026-11-02T09:06:00Z", edit: { points: [Recurrent bleeding, SBP], prompt: "Enumerate two." } }
  - { id: hb.L01.099, verdict: wrong, at: "2026-11-02T09:07:00Z" }
"#;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let hb = dir.path().join("content/hb");
        std::fs::create_dir_all(&hb).unwrap();
        std::fs::write(
            dir.path().join("content/index.yaml"),
            "schema: batna.content/v1\nlectures:\n  - { id: hb.L01, title: Liver Cirrhosis, pages: [1, 22], chunk: 10 }\n",
        )
        .unwrap();
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::copy(root.join("fixtures/content/sample.yaml"), hb.join("L01.yaml")).unwrap();
        std::fs::write(dir.path().join("content/approved.txt"), "# header\nhb.L01.009").unwrap();
        std::fs::write(dir.path().join("export.yaml"), EXPORT).unwrap();
        dir
    }

    #[test]
    fn fix_list_groups_by_lecture_and_keeps_the_latest_verdict() {
        let dir = tree();
        let file = dir.path().join("export.yaml").display().to_string();
        let out = run(dir.path(), &[file]).unwrap();
        assert!(out.contains("## hb.L01: 2 to fix"), "{out}");
        assert!(out.contains("✗ hb.L01.003 (mcq, p. 14) marked wrong\n    note: B is 7-9, recount"), "{out}");
        assert!(out.contains("points → Recurrent bleeding | SBP"), "{out}");
        assert!(
            out.contains("Not in the content any more (renumbered or not merged yet): hb.L01.099"),
            "{out}"
        );
        assert!(out.ends_with("1 ✓ approved (add them to content/approved.txt with --approve)"), "{out}");
    }

    #[test]
    fn approve_appends_only_new_ids() {
        let dir = tree();
        let file = dir.path().join("export.yaml").display().to_string();
        run(dir.path(), &[file.clone(), "--approve".into()]).unwrap();
        let out = run(dir.path(), &[file, "--approve".into()]).unwrap();
        assert!(out.ends_with("0 new approval(s) of 1 ✓"), "{out}");
        let text = std::fs::read_to_string(dir.path().join("content/approved.txt")).unwrap();
        assert_eq!(text, "# header\nhb.L01.009\nhb.L01.001\n");
    }

    #[test]
    fn edits_apply_to_the_item_or_say_why_not() {
        let lec: batna_core::content::Lecture =
            serde_yaml_ng::from_str(include_str!("../../../fixtures/content/sample.yaml")).unwrap();
        let card = &lec.items[0];
        let edit = BTreeMap::from([("back".to_string(), FieldValue::Text("Fibrosis plus nodules.".into()))]);
        let fixed = apply_edit(card, &edit).unwrap();
        assert!(
            matches!(&fixed.body, batna_core::content::Body::Card { back, .. } if back == "Fibrosis plus nodules.")
        );
        let bad = BTreeMap::from([("answer".to_string(), FieldValue::Text("x".into()))]);
        assert!(apply_edit(&lec.items[2], &bad).unwrap_err().contains("doesn't fit a mcq"));
    }
}
