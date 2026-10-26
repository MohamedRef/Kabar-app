//! Scores + answers → the bake-off report and a proposed route table.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use kabar_core::ai::routes::RouteTable;
use kabar_protocol::ai::{EngineId, Task};
use serde::Deserialize;

use super::{KeyEntry, Record, load_run};
use crate::golden;
use crate::paths::Root;

/// Pass bar (docs/BAKEOFF.md).
pub const MIN_MEAN: f64 = 2.5;
pub const MIN_VALID: f64 = 0.98;

/// p50 budget per task (docs/BAKEOFF.md): popover enrichment 2 s, grading
/// 3 s, roleplay 4 s, batch work 30 s.
pub fn latency_budget_ms(task: Task) -> u64 {
    match task {
        Task::AnalyzeText | Task::ExplainSentence => 2_000,
        Task::GradeAnswer | Task::ReadHandwriting | Task::GuessSketch | Task::CheckStrokes => 3_000,
        Task::RoleplayTurn => 4_000,
        _ => 30_000,
    }
}

#[derive(Debug, Deserialize)]
struct Score {
    item: String,
    letter: String,
    score: u8,
}

/// One task × engine line of the report.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub engine: String,
    pub runs: usize,
    pub valid: f64,
    pub first_try: f64,
    pub p50_ms: u64,
    pub judged: usize,
    pub mean: Option<f64>,
    pub zeros: usize,
    /// Why it doesn't pass; empty when it does.
    pub misses: Vec<String>,
}

impl Row {
    pub fn passes(&self) -> bool {
        self.misses.is_empty()
    }
}

fn p50(mut v: Vec<u64>) -> u64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[(v.len() - 1) / 2]
}

/// `items`: how many of the set's items an engine has to answer (those
/// waiting for a device capture don't count), so a `--limit` smoke run
/// never vets a route.
pub fn rows(
    task: Task,
    items: usize,
    records: &[Record],
    scores: &BTreeMap<(String, String), u8>,
) -> Vec<Row> {
    let mut by_engine: BTreeMap<&str, Vec<&Record>> = BTreeMap::new();
    for r in records {
        by_engine.entry(r.engine.as_str()).or_default().push(r);
    }
    by_engine
        .into_iter()
        .map(|(engine, rs)| {
            let n = rs.len();
            let ok: Vec<&&Record> = rs.iter().filter(|r| r.ok).collect();
            let valid = ok.len() as f64 / n as f64;
            let first_try = rs.iter().filter(|r| r.first_ok).count() as f64 / n as f64;
            let p50_ms = p50(ok.iter().map(|r| r.total_ms).collect());
            let judged: Vec<u8> = ok
                .iter()
                .filter_map(|r| scores.get(&(r.item.clone(), engine.to_string())).copied())
                .collect();
            let mean = (!judged.is_empty())
                .then(|| judged.iter().map(|&s| f64::from(s)).sum::<f64>() / judged.len() as f64);
            let zeros = judged.iter().filter(|&&s| s == 0).count();
            let mut misses = Vec::new();
            if n < items {
                misses.push(format!("only {n} of {items} items run"));
            }
            if valid < MIN_VALID {
                misses.push(format!("valid {:.0}% < {:.0}%", valid * 100.0, MIN_VALID * 100.0));
            }
            if judged.len() < ok.len() {
                misses.push(format!("{} of {} answers not judged yet", ok.len() - judged.len(), ok.len()));
            }
            match mean {
                Some(m) if m < MIN_MEAN => misses.push(format!("mean {m:.2} < {MIN_MEAN}")),
                None => misses.push("no scores".into()),
                _ => {}
            }
            if zeros > 0 {
                misses.push(format!("{zeros} answer(s) scored 0"));
            }
            let budget = latency_budget_ms(task);
            if p50_ms > budget {
                misses.push(format!("p50 {:.1} s > {:.0} s", p50_ms as f64 / 1000.0, budget as f64 / 1000.0));
            }
            Row {
                engine: engine.to_string(),
                runs: n,
                valid,
                first_try,
                p50_ms,
                judged: judged.len(),
                mean,
                zeros,
                misses,
            }
        })
        .collect()
}

