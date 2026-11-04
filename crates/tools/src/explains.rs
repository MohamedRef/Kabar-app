//! `kabar-tools explains`: share of closed exercises (multiple choice, fill
//! the blank, build the sentence) that say why the answer is right (#140).
//! A miss without a "why" teaches the least exactly where the distractors are
//! close (register, connectors, affixes), so from BIPA 3 and J4 on a missing
//! `explain` is reported. Advisory: it never fails.

use std::collections::BTreeMap;
use std::fs;

use anyhow::{Result, anyhow};
use clap::Args as ClapArgs;
use serde_json::Value;

use crate::paths::Root;

#[derive(ClapArgs)]
pub struct Args {
    /// Print a GitHub Actions warning for every level from BIPA 3 and J4 on
    /// with closed items that have no `explain`.
    #[arg(long)]
    pub warn: bool,
    /// List the items without `explain` in this level (e.g. bipa5).
    #[arg(long)]
    pub list: Option<String>,
}

const CLOSED: [&str; 3] = ["multiple_choice", "fill_blank", "build_sentence"];

/// Levels whose closed items should all explain themselves.
pub fn expects_explain(level: &str) -> bool {
    let n = |p: &str| level.strip_prefix(p).and_then(|n| n.parse::<u32>().ok());
    n("bipa").is_some_and(|n| n >= 3) || n("j").is_some_and(|n| n >= 4)
}

/// `(exercise index, type)` of every closed item in a lesson without `explain`.
pub fn missing(lesson: &Value) -> (usize, Vec<(usize, String)>) {
    let mut closed = 0;
    let mut out = Vec::new();
    for (i, e) in lesson.get("exercises").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let Some(kind) = e.get("type").and_then(Value::as_str).filter(|k| CLOSED.contains(k)) else {
            continue;
        };
        closed += 1;
        let explained = e.get("explain").is_some_and(|x| x.as_object().is_some_and(|o| !o.is_empty()));
        if !explained {
            out.push((i, kind.to_owned()));
        }
    }
    (closed, out)
}

pub fn run(root: &Root, args: Args) -> Result<()> {
    report_to(root, args, &mut std::io::stdout().lock())
}

pub fn report_to(root: &Root, args: Args, out: &mut dyn std::io::Write) -> Result<()> {
    // level → (closed items, missing items with their lesson)
    let mut levels: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for lang in ["id", "jv"] {
        let dir = root.path().join(format!("content/lessons/{lang}"));
        let mut files: Vec<_> = fs::read_dir(&dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        for f in files.iter().filter(|f| f.extension().is_some_and(|x| x == "yaml")) {
            let doc = kabar_content::yaml_value(f).map_err(|e| anyhow!("{e}"))?;
            let Some(id) = doc.get("id").and_then(Value::as_str) else { continue };
            let level = id.split('.').next().unwrap_or(id).to_owned();
            let (closed, gaps) = missing(&doc);
            let entry = levels.entry(level).or_default();
            entry.0 += closed;
            entry.1.extend(gaps.into_iter().map(|(i, kind)| format!("{id} exercises[{i}] {kind}")));
        }
    }
    writeln!(out, "{:<7} {:>6} {:>9} {:>8}", "level", "closed", "explained", "missing")?;
    for (level, (closed, gaps)) in &levels {
        let share = if *closed == 0 { 100 } else { 100 * (closed - gaps.len()) / closed };
        writeln!(out, "{level:<7} {closed:>6} {share:>8}% {:>8}", gaps.len())?;
    }
    if let Some(level) = &args.list {
        for g in levels.get(level).map(|(_, g)| g.as_slice()).unwrap_or_default() {
            writeln!(out, "{g}")?;
        }
    }
    if args.warn {
        for (level, (closed, gaps)) in levels.iter().filter(|(l, (_, g))| expects_explain(l) && !g.is_empty())
        {
            writeln!(
                out,
                "::warning title=Answer explanations::{level}: {} of {closed} closed items have no explain (kabar-tools explains --list {level})",
                gaps.len()
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_from_bipa3_and_j4_expect_explanations() {
        assert!(!expects_explain("bipa2"));
        assert!(expects_explain("bipa3"));
        assert!(expects_explain("bipa7"));
        assert!(!expects_explain("j3"));
        assert!(expects_explain("j4"));
    }

    #[test]
    fn counts_closed_items_without_explain() {
        let lesson = serde_json::json!({"exercises": [
            {"type": "multiple_choice", "explain": {"en": "why"}},
            {"type": "fill_blank"},
            {"type": "match"},
            {"type": "build_sentence", "explain": {}},
        ]});
        assert_eq!(missing(&lesson), (3, vec![(1, "fill_blank".into()), (3, "build_sentence".into())]));
    }

    fn squash(s: &str) -> String {
        s.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n")
    }

    fn report(s: &crate::testkit::Sandbox, warn: bool, list: Option<&str>) -> String {
        let mut out = Vec::new();
        report_to(&s.root(), Args { warn, list: list.map(Into::into) }, &mut out).unwrap();
        squash(&crate::testkit::text(out))
    }

    #[test]
    fn reports_shares_lists_gaps_and_warns_from_bipa3_on() {
        let s = crate::testkit::Sandbox::empty();
        s.write(
            "content/lessons/id/bipa1.u01.l01.yaml",
            "id: bipa1.u01.l01\nexercises:\n  - {type: multiple_choice}\n",
        );
        s.write(
            "content/lessons/id/bipa3.u02.l01.yaml",
            "id: bipa3.u02.l01\nexercises:\n  - {type: multiple_choice, explain: {en: why}}\n  - {type: match}\n  - {type: fill_blank}\n",
        );
        s.write("content/lessons/id/notes.txt", "not a lesson");
        s.write("content/lessons/id/draft.yaml", "title: no id yet\n");
        s.write(
            "content/lessons/jv/j4.u01.l01.yaml",
            "id: j4.u01.l01\nexercises:\n  - {type: build_sentence, explain: {en: why}}\n",
        );
        s.write("content/lessons/jv/j5.u01.l01.yaml", "id: j5.u01.l01\nexercises: []\n");
        let table = report(&s, false, None);
        for row in ["bipa1 1 0% 1", "bipa3 2 50% 1", "j4 1 100% 0", "j5 0 100% 0"] {
            assert!(table.contains(row), "{row:?} in\n{table}");
        }
        assert!(!table.contains("::warning"));

        let listed = report(&s, false, Some("bipa3"));
        assert!(listed.ends_with("bipa3.u02.l01 exercises[2] fill_blank"), "{listed}");
        assert!(!report(&s, false, Some("bipa9")).contains("exercises["), "an unknown level lists nothing");

        let warned = report(&s, true, None);
        assert!(
            warned.contains(
                "::warning title=Answer explanations::bipa3: 1 of 2 closed items have no explain (kabar-tools explains --list bipa3)"
            ),
            "{warned}"
        );
        assert!(!warned.contains("::bipa1:"), "BIPA 1 and 2 are not expected to explain yet");
        s.cli(&["explains", "--warn"]).unwrap();
    }

    #[test]
    fn a_missing_lesson_folder_is_an_error() {
        let s = crate::testkit::Sandbox::empty();
        let mut out = Vec::new();
        assert!(report_to(&s.root(), Args { warn: false, list: None }, &mut out).is_err());
    }
}
