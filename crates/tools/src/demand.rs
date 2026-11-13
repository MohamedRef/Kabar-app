//! `kabar-tools demand`: what an Indonesian lesson asks of the learner, per
//! level (#633). The level labels climb from A1 to C2+, so the task demand
//! inside the fixed 7-minute shape must climb with them: longer listening
//! texts, comprehension checked in Indonesian rather than Arabic, longer
//! sentences to say and write.
//!
//! The report prints, per BIPA level: lessons, dialogue words per line and
//! lines per lesson, words per speaking / dictation sentence, words per
//! listening text, and the share of `listen` and `multiple_choice` items
//! whose options are all in Arabic.
//!
//! `--check` compares each level with `content/demand-limits.tsv` (a ratchet,
//! like `dialogue-over.tsv`): a level whose Arabic-option shares rise above
//! its limit, or whose listening texts get shorter than its floor, fails.
//! A batch that improves a level lowers (or raises) its line in the same PR.

use std::collections::BTreeMap;
use std::fs;

use anyhow::{Result, anyhow, bail};
use clap::Args as ClapArgs;
use serde_json::Value;

use crate::paths::Root;

#[derive(ClapArgs)]
pub struct Args {
    /// Fail when a level is worse than its line in `content/demand-limits.tsv`.
    #[arg(long)]
    pub check: bool,
}

pub const LIMITS: &str = "content/demand-limits.tsv";

/// Counts for one level.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Level {
    pub lessons: usize,
    dialogue_words: usize,
    dialogue_lines: usize,
    said_words: usize,
    said_sentences: usize,
    listen_words: usize,
    pub listen: usize,
    pub listen_arabic: usize,
    pub choice: usize,
    pub choice_arabic: usize,
}

fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 { 0.0 } else { a as f64 / b as f64 }
}

impl Level {
    pub fn words_per_line(&self) -> f64 {
        ratio(self.dialogue_words, self.dialogue_lines)
    }
    pub fn lines_per_lesson(&self) -> f64 {
        ratio(self.dialogue_lines, self.lessons)
    }
    pub fn words_per_sentence(&self) -> f64 {
        ratio(self.said_words, self.said_sentences)
    }
    pub fn listen_words(&self) -> f64 {
        ratio(self.listen_words, self.listen)
    }
    /// Percent of `listen` items whose options are all Arabic.
    pub fn listen_arabic_pct(&self) -> f64 {
        100.0 * ratio(self.listen_arabic, self.listen)
    }
    /// Percent of `multiple_choice` items whose options are all Arabic.
    pub fn choice_arabic_pct(&self) -> f64 {
        100.0 * ratio(self.choice_arabic, self.choice)
    }
}

fn words(t: &str) -> usize {
    t.split_whitespace().count()
}

fn sentences(t: &str) -> usize {
    t.split(['.', '!', '?']).filter(|s| s.chars().any(char::is_alphanumeric)).count().max(1)
}

