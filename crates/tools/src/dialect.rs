//! `kabar-tools dialect`: Surabaya (Arek) and Semarang forms left in the
//! Javanese track, which teaches only Yogyakarta and Madiun speech
//! (`content/javanese/dialect-guard.yaml`). Advisory until the J1, J3 and
//! J5 relocations land; then CI makes it an error.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use clap::Args as ClapArgs;
use serde_json::Value;

use crate::paths::Root;

#[derive(ClapArgs)]
pub struct Args {
    /// Print a GitHub Actions warning per file instead of failing.
    #[arg(long)]
    pub warn: bool,
}

const FILE: &str = "content/javanese/dialect-guard.yaml";

/// Every string inside a YAML value.
fn strings<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        Value::Object(o) => o.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

/// The listed forms found in `text` as whole words, in order of the list.
pub fn found<'a>(text: &str, forms: &'a [String]) -> Vec<&'a str> {
    let lower: Vec<char> = text.chars().collect();
    let word = |c: char| c.is_alphanumeric() || c == '\'';
    forms
        .iter()
        .filter(|f| {
            let f: Vec<char> = f.chars().collect();
            (0..lower.len()).any(|i| {
                lower[i..].starts_with(&f)
                    && (i == 0 || !word(lower[i - 1]))
                    && lower.get(i + f.len()).is_none_or(|c| !word(*c))
            })
        })
        .map(String::as_str)
        .collect()
}

fn yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(_) => return Ok(Vec::new()),
    };
    out.retain(|p| p.extension().is_some_and(|x| x == "yaml"));
    out.sort();
    Ok(out)
}

pub fn run(root: &Root, args: Args) -> Result<()> {
    let guard = kabar_content::yaml_value(&root.path().join(FILE)).map_err(|e| anyhow!("{e}"))?;
    let table: BTreeMap<String, String> = guard
        .get("forms")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
        .collect();
    let forms: Vec<String> = table.keys().cloned().collect();
    let allowed = |key: &str| guard.get("allow").and_then(|a| a.get(key)).is_some();
    let mut files = yaml_files(&root.path().join("content/lessons/jv"))?;
    files.extend(yaml_files(&root.path().join("content/radio/jv"))?);
    for f in yaml_files(&root.path().join("content/stories/sinetron"))? {
        let doc = kabar_content::yaml_value(&f).map_err(|e| anyhow!("{e}"))?;
        if doc.get("language").and_then(Value::as_str) == Some("jv") {
            files.push(f);
        }
    }
    let mut total = 0;
    for f in &files {
        let doc = kabar_content::yaml_value(f).map_err(|e| anyhow!("{e}"))?;
        let id = doc.get("id").and_then(Value::as_str).unwrap_or_default();
        let name = f.strip_prefix(root.path()).unwrap_or(f).display().to_string();
        if allowed(id) || allowed(&name) {
            continue;
        }
        let mut texts = Vec::new();
        strings(&doc, &mut texts);
        let mut hits: BTreeMap<&str, usize> = BTreeMap::new();
        for t in texts {
            for form in found(t, &forms) {
                *hits.entry(form).or_default() += 1;
            }
        }
        if hits.is_empty() {
            continue;
        }
        total += hits.values().sum::<usize>();
        let list: Vec<String> = hits.iter().map(|(w, n)| format!("{w}×{n} → {}", table[*w])).collect();
        if args.warn {
            println!("::warning file={name},title=Javanese outside Jogja/Madiun::{}", list.join("; "));
        } else {
            println!("{name}: {}", list.join("; "));
        }
    }
    println!("{total} Surabaya/Semarang form(s) in {} Javanese files checked", files.len());
    if total > 0 && !args.warn {
        anyhow::bail!("use the Mataraman forms in {FILE}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_whole_words_only() {
        let forms: Vec<String> = ["rek", "gak", "yok opo", "opo'o"].map(String::from).to_vec();
        assert_eq!(found("Piye kabare, rek?", &forms), ["rek"]);
        assert_eq!(found("Yok opo kabare?", &forms), Vec::<&str>::new(), "case matters: names stay");
        assert_eq!(found("yok opo, gak popo", &forms), ["gak", "yok opo"]);
        assert_eq!(found("rekan kerja, gakpapa", &forms), Vec::<&str>::new());
        assert_eq!(found("opo'o kok ngono", &forms), ["opo'o"]);
    }
}
