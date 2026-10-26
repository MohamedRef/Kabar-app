//! The Arabic Bridge dataset (§12): `content/bridge/id.jsonl` (one card per
//! line, sorted by word) and `content/bridge/sound-rules.json`.
//!
//! Cards are written by Claude Code following `prompts/bridge@v1.md` and
//! cross-checked against Wiktionary etymologies (`kabar-tools kaikki` writes
//! `content/bridge/wiktionary-id.jsonl`): `crosscheck` adds `wiktionary` to
//! a card's sources when Wiktionary agrees, flags disagreements, and lists
//! Wiktionary's Arabic loans that still need a card.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use kabar_protocol::content::{BridgeEntry, DictEntry, Lang, MeaningShift, SoundRule};

use crate::gloss::check_tri;
use crate::paths::Root;
use crate::text::{arabic_skeleton, has_arabic, is_arabic_letter, is_latin_word};

#[derive(Subcommand)]
pub enum Cmd {
    /// Validate the Bridge cards and sound rules.
    Validate,
    /// Compare the cards with Wiktionary etymologies (needs `kabar-tools kaikki` first).
    Crosscheck {
        /// Add `wiktionary` to the sources of cards Wiktionary confirms.
        #[arg(long)]
        write: bool,
    },
    /// Rewrite the cards file sorted by word, one card per line.
    Sort,
    /// Counts by meaning shift, confidence and sound rule.
    Stats,
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Validate => {
            let entries = load_entries(root)?;
            let rules = load_rules(root)?;
            let issues = validate(&entries, &rules);
            for i in &issues {
                println!("bridge: {i}");
            }
            if !issues.is_empty() {
                bail!("{} bridge issue(s)", issues.len());
            }
            println!("bridge ok ({} cards, {} sound rules)", entries.len(), rules.len());
            Ok(())
        }
        Cmd::Crosscheck { write } => crosscheck(root, write),
        Cmd::Sort => {
            let mut entries = load_entries(root)?;
            entries.sort_by(|a, b| a.word.cmp(&b.word));
            save_entries(root, &entries)
        }
        Cmd::Stats => {
            let entries = load_entries(root)?;
            let mut by: BTreeMap<String, usize> = BTreeMap::new();
            for e in &entries {
                *by.entry(format!("shift {:?}", e.shift)).or_default() += 1;
                *by.entry(format!("confidence {:?}", e.confidence)).or_default() += 1;
                for r in &e.sound_rules {
                    *by.entry(format!("rule {r}")).or_default() += 1;
                }
            }
            println!("{} cards", entries.len());
            for (k, n) in by {
                println!("{n:5}  {k}");
            }
            Ok(())
        }
    }
}

fn entries_path(root: &Root) -> PathBuf {
    root.bridge_dir().join("id.jsonl")
}

fn rules_path(root: &Root) -> PathBuf {
    root.bridge_dir().join("sound-rules.json")
}

pub fn wiktionary_path(root: &Root) -> PathBuf {
    root.bridge_dir().join("wiktionary-id.jsonl")
}

pub fn load_entries(root: &Root) -> Result<Vec<BridgeEntry>> {
    read_jsonl(&entries_path(root))
}

pub fn load_rules(root: &Root) -> Result<Vec<SoundRule>> {
    let path = rules_path(root);
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| path.display().to_string())
}

fn save_entries(root: &Root, entries: &[BridgeEntry]) -> Result<()> {
    let mut out = String::new();
    for e in entries {
        out.push_str(&serde_json::to_string(e)?);
        out.push('\n');
    }
    fs::write(entries_path(root), out)?;
    Ok(())
}

pub fn read_jsonl<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<Vec<T>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| serde_json::from_str(l).with_context(|| format!("{}:{}", path.display(), i + 1)))
        .collect()
}

pub fn validate(entries: &[BridgeEntry], rules: &[SoundRule]) -> Vec<String> {
    let mut issues = Vec::new();
    let rule_ids: HashSet<&str> = rules.iter().map(|r| r.id.as_str()).collect();
    let words: HashSet<&str> = entries.iter().map(|e| e.word.as_str()).collect();
    let mut seen = HashSet::new();
    let mut prev: Option<&str> = None;
    for e in entries {
        let w = &e.word;
        let mut p = Vec::new();
        if !seen.insert(w.as_str()) {
            p.push("duplicate".to_string());
        }
        if prev.is_some_and(|prev| prev > w.as_str()) {
            p.push("not sorted (run `kabar-tools bridge sort`)".into());
        }
        prev = Some(w);
        if e.lang != Lang::Id {
            p.push("only Indonesian cards live in id.jsonl".into());
        }
        if !is_latin_word(w) {
            p.push("word must be lowercase Latin".into());
        }
        if !has_arabic(&e.arabic) {
            p.push("arabic must be in Arabic script".into());
        }
        if let Some(root) = &e.root {
            let letters: Vec<&str> = root.split(' ').collect();
            let ok = (2..=5).contains(&letters.len())
                && letters.iter().all(|l| l.chars().count() == 1 && l.chars().all(is_arabic_letter));
            if !ok {
                p.push(format!("root {root:?} should be 2–5 Arabic letters separated by spaces"));
            }
        }
        check_tri(&e.arabic_meaning, "arabic_meaning", &mut p);
        check_tri(&e.meaning, "meaning", &mut p);
        match (&e.shift, &e.shift_note) {
            (MeaningShift::Same, _) => {}
            (_, Some(note)) => check_tri(note, "shift_note", &mut p),
            (_, None) => p.push("shift_note is required when the meaning moved".into()),
        }
        for r in &e.sound_rules {
            if !rule_ids.contains(r.as_str()) {
                p.push(format!("unknown sound rule {r:?}"));
            }
        }
        if e.sources.is_empty() {
            p.push("no sources".into());
        }
        issues.extend(p.into_iter().map(|p| format!("{w}: {p}")));
    }
    let mut seen_rules = HashSet::new();
    for r in rules {
        let mut p = Vec::new();
        if !seen_rules.insert(&r.id) {
            p.push("duplicate rule".to_string());
        }
        check_tri(&r.explanation, "explanation", &mut p);
        if r.examples.is_empty() {
            p.push("no examples".into());
        }
        for x in &r.examples {
            match entries.iter().find(|e| &e.word == x) {
                None if !words.is_empty() => p.push(format!("example {x:?} has no card")),
                Some(e) if !e.sound_rules.contains(&r.id) => {
                    p.push(format!("example {x:?} doesn't list this rule"))
                }
                _ => {}
            }
        }
        issues.extend(p.into_iter().map(|p| format!("rule {}: {p}", r.id)));
    }
    issues
}

