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
    report_to(root, args, &mut std::io::stdout().lock())
}

pub fn report_to(root: &Root, args: Args, out: &mut dyn std::io::Write) -> Result<()> {
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
            writeln!(out, "::warning file={name},title=Javanese outside Jogja/Madiun::{}", list.join("; "))?;
        } else {
            writeln!(out, "{name}: {}", list.join("; "))?;
        }
    }
    writeln!(out, "{total} Surabaya/Semarang form(s) in {} Javanese files checked", files.len())?;
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

    fn tree() -> crate::testkit::Sandbox {
        let s = crate::testkit::Sandbox::empty();
        s.write(
            FILE,
            "forms:\n  rek: \"mas, mbak\"\n  gak: ora\nallow:\n  j1.u01.l02: a Surabaya character speaks\n  content/radio/jv/r2.yaml: quoted on purpose\n",
        );
        s.write(
            "content/lessons/jv/j1.u01.l01.yaml",
            "id: j1.u01.l01\ndialogue:\n  - text: Piye kabare, rek?\n  - text: Aku gak ngerti.\n",
        );
        s.write("content/lessons/jv/j1.u01.l02.yaml", "id: j1.u01.l02\ntext: rek\n");
        s.write("content/radio/jv/r1.yaml", "id: r1\nlines: [Sugeng enjing]\n");
        s.write("content/radio/jv/r2.yaml", "id: r2\nlines: [gak]\n");
        s.write(
            "content/stories/sinetron/e1.yaml",
            "id: e1\nlanguage: jv\nlines: [\"rek, piye?\", ayo rek, 3]\n",
        );
        s.write("content/stories/sinetron/e2.yaml", "id: e2\nlanguage: id\nlines: [gak apa-apa]\n");
        s
    }

    #[test]
    fn reports_forms_outside_jogja_and_madiun_per_file() {
        let s = tree();
        let mut out = Vec::new();
        let err = report_to(&s.root(), Args { warn: false }, &mut out).unwrap_err();
        assert!(err.to_string().contains("Mataraman"), "{err}");
        let text = crate::testkit::text(out);
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            [
                "content/lessons/jv/j1.u01.l01.yaml: gak×1 → ora; rek×1 → mas, mbak",
                "content/stories/sinetron/e1.yaml: rek×2 → mas, mbak",
                "4 Surabaya/Semarang form(s) in 5 Javanese files checked",
            ],
            "allowed files and the Indonesian episode are skipped"
        );

        let mut out = Vec::new();
        report_to(&s.root(), Args { warn: true }, &mut out).unwrap();
        let text = crate::testkit::text(out);
        assert!(
            text.starts_with(
                "::warning file=content/lessons/jv/j1.u01.l01.yaml,title=Javanese outside Jogja/Madiun::gak×1 → ora"
            ),
            "{text}"
        );
        assert!(s.cli(&["dialect"]).is_err());
    }

    #[test]
    fn a_clean_track_passes_and_a_missing_guard_fails() {
        let s = tree();
        s.write(FILE, "forms:\n  cak: mas\n");
        let mut out = Vec::new();
        report_to(&s.root(), Args { warn: false }, &mut out).unwrap();
        assert_eq!(crate::testkit::text(out), "0 Surabaya/Semarang form(s) in 5 Javanese files checked\n");
        std::fs::remove_file(s.path().join(FILE)).unwrap();
        assert!(run(&s.root(), Args { warn: true }).is_err());
    }
}