fn engine_id(label: &str) -> Option<EngineId> {
    if label.starts_with("local-") {
        return Some(EngineId::Local);
    }
    EngineId::ALL.iter().copied().find(|e| e.as_str() == label)
}

/// The route for one task: passing engines, best first (mean, then speed),
/// one local model at most; Claude last, since it is off by default. With
/// no passing engine the current route stays, provisional, and the report
/// says so.
/// Also returns the chosen rows' labels, so the report names the local model.
pub fn propose(task: Task, rows: &[Row], current: &RouteTable) -> (Vec<EngineId>, Vec<String>, bool) {
    let mut passing: Vec<&Row> = rows.iter().filter(|r| r.passes()).collect();
    passing.sort_by(|a, b| {
        let claude = |r: &Row| engine_id(&r.engine) == Some(EngineId::Claude);
        claude(a)
            .cmp(&claude(b))
            .then(b.mean.partial_cmp(&a.mean).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.p50_ms.cmp(&b.p50_ms))
    });
    let mut engines = Vec::new();
    let mut labels = Vec::new();
    for r in passing {
        if let Some(e) = engine_id(&r.engine).filter(|e| !engines.contains(e)) {
            if task.is_personal() && e.trains_on_inputs() {
                continue;
            }
            engines.push(e);
            labels.push(r.engine.clone());
        }
    }
    if engines.is_empty() {
        let current = current.get(task).engines.clone();
        let labels = current.iter().map(|e| e.as_str().to_string()).collect();
        (current, labels, false)
    } else {
        (engines, labels, true)
    }
}

pub fn routes_toml(table: &[(Task, Vec<EngineId>, bool)], current: &RouteTable) -> String {
    let mut s = String::from(
        "# Route table for the AI router (spec §19, PLAN §9), written by\n\
         # `kabar-tools bakeoff report --write` from the bake-off (docs/bakeoff-report.md).\n\
         #\n\
         # Per task: engines that passed the task's golden set, best first. The router\n\
         # also drops engines that are switched off, paid engines while the $0 hard\n\
         # stop is on, and any engine that trains on inputs (gemini_free) for personal\n\
         # tasks. A parse check rejects a personal task listed with gemini_free.\n\
         # status = \"provisional\" where no engine passed yet: answers carry a badge.\n",
    );
    for (task, engines, vetted) in table {
        let names: Vec<String> = engines.iter().map(|e| format!("\"{}\"", e.as_str())).collect();
        let _ = write!(
            s,
            "\n[{}]\nengines = [{}]\nstatus = \"{}\"\n",
            task.as_str(),
            names.join(", "),
            if *vetted { "vetted" } else { "provisional" }
        );
        let route = current.get(*task);
        if let Some(v) = route.prompt_version {
            let _ = writeln!(s, "prompt_version = {v}");
        }
        if let Some(m) = route.max_tokens {
            let _ = writeln!(s, "max_tokens = {m}");
        }
    }
    s
}

fn read_scores(
    run: &Path,
    task: Task,
    key: &BTreeMap<(String, String, String), String>,
) -> Result<BTreeMap<(String, String), u8>> {
    let path = run.join("scores").join(format!("{}.jsonl", task.as_str()));
    let mut out = BTreeMap::new();
    if !path.exists() {
        return Ok(out);
    }
    for (i, line) in fs::read_to_string(&path)?.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let s: Score = serde_json::from_str(line).with_context(|| format!("{}:{}", path.display(), i + 1))?;
        anyhow::ensure!(s.score <= 3, "{}:{}: score must be 0-3", path.display(), i + 1);
        let engine =
            key.get(&(task.as_str().to_string(), s.item.clone(), s.letter.clone())).with_context(|| {
                format!("{}:{}: no answer {}/{} in key.jsonl", path.display(), i + 1, s.item, s.letter)
            })?;
        out.insert((s.item, engine.clone()), s.score);
    }
    Ok(out)
}