fn crosscheck(root: &Root, write: bool) -> Result<()> {
    let mut entries = load_entries(root)?;
    let wikt: Vec<DictEntry> = read_jsonl(&wiktionary_path(root))
        .context("run `kabar-tools kaikki` on the Indonesian extract first")?;
    let mut by_word: BTreeMap<&str, Vec<&DictEntry>> = BTreeMap::new();
    for d in &wikt {
        by_word.entry(d.word.as_str()).or_default().push(d);
    }
    let (mut confirmed, mut differ, mut unknown) = (0, 0, 0);
    for e in &mut entries {
        let Some(ds) = by_word.get(e.word.as_str()) else {
            unknown += 1;
            continue;
        };
        let ours = arabic_skeleton(&e.arabic);
        let agrees = ds.iter().filter_map(|d| d.arabic.as_ref()).any(|a| {
            let theirs = arabic_skeleton(&a.arabic);
            theirs.is_empty() || theirs == ours || theirs.contains(&ours) || ours.contains(&theirs)
        });
        if agrees {
            confirmed += 1;
            if !e.sources.iter().any(|s| s == "wiktionary") {
                e.sources.push("wiktionary".into());
            }
        } else {
            differ += 1;
            let theirs: Vec<&str> =
                ds.iter().filter_map(|d| d.arabic.as_ref()).map(|a| a.arabic.as_str()).collect();
            println!("differs  {}: ours {} · wiktionary {:?}", e.word, e.arabic, theirs);
        }
    }
    let ours: HashSet<&str> = entries.iter().map(|e| e.word.as_str()).collect();
    let mut todo: Vec<&str> =
        by_word.keys().copied().filter(|w| !ours.contains(w) && is_latin_word(w)).collect();
    todo.dedup();
    let todo_path = root.build().join("bridge-todo.txt");
    fs::create_dir_all(todo_path.parent().unwrap())?;
    fs::write(&todo_path, todo.join("\n") + "\n")?;
    println!(
        "{confirmed} confirmed by Wiktionary, {differ} differ, {unknown} not marked Arabic in Wiktionary; \
         {} Wiktionary loans without a card (listed in {})",
        todo.len(),
        todo_path.display()
    );
    if write {
        save_entries(root, &entries)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_cards_and_rules() {
        let card: BridgeEntry = serde_json::from_str(
            r#"{"lang":"id","word":"kalimat","arabic":"كَلِمَة","root":"ك ل م",
            "arabic_meaning":{"arz":"كلمة","ar":"كلمة","en":"word"},
            "meaning":{"arz":"جملة","ar":"جملة","en":"sentence"},
            "shift":"false_friend","sound_rules":["ta_marbuta_at"],
            "confidence":"high","sources":["claude-code"]}"#,
        )
        .unwrap();
        let rule: SoundRule = serde_json::from_str(
            r#"{"id":"ta_marbuta_at","from":"ة","to":"-at",
            "explanation":{"arz":"التاء المربوطة بتبقى -at","ar":"تصير التاء المربوطة -at","en":"Final ta marbuta becomes -at"},
            "examples":["kalimat"]}"#,
        )
        .unwrap();
        let issues = validate(std::slice::from_ref(&card), std::slice::from_ref(&rule));
        assert_eq!(issues, ["kalimat: shift_note is required when the meaning moved"]);

        let mut bad_root = card.clone();
        bad_root.root = Some("كلم".into());
        bad_root.shift = MeaningShift::Same;
        bad_root.sound_rules = vec!["fa_p".into()];
        let issues = validate(&[bad_root], &[rule]);
        assert!(issues.iter().any(|i| i.contains("root")), "{issues:?}");
        assert!(issues.iter().any(|i| i.contains("unknown sound rule")), "{issues:?}");
        assert!(issues.iter().any(|i| i.contains("doesn't list this rule")), "{issues:?}");
    }
}
