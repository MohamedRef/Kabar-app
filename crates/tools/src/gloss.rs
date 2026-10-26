//! Prebuilt glosses for the most frequent words (§18).
//!
//! Work is planned once into `content/glosses/<lang>/plan.tsv` (rank, word,
//! batch) so batches stay stable even if the frequency list is rebuilt
//! later. Each batch of 100 words is one file, `batch-NNN.jsonl`, written by
//! a Claude Code session following `prompts/gloss@v1.md`. A batch is done
//! when its file validates; `next` hands out the first batch that isn't, so
//! the work can stop and resume across sessions in any order.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use kabar_protocol::content::{GlossEntry, Lang, MeaningShift, Pos, Trilingual};

use crate::bridge;
use crate::freq::{parse_lang, read_list};
use crate::paths::Root;
use crate::text::{has_arabic, is_latin_word};

pub const BATCH_SIZE: usize = 100;

#[derive(Subcommand)]
pub enum Cmd {
    /// Plan (or extend the plan to) the top `count` words of the frequency list.
    Plan {
        #[arg(long, default_value = "id")]
        lang: String,
        #[arg(long, default_value_t = 2000)]
        count: usize,
    },
    /// Show which batches are done, invalid or missing.
    Status {
        #[arg(long, default_value = "id")]
        lang: String,
    },
    /// Print the next batch to generate (number + its words), or nothing when all are done.
    Next {
        #[arg(long, default_value = "id")]
        lang: String,
    },
    /// Print the words of one batch.
    Words {
        #[arg(long, default_value = "id")]
        lang: String,
        batch: u32,
    },
    /// Validate one batch, or every batch file present.
    Validate {
        #[arg(long)]
        batch: Option<u32>,
        /// Only this language (`id` or `jv`); both by default.
        #[arg(long)]
        lang: Option<String>,
    },
    /// Write a Markdown review sheet for one batch to build/review/.
    Review {
        #[arg(long, default_value = "id")]
        lang: String,
        batch: u32,
    },
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Plan { lang, count } => plan(root, parse_lang(&lang)?, count),
        Cmd::Status { lang } => {
            let lang = parse_lang(&lang)?;
            let plan = Plan::load(root, lang)?;
            let (mut done, mut bad, mut missing) = (0, 0, 0);
            for batch in plan.batches() {
                let path = batch_path(root, lang, batch);
                let state = if !path.exists() {
                    missing += 1;
                    "missing".to_string()
                } else {
                    match validate_batch(root, &plan, batch) {
                        Ok(issues) if issues.is_empty() => {
                            done += 1;
                            "done".to_string()
                        }
                        Ok(issues) => {
                            bad += 1;
                            format!("{} issue(s)", issues.len())
                        }
                        Err(e) => {
                            bad += 1;
                            format!("unreadable: {e:#}")
                        }
                    }
                };
                println!("batch {batch:03}: {state}");
            }
            println!("{done} done, {bad} need fixes, {missing} missing ({} words planned)", plan.rows.len());
            Ok(())
        }
        Cmd::Next { lang } => {
            let lang = parse_lang(&lang)?;
            let plan = Plan::load(root, lang)?;
            for batch in plan.batches() {
                let ok = batch_path(root, lang, batch).exists()
                    && validate_batch(root, &plan, batch).is_ok_and(|i| i.is_empty());
                if !ok {
                    print_words(&plan, batch);
                    return Ok(());
                }
            }
            Ok(())
        }
        Cmd::Words { lang, batch } => {
            print_words(&Plan::load(root, parse_lang(&lang)?)?, batch);
            Ok(())
        }
        Cmd::Validate { batch, lang } => {
            let langs = match lang {
                Some(l) => vec![parse_lang(&l)?],
                None => vec![Lang::Id, Lang::Jv],
            };
            let mut failures = 0;
            for &lang in &langs {
                let Ok(plan) = Plan::load(root, lang) else { continue };
                let batches: Vec<u32> = match batch {
                    // Without --lang, a batch number only checks the languages that have that file.
                    Some(b) if langs.len() > 1 && !batch_path(root, lang, b).exists() => vec![],
                    Some(b) => vec![b],
                    None => plan.batches().filter(|b| batch_path(root, lang, *b).exists()).collect(),
                };
                for b in batches {
                    let issues = validate_batch(root, &plan, b)?;
                    for issue in &issues {
                        println!("{}/batch-{b:03}: {issue}", lang.code());
                    }
                    failures += issues.len();
                }
            }
            if failures > 0 {
                bail!("{failures} gloss issue(s)");
            }
            println!("glosses ok");
            Ok(())
        }
        Cmd::Review { lang, batch } => {
            let lang = parse_lang(&lang)?;
            let entries = read_batch(&batch_path(root, lang, batch))?;
            let out = root.build().join(format!("review/glosses-{}-{batch:03}.md", lang.code()));
            fs::create_dir_all(out.parent().unwrap())?;
            fs::write(&out, review_sheet(lang, batch, &entries))?;
            println!("wrote {}", out.display());
            Ok(())
        }
    }
}

