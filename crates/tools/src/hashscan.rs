//! Raw-text check for a YAML trap: in a plain (unquoted) scalar, " #"
//! starts a comment, so `text: no #iklan tag` silently loads as "no".
//! Content never uses trailing comments, so any " #" in a plain value is a
//! cut-off line. Quote the value instead.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::paths::Root;

/// Directories holding hand-written YAML.
const DIRS: &[&str] = &["content", "curriculum"];

fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else { return Ok(()) };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            yaml_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
            out.push(path);
        }
    }
    Ok(())
}

/// The plain scalar on a line (after `- ` markers and a `key: `), if any.
fn plain_value(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    while let Some(r) = rest.strip_prefix("- ") {
        rest = r.trim_start();
    }
    if let Some((key, value)) = rest.split_once(": ")
        && !key.contains(['"', '\'', '#', '[', '{'])
    {
        rest = value.trim_start();
    }
    match rest.chars().next() {
        None | Some('"' | '\'' | '|' | '>' | '[' | '{' | '#' | '&' | '*' | '!') => None,
        _ => Some(rest),
    }
}

/// Does this line open a block scalar (`key: |`, `- >-` …)?
fn opens_block(line: &str) -> bool {
    let t = line.trim().trim_end_matches(|c: char| c.is_ascii_digit() || c == '+' || c == '-');
    let Some(head) = t.strip_suffix('|').or_else(|| t.strip_suffix('>')) else { return false };
    head.is_empty() || head.ends_with(": ") || head.ends_with("- ")
}

/// 1-based line numbers whose plain value contains " #".
pub fn scan(text: &str) -> Vec<usize> {
    let mut hits = Vec::new();
    let mut block: Option<usize> = None;
    for (i, line) in text.lines().enumerate() {
        let indent = line.len() - line.trim_start().len();
        if let Some(b) = block {
            if line.trim().is_empty() || indent > b {
                continue;
            }
            block = None;
        }
        if line.trim_start().starts_with('#') {
            continue;
        }
        if opens_block(line) {
            block = Some(indent);
            continue;
        }
        if plain_value(line).is_some_and(|v| v.contains(" #")) {
            hits.push(i + 1);
        }
    }
    hits
}

/// Every `file:line` in the content tree with a comment-cut plain scalar.
pub fn check(root: &Root) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for dir in DIRS {
        yaml_files(&root.path().join(dir), &mut files)?;
    }
    files.sort();
    let mut issues = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file)?;
        let rel = file.strip_prefix(root.path()).unwrap_or(&file).display().to_string();
        for n in scan(&text) {
            issues.push(format!(
                "{rel}:{n}: \" #\" in an unquoted value starts a YAML comment; quote the value"
            ));
        }
    }
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::scan;

    #[test]
    fn flags_hash_in_plain_values_only() {
        let yaml = "\
# a comment line
text: Tidak ada tanda #iklan.
quoted: 'POV: makan #kuliner'
dq: \"ada #tag\"
lines:
  - speaker: Rina
    text: Lihat #promo ini!
  - plain list #item
body:
  en: |
    A block can say #iklan freely.
    - even # here
  ar: ok
flow: [a, b]
";
        assert_eq!(scan(yaml), vec![2, 7, 8]);
    }

    #[test]
    fn hash_without_space_is_fine() {
        assert!(scan("text: C# dan F#\nurl: a.com/#x\n").is_empty());
    }
}
