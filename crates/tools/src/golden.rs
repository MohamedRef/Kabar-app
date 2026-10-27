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
use kabar_core::ai::tasks::{
    AiOutput, AnalyzeText, CheckStrokes, CoachMessage, DailyPlan, DailyRecap, ExplainSentence, GradeAnswer,
    GuessSketch, LevelCheck, MindMap, Placement, RadioScript, ReadHandwriting, RoleplayTurn,
    StructureScreenshot, TranslateText, WeeklyReport,
};
use kabar_core::learn::LearnEntry;
use kabar_core::pagi::{self, Story};
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

/// Every expected output deserializes into its task's type in
/// `kabar_core::ai::tasks` and passes the same checks the router runs on a
/// live answer; a few tasks also check the input's shape.
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
            if let Some(out) = typed::<AnalyzeText>(input, expect, &mut p) {
                let words = input.get("words").and_then(Value::as_array).map_or(0, Vec::len);
                if out.tokens.len() != words {
                    p.push(format!("expect has {} tokens for {words} words", out.tokens.len()));
                }
            }
        }
        Task::GradeAnswer => {
            if !str_field("exercise") || !str_list("references") || !str_field("answer") {
                p.push("input needs exercise, references[] and answer".into());
            }
            if expect.is_null() {
                p.push("expect is required".into());
            }
            typed::<GradeAnswer>(input, expect, &mut p);
        }
        Task::LearnEntry => {
            let sources = input.get("sources").and_then(Value::as_array);
            if !str_field("lang") || !str_field("word") || !sources.is_some_and(|s| !s.is_empty()) {
                p.push("input needs lang, word and sources[]".into());
            }
            typed::<LearnEntry>(input, expect, &mut p);
        }
        Task::ExplainSentence => {
            if !str_field("text") || !str_field("lang") || !str_field("voice") {
                p.push("input needs text, lang and voice".into());
            }
            typed::<ExplainSentence>(input, expect, &mut p);
        }
        Task::TranslateText => {
            if !str_field("lang") || !str_list("lines") {
                p.push("input needs lang and lines[]".into());
            }
            typed::<TranslateText>(input, expect, &mut p);
        }
        Task::RoleplayTurn => {
            if !str_field("persona") || !str_field("level") || !str_field("learner") {
                p.push("input needs persona, level and learner".into());
            }
            typed::<RoleplayTurn>(input, expect, &mut p);
        }
        Task::PlacementTest => {
            typed::<Placement>(input, expect, &mut p);
        }
        Task::LevelCheck => {
            typed::<LevelCheck>(input, expect, &mut p);
        }
        Task::DailyPlan => {
            typed::<DailyPlan>(input, expect, &mut p);
        }
        Task::DailyRecap => {
            typed::<DailyRecap>(input, expect, &mut p);
        }
        Task::WeeklyReport => {
            typed::<WeeklyReport>(input, expect, &mut p);
        }
        Task::CoachMessage => {
            if !str_field("trigger") {
                p.push("input needs trigger".into());
            }
            typed::<CoachMessage>(input, expect, &mut p);
        }
        Task::RadioScript => {
            if !str_list("voices") || !input.get("target_words").is_some_and(Value::is_array) {
                p.push("input needs voices[] and target_words[]".into());
            }
            typed::<RadioScript>(input, expect, &mut p);
        }
        Task::StructureScreenshot => {
            typed::<StructureScreenshot>(input, expect, &mut p);
        }
        Task::ReadHandwriting => {
            typed::<ReadHandwriting>(input, expect, &mut p);
        }
        Task::GuessSketch => {
            typed::<GuessSketch>(input, expect, &mut p);
        }
        Task::CheckStrokes => {
            typed::<CheckStrokes>(input, expect, &mut p);
        }
        Task::MindMap => {
            typed::<MindMap>(input, expect, &mut p);
        }
        Task::StoryEpisode => {
            let series = input.get("series").and_then(Value::as_str);
            if !matches!(series, Some("sinetron" | "kabar_pagi")) {
                p.push("input.series must be sinetron or kabar_pagi".into());
            }
            let level = input.get("level").and_then(Value::as_str).unwrap_or_default();
            if !is_story_level(level) {
                p.push(format!("input.level {level:?} must be bipa1..bipa7 or j1..j6"));
            }
            if !str_list("target_words") {
                p.push("input needs target_words[]".into());
            }
            let length = input.get("length_words").and_then(Value::as_u64);
            match length {
                Some(n) if n > 0 && n <= pagi::MAX_WORDS as u64 => {}
                _ => p.push(format!("input.length_words must be 1..={}", pagi::MAX_WORDS)),
            }
            match serde_json::from_value::<Story>(expect.clone()) {
                Ok(out) => {
                    if let Err(e) = out.validate() {
                        p.push(format!("expect: {e}"));
                    }
                    if let Some(n) = length {
                        let words = out.word_count() as f64;
                        let (lo, hi) = (n as f64 * 0.8, n as f64 * 1.2);
                        if words < lo || words > hi {
                            p.push(format!("expect has {words} Indonesian words, outside ±20% of {n}"));
                        }
                    }
                    if out.level != level {
                        p.push(format!("expect.level {:?} differs from input.level {level:?}", out.level));
                    }
                    let asked = ["target_words", "due_words"]
                        .iter()
                        .filter_map(|k| input.get(*k).and_then(Value::as_array))
                        .flatten()
                        .filter_map(Value::as_str);
                    let missing: Vec<&str> = asked.filter(|w| !out.words.iter().any(|x| x == w)).collect();
                    if !missing.is_empty() {
                        p.push(format!("expect.words is missing {}", missing.join(", ")));
                    }
                    if series == Some("sinetron")
                        && out.summary.as_deref().is_none_or(|s| s.trim().is_empty())
                    {
                        p.push("sinetron expect needs a summary".into());
                    }
                }
                Err(e) => p.push(format!("expect is not a Story: {e}")),
            }
        }
    }
    p
}