pub struct PlanRow {
    pub rank: u32,
    pub word: String,
    pub batch: u32,
}

pub struct Plan {
    pub lang: Lang,
    pub rows: Vec<PlanRow>,
}

impl Plan {
    pub fn load(root: &Root, lang: Lang) -> Result<Plan> {
        let path = plan_path(root, lang);
        let text = fs::read_to_string(&path).with_context(|| format!("no plan at {}", path.display()))?;
        let rows = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() != 3 {
                    bail!("bad plan row {l:?}");
                }
                Ok(PlanRow { rank: c[0].parse()?, word: c[1].to_string(), batch: c[2].parse()? })
            })
            .collect::<Result<_>>()?;
        Ok(Plan { lang, rows })
    }

    pub fn batches(&self) -> impl Iterator<Item = u32> + '_ {
        let mut seen: Vec<u32> = self.rows.iter().map(|r| r.batch).collect();
        seen.dedup();
        seen.into_iter()
    }

    pub fn batch(&self, batch: u32) -> impl Iterator<Item = &PlanRow> {
        self.rows.iter().filter(move |r| r.batch == batch)
    }
}

fn plan_path(root: &Root, lang: Lang) -> PathBuf {
    root.gloss_dir(lang).join("plan.tsv")
}

pub fn batch_path(root: &Root, lang: Lang, batch: u32) -> PathBuf {
    root.gloss_dir(lang).join(format!("batch-{batch:03}.jsonl"))
}

/// Append the next most frequent unplanned words until `count` are planned.
/// Existing rows never move, so finished batches stay valid.
fn plan(root: &Root, lang: Lang, count: usize) -> Result<()> {
    let freq = read_list(&root.freq_list(lang))?;
    let mut rows = Plan::load(root, lang).map(|p| p.rows).unwrap_or_default();
    let planned: HashSet<String> = rows.iter().map(|r| r.word.clone()).collect();
    let before = rows.len();
    for row in freq.into_iter().filter(|r| !planned.contains(&r.word)) {
        if rows.len() >= count {
            break;
        }
        let batch = (rows.len() / BATCH_SIZE) as u32 + 1;
        rows.push(PlanRow { rank: row.rank, word: row.word, batch });
    }
    let mut out = String::from(
        "# Gloss work plan: rank in the frequency list, word, batch number.\n\
         # Generated by `kabar-tools gloss plan`; rows are only ever appended.\n",
    );
    for r in &rows {
        writeln!(out, "{}\t{}\t{}", r.rank, r.word, r.batch)?;
    }
    let path = plan_path(root, lang);
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(&path, out)?;
    println!(
        "planned {} words ({} new) in {} batches",
        rows.len(),
        rows.len() - before,
        rows.len().div_ceil(BATCH_SIZE)
    );
    Ok(())
}