fn is_arabic(t: &str) -> bool {
    t.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

fn all_arabic(e: &Value) -> bool {
    let opts: Vec<&str> =
        e.get("options").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    !opts.is_empty() && opts.iter().all(|o| is_arabic(o))
}

/// Adds one lesson file's counts to its level.
pub fn add(level: &mut Level, lesson: &Value) {
    level.lessons += 1;
    let lines = lesson.pointer("/dialogue/lines").and_then(Value::as_array).into_iter().flatten();
    for l in lines.filter_map(|l| l.get("text").and_then(Value::as_str)) {
        level.dialogue_lines += 1;
        level.dialogue_words += words(l);
    }
    for e in lesson.get("exercises").and_then(Value::as_array).into_iter().flatten() {
        let text = e.get("text").and_then(Value::as_str).unwrap_or_default();
        match e.get("type").and_then(Value::as_str).unwrap_or_default() {
            "speaking" | "dictation" => {
                level.said_words += words(text);
                level.said_sentences += sentences(text);
            }
            "listen" => {
                level.listen += 1;
                level.listen_words += words(text);
                level.listen_arabic += usize::from(all_arabic(e));
            }
            "multiple_choice" => {
                level.choice += 1;
                level.choice_arabic += usize::from(all_arabic(e));
            }
            _ => {}
        }
    }
}

/// Every Indonesian level, from the lesson files.
pub fn levels(root: &Root) -> Result<BTreeMap<String, Level>> {
    let dir = root.path().join("content/lessons/id");
    let mut files: Vec<_> = fs::read_dir(&dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    files.sort();
    let mut out: BTreeMap<String, Level> = BTreeMap::new();
    for f in files.iter().filter(|f| f.extension().is_some_and(|x| x == "yaml")) {
        let doc = kabar_content::yaml_value(f).map_err(|e| anyhow!("{e}"))?;
        let Some(id) = doc.get("id").and_then(Value::as_str) else { continue };
        let level = id.split('.').next().unwrap_or(id).to_owned();
        add(out.entry(level).or_default(), &doc);
    }
    Ok(out)
}

/// A level's line in the limits file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limit {
    pub max_listen_arabic_pct: f64,
    pub max_choice_arabic_pct: f64,
    pub min_listen_words: f64,
}

/// `level<TAB>max listen Arabic %<TAB>max choice Arabic %<TAB>min listen words`.
pub fn limits(root: &Root) -> Result<BTreeMap<String, Limit>> {
    let text = fs::read_to_string(root.path().join(LIMITS)).map_err(|e| anyhow!("{LIMITS}: {e}"))?;
    let mut out = BTreeMap::new();
    for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#')) {
        let cols: Vec<&str> = line.split('\t').collect();
        let num = |i: usize| -> Result<f64> {
            cols.get(i)
                .and_then(|c| c.trim().parse().ok())
                .ok_or_else(|| anyhow!("{LIMITS}:{}: column {} is not a number", n + 1, i + 1))
        };
        out.insert(
            cols[0].trim().to_owned(),
            Limit {
                max_listen_arabic_pct: num(1)?,
                max_choice_arabic_pct: num(2)?,
                min_listen_words: num(3)?,
            },
        );
    }
    Ok(out)
}

/// What is wrong with `level` against its limit, if anything.
pub fn breaches(name: &str, level: &Level, limit: &Limit) -> Vec<String> {
    let mut out = Vec::new();
    if level.listen_arabic_pct() > limit.max_listen_arabic_pct + 1e-9 {
        out.push(format!(
            "{name}: {:.0}% of listen items have only Arabic options (limit {:.0}%)",
            level.listen_arabic_pct(),
            limit.max_listen_arabic_pct
        ));
    }
    if level.choice_arabic_pct() > limit.max_choice_arabic_pct + 1e-9 {
        out.push(format!(
            "{name}: {:.0}% of multiple-choice items have only Arabic options (limit {:.0}%)",
            level.choice_arabic_pct(),
            limit.max_choice_arabic_pct
        ));
    }
    if level.listen_words() + 1e-9 < limit.min_listen_words {
        out.push(format!(
            "{name}: listening texts average {:.1} words (floor {:.1})",
            level.listen_words(),
            limit.min_listen_words
        ));
    }
    out
}

pub fn run(root: &Root, args: Args) -> Result<()> {
    report_to(root, args, &mut std::io::stdout().lock())
}

