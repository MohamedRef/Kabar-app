//! Golden sets for the runtime AI tasks (§19): `content/golden/<task>.yaml`,
//! one [`GoldenSet`] per task, read by the bake-off runner. `input` is exactly
//! what the app sends (the router hashes it for the cache and renders it into
//! the prompt), and for typed tasks `expect` deserializes into the task's
//! output struct in `kabar_core::ai::tasks`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use kabar_core::ai::tasks::{AiOutput, AnalyzeText, GradeAnswer};
use kabar_protocol::ai::Task;
use kabar_protocol::content::GoldenSet;
use serde_json::Value;

use crate::paths::Root;

/// Items each set needs before the bake-off can judge an engine (docs/BAKEOFF.md).
pub const MIN_ITEMS: usize = 30;

/// Tag for vision items whose image must be captured on a device first.
pub const NEEDS_CAPTURE: &str = "needs-capture";

#[derive(Subcommand)]
pub enum Cmd {
    /// Validate every golden set.
    Validate,
    /// Items per task, verified and pending capture.
    Stats,
}

pub fn dir(root: &Root) -> PathBuf {
    root.path().join("content/golden")
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Validate => {
            let mut issues = Vec::new();
            for task in Task::ALL {
                let path = dir(root).join(format!("{}.yaml", task.as_str()));
                if !path.exists() {
                    issues.push(format!("{}: missing", task.as_str()));
                    continue;
                }
                match load(&path) {
                    Ok(set) => issues.extend(
                        validate(root, task, &set).into_iter().map(|i| format!("{}: {i}", task.as_str())),
                    ),
                    Err(e) => issues.push(format!("{}: {e:#}", task.as_str())),
                }
            }
            for i in &issues {
                println!("golden {i}");
            }
            if !issues.is_empty() {
                bail!("{} golden-set issue(s)", issues.len());
            }
            println!("golden sets ok ({} tasks)", Task::ALL.len());
            Ok(())
        }
        Cmd::Stats => {
            for task in Task::ALL {
                let path = dir(root).join(format!("{}.yaml", task.as_str()));
                let Ok(set) = load(&path) else {
                    println!("{:22} missing", task.as_str());
                    continue;
                };
                let verified = set.items.iter().filter(|i| i.verified).count();
                let capture = set.items.iter().filter(|i| i.tags.iter().any(|t| t == NEEDS_CAPTURE)).count();
                println!(
                    "{:22} {:3} items, {verified} verified, {capture} need capture",
                    task.as_str(),
                    set.items.len()
                );
            }
            Ok(())
        }
    }
}

pub fn load(path: &Path) -> Result<GoldenSet> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml_ng::from_str(&text).with_context(|| path.display().to_string())
}

pub fn validate(root: &Root, task: Task, set: &GoldenSet) -> Vec<String> {
    let mut issues = Vec::new();
    if set.task != task.as_str() {
        issues.push(format!("task field is {:?}", set.task));
    }
    if set.rubric.trim().is_empty() {
        issues.push("empty rubric".into());
    }
    if set.items.len() < MIN_ITEMS {
        issues.push(format!("{} items ({MIN_ITEMS} needed)", set.items.len()));
    }
    let mut ids = HashSet::new();
    for item in &set.items {
        let mut p = Vec::new();
        if !ids.insert(item.id.as_str()) {
            p.push("duplicate id".to_string());
        }
        if !item.input.is_object() {
            p.push("input must be an object".into());
        }
        if item.expect.is_null() && item.criteria.is_empty() {
            p.push("needs an expected answer or criteria".into());
        }
        match &item.image {
            Some(img) if !root.path().join(img).exists() => p.push(format!("image {img} not found")),
            None if task.needs_vision() && !item.tags.iter().any(|t| t == NEEDS_CAPTURE) => p.push(format!(
                "vision item without an image (tag it {NEEDS_CAPTURE} if it must be captured)"
            )),
            _ => {}
        }
        p.extend(check_typed(task, &item.input, &item.expect));
        issues.extend(p.into_iter().map(|p| format!("{}: {p}", item.id)));
    }
    issues
}

/// Inputs and expected outputs of the tasks the router already types.
fn check_typed(task: Task, input: &Value, expect: &Value) -> Vec<String> {
    let mut p = Vec::new();
    let str_field = |k: &str| input.get(k).and_then(Value::as_str).is_some_and(|s| !s.trim().is_empty());
    let str_list = |k: &str| {
        input
            .get(k)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty() && a.iter().all(Value::is_string))
    };
    match task {
        Task::AnalyzeText => {
            if !str_field("text") || !str_list("words") {
                p.push("input needs text and words[]".into());
            }
            if !expect.is_null() {
                match serde_json::from_value::<AnalyzeText>(expect.clone()) {
                    Ok(out) => {
                        if let Err(e) = out.validate() {
                            p.push(format!("expect: {e}"));
                        }
                        let words = input.get("words").and_then(Value::as_array).map_or(0, Vec::len);
                        if out.tokens.len() != words {
                            p.push(format!("expect has {} tokens for {words} words", out.tokens.len()));
                        }
                    }
                    Err(e) => p.push(format!("expect is not an AnalyzeText: {e}")),
                }
            }
        }
        Task::GradeAnswer => {
            if !str_field("exercise") || !str_list("references") || !str_field("answer") {
                p.push("input needs exercise, references[] and answer".into());
            }
            match serde_json::from_value::<GradeAnswer>(expect.clone()) {
                Ok(out) => {
                    if let Err(e) = out.validate() {
                        p.push(format!("expect: {e}"));
                    }
                }
                Err(e) => p.push(format!("expect is not a GradeAnswer: {e}")),
            }
        }
        _ => {}
    }
    p
}