fn print_words(plan: &Plan, batch: u32) {
    println!("# batch {batch:03} ({})", plan.lang.code());
    for r in plan.batch(batch) {
        println!("{}\t{}", r.rank, r.word);
    }
}

pub fn read_batch(path: &Path) -> Result<Vec<GlossEntry>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| serde_json::from_str(l).with_context(|| format!("{}:{}", path.display(), i + 1)))
        .collect()
}

/// Every problem in one batch file; empty means the batch is done.
pub fn validate_batch(root: &Root, plan: &Plan, batch: u32) -> Result<Vec<String>> {
    let entries = match read_batch(&batch_path(root, plan.lang, batch)) {
        Ok(e) => e,
        Err(e) => return Ok(vec![format!("{e:#}")]),
    };
    let mut issues = Vec::new();
    let expected: Vec<&PlanRow> = plan.batch(batch).collect();
    if expected.is_empty() {
        issues.push("batch is not in the plan".to_string());
    }
    let mut seen = HashSet::new();
    for (i, row) in expected.iter().enumerate() {
        match entries.get(i) {
            Some(e) if e.word == row.word && e.rank == row.rank => {}
            Some(e) => issues.push(format!(
                "line {}: expected {} (rank {}), found {} (rank {})",
                i + 1,
                row.word,
                row.rank,
                e.word,
                e.rank
            )),
            None => issues.push(format!("missing {} (rank {})", row.word, row.rank)),
        }
    }
    if entries.len() > expected.len() {
        issues.push(format!("{} extra line(s)", entries.len() - expected.len()));
    }
    for e in &entries {
        if !seen.insert(e.word.clone()) {
            issues.push(format!("{}: duplicate", e.word));
        }
        for problem in check_entry(e, plan.lang) {
            issues.push(format!("{}: {problem}", e.word));
        }
    }
    Ok(issues)
}

/// Glosses and Bridge cards must agree: every Arabic-origin gloss has a card
/// (its own, or its lemma's or root's), every gloss of a carded word says
/// so, and false friends warn.
/// Kept out of `validate_batch` so a batch can be written before its cards.
pub fn crosscheck_bridge(root: &Root, lang: Lang) -> Result<Vec<String>> {
    let cards = bridge::load_entries(root)?;
    let cards: BTreeMap<&str, MeaningShift> = cards.iter().map(|b| (b.word.as_str(), b.shift)).collect();
    let plan = Plan::load(root, lang)?;
    // Javanese glosses name an Indonesian equivalent; its own lemma and root
    // (menghormati → hormat) lead to the card too. When that equivalent is not
    // Arabic (asmanipun → namanya), the loan is Javanese-only and the
    // Indonesian Bridge has no place for it.
    let mut id_stems: BTreeMap<String, (Vec<String>, bool)> = BTreeMap::new();
    if lang == Lang::Jv {
        let id_plan = Plan::load(root, Lang::Id)?;
        for batch in id_plan.batches() {
            let path = batch_path(root, Lang::Id, batch);
            if path.exists() {
                for g in read_batch(&path)? {
                    let arabic = g.arabic_origin.is_some();
                    id_stems.insert(
                        g.word.clone(),
                        ([Some(g.lemma), g.root].into_iter().flatten().collect(), arabic),
                    );
                }
            }
        }
    }
    let mut issues = Vec::new();
    for batch in plan.batches() {
        let path = batch_path(root, lang, batch);
        if !path.exists() {
            continue;
        }
        for e in read_batch(&path)? {
            // Derived words (memikirkan) share the card of their lemma or root (pikir).
            let card = [Some(&e.word), Some(&e.lemma), e.root.as_ref()]
                .into_iter()
                .flatten()
                .find_map(|w| cards.get(w.as_str()));
            // Javanese words borrow through Indonesian (kewan → hewan): the card of
            // the first Indonesian equivalent counts for an Arabic-origin gloss,
            // but only the word's own card demands one (pas ≠ waktu).
            let first =
                e.indonesian.as_deref().and_then(|i| i.split(',').next()).map(|f| f.trim().to_lowercase());
            let id_entry = first.as_ref().and_then(|f| id_stems.get(f));
            let via_indonesian = || {
                let stems = id_entry.into_iter().flat_map(|(s, _)| s).map(String::as_str);
                first.as_deref().into_iter().chain(stems).find_map(|w| cards.get(w))
            };
            let javanese_only = id_entry.is_some_and(|(_, arabic)| !arabic);
            // Names (Musa, Mesir) and set phrases (swt) carry an Arabic origin
            // without being Bridge material.
            let name_or_phrase = e.senses.iter().all(|s| matches!(s.pos, Pos::ProperNoun | Pos::Phrase));
            match (card, &e.arabic_origin) {
                (None, Some(_)) if name_or_phrase || via_indonesian().is_some() || javanese_only => {}
                (None, Some(a)) => issues.push(format!("{}: arabic_origin {a} but no Bridge card", e.word)),
                (Some(_), None) => issues.push(format!("{}: has a Bridge card but no arabic_origin", e.word)),
                (Some(MeaningShift::FalseFriend), Some(_)) if e.false_friend.is_none() => {
                    issues.push(format!("{}: false friend in the Bridge but no false_friend warning", e.word))
                }
                _ => {}
            }
        }
    }
    Ok(issues)
}

