//! The Javanese speech-level table (§13, §17): `content/javanese/speech-levels.jsonl`,
//! one meaning per line with its ngoko, madya, krama, krama inggil and krama
//! andhap forms. Written by Claude Code following `prompts/speech-levels@v1.md`;
//! everything stays unverified until the learner or a native speaker approves it.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Subcommand;
use kabar_protocol::content::{SpeechLevel, SpeechLevelEntry};

use crate::bridge::read_jsonl;
use crate::gloss::check_tri;
use crate::paths::Root;

#[derive(Subcommand)]
pub enum Cmd {
    /// Validate the speech-level table.
    Validate,
    /// Counts per level.
    Stats,
}

pub fn path(root: &Root) -> PathBuf {
    root.path().join("content/javanese/speech-levels.jsonl")
}

pub fn load(root: &Root) -> Result<Vec<SpeechLevelEntry>> {
    read_jsonl(&path(root))
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    let rows = load(root)?;
    match cmd {
        Cmd::Validate => {
            let issues = validate(&rows);
            for i in &issues {
                println!("speech levels: {i}");
            }
            if !issues.is_empty() {
                bail!("{} speech-level issue(s)", issues.len());
            }
            println!("speech levels ok ({} rows)", rows.len());
            Ok(())
        }
        Cmd::Stats => {
            let count =
                |f: fn(&SpeechLevelEntry) -> &Vec<String>| rows.iter().filter(|r| !f(r).is_empty()).count();
            println!("{} rows", rows.len());
            println!("{:5}  with madya", count(|r| &r.madya));
            println!("{:5}  with krama", count(|r| &r.krama));
            println!("{:5}  with krama inggil", count(|r| &r.krama_inggil));
            println!("{:5}  with krama andhap", count(|r| &r.krama_andhap));
            Ok(())
        }
    }
}

/// Forms at one level of a row.
pub fn forms(row: &SpeechLevelEntry, level: SpeechLevel) -> &[String] {
    match level {
        SpeechLevel::Ngoko => &row.ngoko,
        SpeechLevel::Madya => &row.madya,
        SpeechLevel::Krama => &row.krama,
        SpeechLevel::KramaInggil => &row.krama_inggil,
        SpeechLevel::KramaAndhap => &row.krama_andhap,
    }
}

const LEVELS: [SpeechLevel; 5] = [
    SpeechLevel::Ngoko,
    SpeechLevel::Madya,
    SpeechLevel::Krama,
    SpeechLevel::KramaInggil,
    SpeechLevel::KramaAndhap,
];

/// Javanese words: lowercase Latin letters (plain e, no é/è: the learner's
/// chosen spelling), hyphens for reduplication and single spaces for
/// multi-word forms.
fn is_form(s: &str) -> bool {
    !s.is_empty()
        && s.split(' ').all(|w| {
            !w.is_empty() && w.split('-').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase()))
        })
}

pub fn validate(rows: &[SpeechLevelEntry]) -> Vec<String> {
    let mut issues = Vec::new();
    let mut ids = HashSet::new();
    for r in rows {
        let mut p = Vec::new();
        if !ids.insert(r.id.as_str()) {
            p.push("duplicate id".to_string());
        }
        if r.id.is_empty() || !r.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            p.push("id must be snake_case".into());
        }
        if r.ngoko.is_empty() {
            p.push("ngoko form is required".into());
        }
        if [&r.madya, &r.krama, &r.krama_inggil, &r.krama_andhap].iter().all(|l| l.is_empty()) {
            p.push("no form above ngoko: the row teaches nothing".into());
        }
        for level in LEVELS {
            for f in forms(r, level) {
                if !is_form(f) {
                    p.push(format!("{level:?} form {f:?} must be lowercase Latin"));
                }
            }
        }
        if r.indonesian.trim().is_empty() {
            p.push("empty indonesian".into());
        }
        check_tri(&r.meaning, "meaning", &mut p);
        if let Some(n) = &r.note {
            check_tri(n, "note", &mut p);
        }
        for (i, x) in r.examples.iter().enumerate() {
            check_tri(&x.translation, &format!("example {}", i + 1), &mut p);
            let text = x.text.to_lowercase();
            if !forms(r, x.level).iter().any(|f| text.contains(f.as_str())) {
                p.push(format!("example {} ({:?}) doesn't use a {:?} form", i + 1, x.level, x.level));
            }
        }
        if r.sources.is_empty() {
            p.push("no sources".into());
        }
        issues.extend(p.into_iter().map(|p| format!("{}: {p}", r.id)));
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_rows() {
        let row: SpeechLevelEntry = serde_json::from_str(
            r#"{"id":"eat","pos":"verb","meaning":{"arz":"ياكل","ar":"يأكل","en":"to eat"},"indonesian":"makan",
            "ngoko":["mangan"],"krama":["nedha"],"krama_inggil":["dhahar"],
            "examples":[{"level":"krama_inggil","text":"Panjenengan sampun dhahar?","translation":{"arz":"حضرتك أكلت؟","ar":"هل أكلتَ؟","en":"Have you eaten?"}}],
            "confidence":"high","sources":["claude-code"]}"#,
        )
        .unwrap();
        assert_eq!(validate(std::slice::from_ref(&row)), Vec::<String>::new());

        let mut bad = row.clone();
        bad.krama.clear();
        bad.krama_inggil = vec!["Dhahar".into()];
        bad.examples[0].level = SpeechLevel::Krama;
        let issues = validate(&[row, bad]);
        assert!(issues.iter().any(|i| i.contains("duplicate")), "{issues:?}");
        assert!(issues.iter().any(|i| i.contains("lowercase")), "{issues:?}");
        assert!(issues.iter().any(|i| i.contains("doesn't use")), "{issues:?}");
    }
}