pub fn report_to(root: &Root, args: Args, out: &mut dyn std::io::Write) -> Result<()> {
    let levels = levels(root)?;
    writeln!(
        out,
        "{:<6} {:>7} {:>10} {:>11} {:>14} {:>12} {:>13} {:>13}",
        "level",
        "lessons",
        "words/line",
        "lines/dlg",
        "words/sentence",
        "listen words",
        "listen ar %",
        "choice ar %"
    )?;
    for (name, l) in &levels {
        writeln!(
            out,
            "{name:<6} {:>7} {:>10.1} {:>11.1} {:>14.1} {:>12.1} {:>12.0}% {:>12.0}%",
            l.lessons,
            l.words_per_line(),
            l.lines_per_lesson(),
            l.words_per_sentence(),
            l.listen_words(),
            l.listen_arabic_pct(),
            l.choice_arabic_pct()
        )?;
    }
    if args.check {
        let limits = limits(root)?;
        let problems: Vec<String> = levels
            .iter()
            .filter_map(|(name, l)| limits.get(name).map(|lim| breaches(name, l, lim)))
            .flatten()
            .collect();
        for p in &problems {
            writeln!(out, "{p}")?;
        }
        if !problems.is_empty() {
            bail!("lesson demand went down; see {LIMITS} and #633");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lesson(yaml: &str) -> Value {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    #[test]
    fn counts_demand_per_lesson() {
        let mut l = Level::default();
        add(
            &mut l,
            &lesson(
                "id: bipa5.u01.l01
dialogue: {lines: [{text: Satu dua tiga empat}, {text: Lima enam}]}
exercises:
  - {type: speaking, text: Saya pergi. Dia pulang ke rumah.}
  - {type: listen, text: Rumput laut dipanen sekitar empat puluh lima hari setelah ditanam., options: [بعد ٤٥ يوم, بعد يومين]}
  - {type: listen, text: Saya lapar, options: [Saya lapar, Saya haus]}
  - {type: multiple_choice, options: [Setuju, Menolak]}
  - {type: match_pairs}
",
            ),
        );
        assert_eq!(l.lessons, 1);
        assert_eq!(l.words_per_line(), 3.0);
        assert_eq!(l.lines_per_lesson(), 2.0);
        assert_eq!(l.words_per_sentence(), 3.0);
        assert_eq!(l.listen_words(), 6.0);
        assert_eq!(l.listen_arabic_pct(), 50.0);
        assert_eq!(l.choice_arabic_pct(), 0.0);
    }

    #[test]
    fn options_count_as_arabic_only_when_every_one_is() {
        assert!(all_arabic(&lesson("options: [نعم, لا]")));
        assert!(!all_arabic(&lesson("options: [نعم, Tidak]")));
        assert!(!all_arabic(&lesson("{}")), "typed answers have no options");
    }

    #[test]
    fn breaches_name_each_limit() {
        let mut l = Level::default();
        add(
            &mut l,
            &lesson(
                "exercises: [{type: listen, text: Satu dua, options: [نعم]}, {type: multiple_choice, options: [لا]}]",
            ),
        );
        let ok = Limit { max_listen_arabic_pct: 100.0, max_choice_arabic_pct: 100.0, min_listen_words: 2.0 };
        assert!(breaches("bipa5", &l, &ok).is_empty());
        let tight = Limit { max_listen_arabic_pct: 30.0, max_choice_arabic_pct: 5.0, min_listen_words: 20.0 };
        let b = breaches("bipa5", &l, &tight);
        assert_eq!(b.len(), 3, "{b:?}");
        assert!(b[0].starts_with("bipa5: 100% of listen items"), "{b:?}");
    }

    #[test]
    fn check_reads_the_limits_file_and_fails_a_worse_level() {
        let s = crate::testkit::Sandbox::empty();
        s.write(
            "content/lessons/id/bipa5.u01.l01.yaml",
            "id: bipa5.u01.l01\nexercises:\n  - {type: listen, text: Satu dua tiga, options: [نعم, لا]}\n",
        );
        s.write("content/lessons/id/notes.yaml", "title: no id\n");
        s.write(LIMITS, "# level\tlisten\tchoice\twords\nbipa5\t100\t0\t3\n");
        let mut out = Vec::new();
        report_to(&s.root(), Args { check: true }, &mut out).unwrap();
        let text = crate::testkit::text(out);
        assert!(text.contains("bipa5"), "{text}");
        s.write(LIMITS, "bipa5\t50\t0\t3\n");
        let mut out = Vec::new();
        assert!(report_to(&s.root(), Args { check: true }, &mut out).is_err());
        assert!(crate::testkit::text(out).contains("limit 50%"));
        s.write(LIMITS, "bipa5\tmany\t0\t3\n");
        assert!(s.cli(&["demand", "--check"]).is_err());
        assert!(s.cli(&["demand"]).is_ok(), "the report alone never reads the limits");
    }
}