/// Content checks a JSON Schema can't express.
pub fn check_entry(e: &GlossEntry, lang: Lang) -> Vec<String> {
    let mut p = Vec::new();
    if e.lang != lang {
        p.push(format!("lang is {:?}", e.lang));
    }
    if !is_latin_word(&e.word) {
        p.push("word must be lowercase Latin".into());
    }
    if e.lemma.trim().is_empty() {
        p.push("empty lemma".into());
    }
    if !(1..=4).contains(&e.senses.len()) {
        p.push(format!("{} senses (1–4 expected)", e.senses.len()));
    }
    if !(2..=3).contains(&e.examples.len()) {
        p.push(format!("{} examples (2–3 expected)", e.examples.len()));
    }
    if !e.affixes.is_empty() {
        match &e.root {
            Some(root) if e.affixes.iter().any(|a| a == root) => {}
            Some(root) => p.push(format!("affixes {:?} don't include the root {root:?}", e.affixes)),
            None => p.push("affixes without a root".into()),
        }
    }
    for (i, s) in e.senses.iter().enumerate() {
        check_tri(&s.gloss, &format!("sense {}", i + 1), &mut p);
    }
    for (i, x) in e.examples.iter().enumerate() {
        let what = format!("example {}", i + 1);
        check_tri(&x.translation, &what, &mut p);
        if usize::from(x.sense) >= e.senses.len() {
            p.push(format!("{what}: sense {} doesn't exist", x.sense));
        }
        let text = x.text.to_lowercase();
        let forms = [Some(&e.word), Some(&e.lemma), e.root.as_ref()];
        if !forms.iter().flatten().any(|f| text.contains(f.as_str())) {
            p.push(format!("{what} doesn't contain {:?}", e.word));
        }
        if !x.text.trim_end().ends_with(['.', '?', '!', '"', '…']) {
            p.push(format!("{what} should end with punctuation"));
        }
        if has_arabic(&x.text) {
            p.push(format!("{what}: Arabic script in the sentence"));
        }
    }
    if let Some(n) = &e.note {
        check_tri(n, "note", &mut p);
    }
    if let Some(f) = &e.false_friend {
        check_tri(f, "false_friend", &mut p);
        if e.arabic_origin.is_none() {
            p.push("false_friend needs arabic_origin".into());
        }
    }
    if let Some(a) = &e.arabic_origin
        && !has_arabic(a)
    {
        p.push("arabic_origin must be in Arabic script".into());
    }
    match lang {
        Lang::Jv if e.indonesian.as_deref().is_none_or(|i| i.trim().is_empty()) => {
            p.push("Javanese glosses need the Indonesian equivalent".into())
        }
        Lang::Id if e.indonesian.is_some() || e.speech_level.is_some() => {
            p.push("indonesian/speech_level are for Javanese glosses only".into())
        }
        _ => {}
    }
    if e.provenance.generator.trim().is_empty() {
        p.push("empty provenance.generator".into());
    }
    p
}