pub fn report(root: &Root, run: &Path, write: bool) -> Result<()> {
    let records = load_run(run)?;
    let mut key = BTreeMap::new();
    let key_path = run.join("key.jsonl");
    if key_path.exists() {
        for line in fs::read_to_string(&key_path)?.lines().filter(|l| !l.trim().is_empty()) {
            let k: KeyEntry = serde_json::from_str(line)?;
            key.insert((k.task, k.item, k.letter), k.engine);
        }
    }
    let current = RouteTable::builtin();
    let mut md = format!(
        "# Bake-off report\n\nRun: `{}`. Pass bar: mean ≥ {MIN_MEAN}, no 0, valid ≥ {:.0}%, p50 within the task's \
         budget (docs/BAKEOFF.md). *Valid* counts the router's one repair retry; *first try* doesn't.\n",
        run.strip_prefix(root.path()).unwrap_or(run).display(),
        MIN_VALID * 100.0
    );
    let mut table = Vec::new();
    let mut gaps = Vec::new();
    for task in Task::ALL {
        let rs = records.get(&task).map(Vec::as_slice).unwrap_or_default();
        let scores = read_scores(run, task, &key)?;
        let set = golden::load(&golden::dir(root).join(format!("{}.yaml", task.as_str())))?;
        let items = set
            .items
            .iter()
            .filter(|i| !(i.image.is_none() && i.tags.iter().any(|t| t == golden::NEEDS_CAPTURE)))
            .count();
        let rows = rows(task, items, rs, &scores);
        let (engines, labels, vetted) = propose(task, &rows, &current);
        let _ = write!(
            md,
            "\n## {}\n\nBudget p50 {:.0} s. Route: {} ({}).\n\n",
            task.as_str(),
            latency_budget_ms(task) as f64 / 1000.0,
            labels.iter().map(|l| format!("`{l}`")).collect::<Vec<_>>().join(", "),
            if vetted { "vetted" } else { "provisional: no engine passed" }
        );
        if rows.is_empty() {
            md.push_str("Not run.\n");
        } else {
            md.push_str("| Engine | Runs | Valid | First try | p50 | Judged | Mean | Zeros | Result |\n|---|---|---|---|---|---|---|---|---|\n");
            for r in &rows {
                let _ = writeln!(
                    md,
                    "| {} | {} | {:.0}% | {:.0}% | {:.1} s | {} | {} | {} | {} |",
                    r.engine,
                    r.runs,
                    r.valid * 100.0,
                    r.first_try * 100.0,
                    r.p50_ms as f64 / 1000.0,
                    r.judged,
                    r.mean.map_or("–".into(), |m| format!("{m:.2}")),
                    r.zeros,
                    if r.passes() { "pass".into() } else { r.misses.join("; ") }
                );
            }
        }
        if !vetted {
            gaps.push(task.as_str());
        }
        table.push((task, engines, vetted));
    }
    if !gaps.is_empty() {
        let _ = write!(
            md,
            "\n## Gaps\n\nNo engine passed yet for: {}. Their routes stay provisional.\n",
            gaps.join(", ")
        );
    }
    let toml = routes_toml(&table, &current);
    RouteTable::parse(&toml).map_err(anyhow::Error::msg).context("the proposed route table doesn't parse")?;
    fs::write(run.join("report.md"), &md)?;
    fs::write(run.join("routes.toml"), &toml)?;
    println!("{}", run.join("report.md").display());
    if write {
        fs::write(root.path().join("docs/bakeoff-report.md"), &md)?;
        fs::write(root.path().join("prompts/routes.toml"), &toml)?;
        println!("wrote docs/bakeoff-report.md and prompts/routes.toml");
    }
    println!("{} of {} tasks vetted", table.iter().filter(|t| t.2).count(), table.len());
    Ok(())
}