/// Deserializes and checks `expect` as `T`; `None` (and an issue) when it
/// doesn't fit, `None` silently when there is no expected answer.
fn typed<T: AiOutput>(input: &Value, expect: &Value, p: &mut Vec<String>) -> Option<T> {
    if expect.is_null() {
        return None;
    }
    match serde_json::from_value::<T>(expect.clone()) {
        Ok(out) => {
            let checked = out
                .validate()
                .and_then(|()| if T::EXPECT_IS_WHOLE_ANSWER { out.check(input) } else { Ok(()) });
            if let Err(e) = checked {
                p.push(format!("expect: {e}"));
            }
            Some(out)
        }
        Err(e) => {
            p.push(format!("expect is not a {} output: {e}", T::TASK.as_str()));
            None
        }
    }
}

/// `bipa1`..`bipa7` or `j1`..`j6`, lowercase, as the app sends them.
fn is_story_level(level: &str) -> bool {
    (1..=7).any(|n| level == format!("bipa{n}")) || (1..=6).any(|n| level == format!("j{n}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The §24 morphology test texts as a small story.
    fn story(level: &str, words: &[&str], summary: Option<&str>) -> Value {
        let s = |id: &str| json!({"id": id, "arz": "أنا بقرا كتاب.", "ar": "أقرأ كتابًا.", "en": "I read a book."});
        let mut v = json!({
            "title": "Hari Minggu",
            "level": level,
            "sentences": [
                s("Apa kabar?"), s("Saya sedang membaca buku."), s("Rumahnya dibersihkan kemarin."),
                s("Anak-anak bermain di taman."), s("Kesehatan itu penting."), s("Bukunya dibaca lagi."),
            ],
            "words": words,
        });
        if let Some(sum) = summary {
            v["summary"] = json!(sum);
        }
        v
    }

    fn input(series: &str, level: &str, length: u64) -> Value {
        json!({
            "series": series,
            "level": level,
            "target_words": ["baca"],
            "due_words": ["anak"],
            "length_words": length,
        })
    }

    fn issues(input: Value, expect: Value) -> String {
        check_typed(Task::StoryEpisode, &input, &expect).join(" | ")
    }

    #[test]
    fn story_episode_accepts_a_good_item() {
        assert_eq!(issues(input("kabar_pagi", "bipa1", 20), story("bipa1", &["baca", "anak"], None)), "");
        let sinetron = story("j1", &["baca", "anak"], Some("Sari reads a book."));
        assert_eq!(issues(input("sinetron", "j1", 20), sinetron), "");
    }

    #[test]
    fn story_episode_reports_each_problem() {
        let p = issues(input("sinetron", "BIPA 1", 300), story("bipa1", &["baca"], None));
        for want in [
            "input.level",
            "length_words",
            "outside ±20%",
            "differs from input.level",
            "missing anak",
            "summary",
        ] {
            assert!(p.contains(want), "{p:?} should mention {want:?}");
        }
        let p = issues(input("radio", "j1", 20), story("j1", &["baca", "anak", "pasar"], Some("A summary.")));
        assert!(p.contains("input.series") && p.contains("`pasar`"), "{p}");
        assert!(issues(input("kabar_pagi", "bipa1", 20), json!({"title": "x"})).contains("not a Story"));
    }
}