pub fn check_tri(t: &Trilingual, what: &str, p: &mut Vec<String>) {
    for (field, text) in [("arz", &t.arz), ("ar", &t.ar)] {
        if !has_arabic(text) {
            p.push(format!("{what}: {field} must be Arabic"));
        }
    }
    // English may quote an Arabic word («كلمة»), but must be English text.
    if !t.en.chars().any(|c| c.is_ascii_alphabetic()) {
        p.push(format!("{what}: en must be English"));
    }
    for text in [&t.arz, &t.ar, &t.en] {
        if text.trim() != text || text.contains("TODO") {
            p.push(format!("{what}: untidy text {text:?}"));
        }
    }
}

fn review_sheet(lang: Lang, batch: u32, entries: &[GlossEntry]) -> String {
    let mut md = format!(
        "# Glosses {} · batch {batch:03}\n\nTick ✅ or write a fix next to each word.\n\n",
        lang.code()
    );
    for e in entries {
        let breakdown =
            if e.affixes.is_empty() { String::new() } else { format!(" ({})", e.affixes.join(" + ")) };
        let _ = writeln!(md, "## {}. {}{breakdown}\n", e.rank, e.word);
        for s in &e.senses {
            let _ = writeln!(md, "- *{:?}* · {} · {} · {}", s.pos, s.gloss.arz, s.gloss.ar, s.gloss.en);
        }
        for x in &e.examples {
            let _ = writeln!(md, "- > {}  \n  {} · {}", x.text, x.translation.arz, x.translation.en);
        }
        if let Some(a) = &e.arabic_origin {
            let _ = writeln!(md, "- جسر: {a}");
        }
        if let Some(f) = &e.false_friend {
            let _ = writeln!(md, "- ⚠️ {}", f.arz);
        }
        if let Some(n) = &e.note {
            let _ = writeln!(md, "- ℹ️ {}", n.arz);
        }
        md.push('\n');
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(json: &str) -> GlossEntry {
        serde_json::from_str(json).unwrap()
    }

    const GOOD: &str = r#"{"lang":"id","word":"membaca","rank":900,"lemma":"membaca","root":"baca","affixes":["meN-","baca"],
        "senses":[{"pos":"verb","gloss":{"arz":"يقرا","ar":"يقرأ","en":"to read"}}],
        "examples":[
          {"text":"Saya sedang membaca buku.","translation":{"arz":"أنا بقرا كتاب.","ar":"أنا أقرأ كتابًا.","en":"I am reading a book."}},
          {"text":"Dia suka membaca koran.","translation":{"arz":"هو بيحب يقرا الجرنال.","ar":"يحب قراءة الصحف.","en":"He likes reading the newspaper."}}],
        "provenance":{"generator":"claude-code","prompt":"gloss@v1"}}"#;

    #[test]
    fn good_entry_passes() {
        assert_eq!(check_entry(&entry(GOOD), Lang::Id), Vec::<String>::new());
    }

    #[test]
    fn catches_content_mistakes() {
        let bad = GOOD
            .replace("\"ar\":\"يقرأ\"", "\"ar\":\"to read\"")
            .replace("Dia suka membaca koran.", "Dia suka koran")
            .replace("[\"meN-\",\"baca\"]", "[\"meN-\",\"bac\"]");
        let problems = check_entry(&entry(&bad), Lang::Id);
        assert!(problems.iter().any(|p| p.contains("ar must be Arabic")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("doesn't contain")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("punctuation")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("root")), "{problems:?}");
    }
}
