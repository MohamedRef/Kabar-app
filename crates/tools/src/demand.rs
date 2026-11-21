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
//!
//! It also measures option shape for every level, Indonesian and Javanese
//! (#655, #661): the key's rank by length (longest, middle, shortest; ties
//! split evenly across occupied ranks) and the median key / mean-wrong
//! length ratio (`RATIO_LOW`–`RATIO_HIGH`). A learner who can score by shape
//! is not listening, whether the key hides in the longest option, avoids it,
//! or piles up in the middle (#661). Each rank share and length ratio may
//! only stay inside its band (`BAND_LOW`–`BAND_HIGH`, `RATIO_LOW`–`RATIO_HIGH`)
//! or move toward it. `--longest <level>` lists the items whose answer is
//! the single longest option, with lengths, so a rewrite batch knows where
//! to start.

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
    /// List the items of this level (e.g. `bipa5`, `j2`) whose answer is the
    /// single longest option.
    #[arg(long, value_name = "LEVEL")]
    pub longest: Option<String>,
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
    /// `listen` / `multiple_choice` items with options and an answer index.
    pub listen_shaped: usize,
    pub choice_shaped: usize,
    /// How often each length rank (longest, middle, shortest) holds the answer.
    pub listen_longest: f64,
    pub listen_middle: f64,
    pub listen_shortest: f64,
    pub choice_longest: f64,
    pub choice_middle: f64,
    pub choice_shortest: f64,
    /// Key length / mean wrong-option length for each shaped item.
    pub listen_ratios: Vec<f64>,
    pub choice_ratios: Vec<f64>,
}

fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 { 0.0 } else { a as f64 / b as f64 }
}

fn pct_of(sum: f64, n: usize) -> f64 {
    if n == 0 { 0.0 } else { 100.0 * sum / n as f64 }
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 1.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) { (sorted[mid - 1] + sorted[mid]) / 2.0 } else { sorted[mid] }
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
    /// Percent of `listen` items where the longest option is the answer.
    pub fn listen_longest_pct(&self) -> f64 {
        pct_of(self.listen_longest, self.listen_shaped)
    }
    /// Percent of `listen` items where a middle-length option is the answer.
    pub fn listen_middle_pct(&self) -> f64 {
        pct_of(self.listen_middle, self.listen_shaped)
    }
    /// Percent of `listen` items where the shortest option is the answer.
    pub fn listen_shortest_pct(&self) -> f64 {
        pct_of(self.listen_shortest, self.listen_shaped)
    }
    /// Percent of `multiple_choice` items where the longest option is the answer.
    pub fn choice_longest_pct(&self) -> f64 {
        pct_of(self.choice_longest, self.choice_shaped)
    }
    /// Percent of `multiple_choice` items where a middle-length option is the answer.
    pub fn choice_middle_pct(&self) -> f64 {
        pct_of(self.choice_middle, self.choice_shaped)
    }
    /// Percent of `multiple_choice` items where the shortest option is the answer.
    pub fn choice_shortest_pct(&self) -> f64 {
        pct_of(self.choice_shortest, self.choice_shaped)
    }
    /// Median key / mean-wrong length ratio across `listen` items.
    pub fn listen_ratio(&self) -> f64 {
        median(&self.listen_ratios)
    }
    /// Median key / mean-wrong length ratio across `multiple_choice` items.
    pub fn choice_ratio(&self) -> f64 {
        median(&self.choice_ratios)
    }
}

/// An option's length as the eye sees it: characters, outer spaces trimmed.
pub fn option_len(o: &str) -> usize {
    o.trim().chars().count()
}

/// The options and answer index of a closed item, if it has both.
fn shaped(e: &Value) -> Option<(Vec<&str>, usize)> {
    let opts: Vec<&str> = e.get("options")?.as_array()?.iter().filter_map(Value::as_str).collect();
    let answer = usize::try_from(e.get("answer")?.as_u64()?).ok()?;
    (opts.len() >= 2 && answer < opts.len()).then_some((opts, answer))
}

/// The key's length-rank weights `[longest, middle, shortest]` on one item,
/// splitting ties evenly across the ranks they occupy (#661).
pub fn rank_scores(opts: &[&str], answer: usize) -> [f64; 3] {
    if opts.len() < 2 || answer >= opts.len() {
        return [0.0; 3];
    }
    let lens: Vec<usize> = opts.iter().map(|o| option_len(o)).collect();
    let key = lens[answer];
    let longer = lens.iter().filter(|&&l| l > key).count();
    let ties = lens.iter().filter(|&&l| l == key).count();
    let share = 1.0 / ties as f64;
    let mut out = [0.0; 3];
    for rank in longer..longer + ties {
        if rank == 0 {
            out[0] += share;
        } else if rank + 1 == lens.len() {
            out[2] += share;
        } else {
            out[1] += share;
        }
    }
    out
}

/// What "pick the longest option" scores on one item: 1 when the answer is
/// the only longest, 1/n when it ties with n−1 others, else 0.
pub fn longest_score(opts: &[&str], answer: usize) -> f64 {
    rank_scores(opts, answer)[0]
}

/// Key length divided by the mean length of the wrong options (#661).
pub fn option_ratio(opts: &[&str], answer: usize) -> Option<f64> {
    if opts.len() < 2 || answer >= opts.len() {
        return None;
    }
    let key = option_len(opts[answer]) as f64;
    let wrong_sum: usize =
        opts.iter().enumerate().filter(|&(i, _)| i != answer).map(|(_, o)| option_len(o)).sum();
    let mean_wrong = wrong_sum as f64 / (opts.len() - 1) as f64;
    Some(if mean_wrong > 0.0 { key / mean_wrong } else { 1.0 })
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
                if let Some((opts, answer)) = shaped(e) {
                    let [longest, middle, shortest] = rank_scores(&opts, answer);
                    level.listen_shaped += 1;
                    level.listen_longest += longest;
                    level.listen_middle += middle;
                    level.listen_shortest += shortest;
                    if let Some(r) = option_ratio(&opts, answer) {
                        level.listen_ratios.push(r);
                    }
                }
            }
            "multiple_choice" => {
                level.choice += 1;
                level.choice_arabic += usize::from(all_arabic(e));
                if let Some((opts, answer)) = shaped(e) {
                    let [longest, middle, shortest] = rank_scores(&opts, answer);
                    level.choice_shaped += 1;
                    level.choice_longest += longest;
                    level.choice_middle += middle;
                    level.choice_shortest += shortest;
                    if let Some(r) = option_ratio(&opts, answer) {
                        level.choice_ratios.push(r);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The lesson files of every language, sorted.
fn lesson_files(root: &Root) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for lang in ["id", "jv"] {
        let dir = root.path().join("content/lessons").join(lang);
        if !dir.is_dir() {
            continue;
        }
        files.extend(fs::read_dir(&dir)?.filter_map(|e| e.ok().map(|e| e.path())));
    }
    files.retain(|f| f.extension().is_some_and(|x| x == "yaml"));
    files.sort();
    Ok(files)
}

/// Every level (BIPA 1–7, then J1–J6), from the lesson files.
pub fn levels(root: &Root) -> Result<BTreeMap<String, Level>> {
    let mut out: BTreeMap<String, Level> = BTreeMap::new();
    for f in lesson_files(root)? {
        let doc = kabar_content::yaml_value(&f).map_err(|e| anyhow!("{e}"))?;
        let Some(id) = doc.get("id").and_then(Value::as_str) else { continue };
        let level = id.split('.').next().unwrap_or(id).to_owned();
        add(out.entry(level).or_default(), &doc);
    }
    Ok(out)
}

/// One item whose answer is the single longest option.
#[derive(Debug, Clone, PartialEq)]
pub struct Longest {
    pub lesson: String,
    pub exercise: String,
    pub kind: String,
    /// Option lengths in characters, answer first.
    pub answer_len: usize,
    pub wrong_lens: Vec<usize>,
}

/// The items of `level` whose answer is the single longest option.
pub fn longest_items(root: &Root, level: &str) -> Result<Vec<Longest>> {
    let mut out = Vec::new();
    for f in lesson_files(root)? {
        let doc = kabar_content::yaml_value(&f).map_err(|e| anyhow!("{e}"))?;
        let Some(id) = doc.get("id").and_then(Value::as_str) else { continue };
        if id.split('.').next() != Some(level) {
            continue;
        }
        for e in doc.get("exercises").and_then(Value::as_array).into_iter().flatten() {
            let kind = e.get("type").and_then(Value::as_str).unwrap_or_default();
            if !matches!(kind, "listen" | "multiple_choice") {
                continue;
            }
            let Some((opts, answer)) = shaped(e) else { continue };
            if longest_score(&opts, answer) < 1.0 {
                continue;
            }
            out.push(Longest {
                lesson: id.to_owned(),
                exercise: e.get("id").and_then(Value::as_str).unwrap_or("?").to_owned(),
                kind: kind.to_owned(),
                answer_len: option_len(opts[answer]),
                wrong_lens: opts
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != answer)
                    .map(|(_, o)| option_len(o))
                    .collect(),
            });
        }
    }
    Ok(out)
}

/// A level's line in the limits file. `None` is a `-` column: no limit.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Limit {
    pub max_listen_arabic_pct: Option<f64>,
    pub max_choice_arabic_pct: Option<f64>,
    pub min_listen_words: Option<f64>,
    /// Shares last recorded for the level, not ceilings: see [`off_band`].
    pub seen_listen_longest_pct: Option<f64>,
    pub seen_choice_longest_pct: Option<f64>,
    pub seen_listen_middle_pct: Option<f64>,
    pub seen_listen_shortest_pct: Option<f64>,
    pub seen_choice_middle_pct: Option<f64>,
    pub seen_choice_shortest_pct: Option<f64>,
    /// Median key / mean-wrong length ratios: see [`off_ratio_band`].
    pub seen_listen_ratio: Option<f64>,
    pub seen_choice_ratio: Option<f64>,
}

/// The band around chance for each length rank (three options: chance 33%).
pub const BAND_LOW: f64 = 25.0;
pub const BAND_HIGH: f64 = 40.0;

/// The band around parity for median key / mean-wrong option length (#661).
pub const RATIO_LOW: f64 = 0.9;
pub const RATIO_HIGH: f64 = 1.1;

/// How far a rank share is from `BAND_LOW`–`BAND_HIGH`; 0 inside it.
pub fn off_band(pct: f64) -> f64 {
    if pct < BAND_LOW {
        BAND_LOW - pct
    } else if pct > BAND_HIGH {
        pct - BAND_HIGH
    } else {
        0.0
    }
}

/// How far a key / wrong length ratio is from `RATIO_LOW`–`RATIO_HIGH`; 0 inside it.
pub fn off_ratio_band(r: f64) -> f64 {
    if r < RATIO_LOW {
        RATIO_LOW - r
    } else if r > RATIO_HIGH {
        r - RATIO_HIGH
    } else {
        0.0
    }
}

/// Parses `content/demand-limits.tsv` (6 columns, or 12 columns with middle /
/// shortest rank shares and median key / wrong length ratios; `-` is no limit).
pub fn limits(root: &Root) -> Result<BTreeMap<String, Limit>> {
    let text = fs::read_to_string(root.path().join(LIMITS)).map_err(|e| anyhow!("{LIMITS}: {e}"))?;
    let mut out = BTreeMap::new();
    for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#')) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 6 && cols.len() != 12 {
            bail!("{LIMITS}:{}: expected 6 or 12 tab-separated columns, found {}", n + 1, cols.len());
        }
        let num = |i: usize| -> Result<Option<f64>> {
            let Some(raw) = cols.get(i) else { return Ok(None) };
            let c = raw.trim();
            if c == "-" {
                return Ok(None);
            }
            c.parse()
                .map(Some)
                .map_err(|_| anyhow!("{LIMITS}:{}: column {} is not a number or -", n + 1, i + 1))
        };
        out.insert(
            cols[0].trim().to_owned(),
            Limit {
                max_listen_arabic_pct: num(1)?,
                max_choice_arabic_pct: num(2)?,
                min_listen_words: num(3)?,
                seen_listen_longest_pct: num(4)?,
                seen_choice_longest_pct: num(5)?,
                seen_listen_middle_pct: num(6)?,
                seen_listen_shortest_pct: num(7)?,
                seen_choice_middle_pct: num(8)?,
                seen_choice_shortest_pct: num(9)?,
                seen_listen_ratio: num(10)?,
                seen_choice_ratio: num(11)?,
            },
        );
    }
    Ok(out)
}

/// What is wrong with `level` against its limit, if anything.
pub fn breaches(name: &str, level: &Level, limit: &Limit) -> Vec<String> {
    let mut out = Vec::new();
    let mut over = |value: f64, max: Option<f64>, what: &str| {
        if let Some(max) = max.filter(|&m| value > m + 0.05) {
            out.push(format!("{name}: {value:.1}% of {what} (limit {max:.1}%)"));
        }
    };
    over(level.listen_arabic_pct(), limit.max_listen_arabic_pct, "listen items have only Arabic options");
    over(
        level.choice_arabic_pct(),
        limit.max_choice_arabic_pct,
        "multiple-choice items have only Arabic options",
    );
    let mut shape = |value: f64, seen: Option<f64>, what: &str| {
        if let Some(seen) = seen.filter(|&s| off_band(value) > off_band(s) + 0.05) {
            out.push(format!(
                "{name}: {value:.1}% of {what} (recorded {seen:.1}%; stay inside {BAND_LOW:.0}–{BAND_HIGH:.0}% or move toward it)"
            ));
        }
    };
    for (value, seen, what) in [
        (
            level.listen_longest_pct(),
            limit.seen_listen_longest_pct,
            "listen items have the longest option as the answer",
        ),
        (
            level.listen_middle_pct(),
            limit.seen_listen_middle_pct,
            "listen items have the middle-length option as the answer",
        ),
        (
            level.listen_shortest_pct(),
            limit.seen_listen_shortest_pct,
            "listen items have the shortest option as the answer",
        ),
        (
            level.choice_longest_pct(),
            limit.seen_choice_longest_pct,
            "multiple-choice items have the longest option as the answer",
        ),
        (
            level.choice_middle_pct(),
            limit.seen_choice_middle_pct,
            "multiple-choice items have the middle-length option as the answer",
        ),
        (
            level.choice_shortest_pct(),
            limit.seen_choice_shortest_pct,
            "multiple-choice items have the shortest option as the answer",
        ),
    ] {
        shape(value, seen, what);
    }
    let mut ratio_check = |value: f64, seen: Option<f64>, kind: &str| {
        if let Some(seen) = seen.filter(|&s| off_ratio_band(value) > off_ratio_band(s) + 0.005) {
            out.push(format!(
                "{name}: {kind} key/wrong length ratio is {value:.2} (recorded {seen:.2}; stay inside {RATIO_LOW:.2}–{RATIO_HIGH:.2} or move toward it)"
            ));
        }
    };
    ratio_check(level.listen_ratio(), limit.seen_listen_ratio, "listen");
    ratio_check(level.choice_ratio(), limit.seen_choice_ratio, "multiple-choice");
    if let Some(min) = limit.min_listen_words.filter(|&m| level.listen_words() + 0.05 < m) {
        out.push(format!(
            "{name}: listening texts average {:.1} words (floor {min:.1})",
            level.listen_words()
        ));
    }
    out
}

pub fn run(root: &Root, args: Args) -> Result<()> {
    report_to(root, args, &mut std::io::stdout().lock())
}

pub fn report_to(root: &Root, args: Args, out: &mut dyn std::io::Write) -> Result<()> {
    if let Some(level) = &args.longest {
        let items = longest_items(root, level)?;
        for i in &items {
            let wrong: Vec<String> = i.wrong_lens.iter().map(usize::to_string).collect();
            writeln!(out, "{}\t{}\t{}\t{}\t{}", i.lesson, i.exercise, i.kind, i.answer_len, wrong.join(","))?;
        }
        writeln!(out, "{level}: answer is the single longest option in {} of the items above", items.len())?;
        return Ok(());
    }
    let levels = levels(root)?;
    writeln!(
        out,
        "{:<6} {:>7} {:>10} {:>11} {:>14} {:>12} {:>13} {:>13} {:>16} {:>16}",
        "level",
        "lessons",
        "words/line",
        "lines/dlg",
        "words/sentence",
        "listen words",
        "listen ar %",
        "choice ar %",
        "listen longest %",
        "choice longest %"
    )?;
    for (name, l) in &levels {
        writeln!(
            out,
            "{name:<6} {:>7} {:>10.1} {:>11.1} {:>14.1} {:>12.1} {:>12.0}% {:>12.0}% {:>15.1}% {:>15.1}%",
            l.lessons,
            l.words_per_line(),
            l.lines_per_lesson(),
            l.words_per_sentence(),
            l.listen_words(),
            l.listen_arabic_pct(),
            l.choice_arabic_pct(),
            l.listen_longest_pct(),
            l.choice_longest_pct()
        )?;
    }
    writeln!(
        out,
        "\n{:<6} {:>22} {:>13} {:>22} {:>13}",
        "level",
        "listen long/mid/short",
        "listen ratio",
        "choice long/mid/short",
        "choice ratio"
    )?;
    for (name, l) in &levels {
        let listen_ranks = format!(
            "{:.1}%/{:.1}%/{:.1}%",
            l.listen_longest_pct(),
            l.listen_middle_pct(),
            l.listen_shortest_pct()
        );
        let choice_ranks = format!(
            "{:.1}%/{:.1}%/{:.1}%",
            l.choice_longest_pct(),
            l.choice_middle_pct(),
            l.choice_shortest_pct()
        );
        writeln!(
            out,
            "{name:<6} {:>22} {:>13.2} {:>22} {:>13.2}",
            listen_ranks,
            l.listen_ratio(),
            choice_ranks,
            l.choice_ratio()
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
            bail!("lesson demand went down; see {LIMITS}, #633, #655 and #661");
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
    fn longest_option_counts_ties_evenly() {
        assert_eq!(longest_score(&["Ya.", "Tidak sama sekali.", "Ya"], 1), 1.0);
        assert_eq!(longest_score(&["Ya.", "Tidak sama sekali.", "Ya"], 0), 0.0);
        assert_eq!(longest_score(&["Satu", "Dua.", "Tiga"], 0), 1.0 / 3.0, "three of four characters tie");
        assert_eq!(rank_scores(&["Ya.", "Tidak sama sekali.", "Ya"], 0), [0.0, 1.0, 0.0]);
        assert_eq!(rank_scores(&["Ya.", "Tidak sama sekali.", "Ya"], 2), [0.0, 0.0, 1.0]);
        assert_eq!(rank_scores(&["Satu", "Dua.", "Tiga"], 0), [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]);
        assert_eq!(option_ratio(&["Satu", "Dua.", "Tiga"], 0), Some(1.0));
        assert_eq!(option_ratio(&["Panjang.", "Ya", "Ya"], 0), Some(4.0));
        assert_eq!(option_len("  Ini  "), 3, "outer spaces do not count");
        let mut l = Level::default();
        add(
            &mut l,
            &lesson(
                "exercises:
  - {type: listen, text: Satu, options: [Pendek., Jawaban yang panjang sekali., Sedang saja.], answer: 2}
  - {type: listen, text: Dua, options: [Jawaban yang panjang sekali., Pendek.], answer: 1}
  - {type: listen, text: Tiga, options: [Sama, Sama], answer: 0}
  - {type: listen, text: Empat, options: [Diketik]}
  - {type: multiple_choice, options: [Panjang sekali, Ya], answer: 0}
",
            ),
        );
        assert_eq!(l.listen_shaped, 3, "an item without an answer index has no shape");
        let listen_sum = l.listen_longest_pct() + l.listen_middle_pct() + l.listen_shortest_pct();
        assert!((listen_sum - 100.0).abs() < 1e-9);
        assert_eq!(l.choice_longest_pct(), 100.0);
        assert_eq!(l.choice_ratio(), 7.0);
    }

    #[test]
    fn breaches_name_each_limit() {
        let mut l = Level::default();
        add(
            &mut l,
            &lesson(
                "exercises: [{type: listen, text: Satu dua, options: [نعم, لا], answer: 1}, {type: multiple_choice, options: [لا, Ya sekali], answer: 1}]",
            ),
        );
        let ok = Limit {
            max_listen_arabic_pct: Some(100.0),
            max_choice_arabic_pct: Some(100.0),
            min_listen_words: Some(2.0),
            seen_listen_longest_pct: Some(0.0),
            seen_choice_longest_pct: Some(100.0),
            ..Limit::default()
        };
        assert!(breaches("bipa5", &l, &ok).is_empty());
        assert!(breaches("bipa5", &l, &Limit::default()).is_empty(), "- means no limit");
        let tight = Limit {
            max_listen_arabic_pct: Some(30.0),
            max_choice_arabic_pct: Some(5.0),
            min_listen_words: Some(20.0),
            seen_listen_longest_pct: Some(0.0),
            seen_choice_longest_pct: Some(40.0),
            ..Limit::default()
        };
        let b = breaches("bipa5", &l, &tight);
        assert_eq!(b.len(), 3, "{b:?}");
        assert!(b[0].starts_with("bipa5: 100.0% of listen items"), "{b:?}");
        assert!(b.iter().any(|x| x.contains("multiple-choice items have the longest option")), "{b:?}");
        assert!(!b.iter().any(|x| x.contains("listen items have the longest")), "the short answer: {b:?}");
    }

    #[test]
    fn rank_and_ratio_measures_only_stay_in_their_band_or_move_toward_it() {
        let level = |listen_longest: f64, listen_middle: f64, ratio: f64| Level {
            listen_shaped: 100,
            listen_longest,
            listen_middle,
            listen_shortest: 100.0 - listen_longest - listen_middle,
            listen_ratios: vec![ratio],
            ..Level::default()
        };
        let seen = |longest: f64, middle: f64, ratio: f64| Limit {
            seen_listen_longest_pct: Some(longest),
            seen_listen_middle_pct: Some(middle),
            seen_listen_ratio: Some(ratio),
            ..Limit::default()
        };
        let breached = |l: Level, s: Limit| !breaches("bipa5", &l, &s).is_empty();
        assert_eq!((off_band(7.1), off_band(33.0), off_band(55.0)), (BAND_LOW - 7.1, 0.0, 15.0));
        assert!((off_ratio_band(0.8) - 0.1).abs() < 1e-9);
        assert_eq!(off_ratio_band(1.0), 0.0);
        assert!((off_ratio_band(1.25) - 0.15).abs() < 1e-9);
        // Recorded far below/above chance: moving toward 25-40% and 0.90-1.10 passes.
        assert!(!breached(level(7.1, 69.1, 0.99), seen(7.1, 69.1, 0.99)));
        assert!(!breached(level(20.0, 50.0, 1.00), seen(7.1, 69.1, 0.99)));
        assert!(!breached(level(33.0, 34.0, 1.00), seen(7.1, 69.1, 0.99)));
        // Moving further away fails on longest, middle, or ratio.
        assert!(breached(level(5.0, 69.1, 0.99), seen(7.1, 69.1, 0.99)));
        assert!(breached(level(20.0, 72.0, 0.99), seen(7.1, 69.1, 0.99)));
        assert!(breached(level(33.0, 34.0, 1.18), seen(33.0, 34.0, 1.00)));
        assert!(!breached(level(33.0, 34.0, 1.18), seen(33.0, 34.0, 1.25)));
        let b = breaches("bipa5", &level(5.0, 69.1, 1.18), &seen(7.1, 69.1, 1.00));
        assert!(b.iter().any(|x| x.contains("recorded 7.1%") && x.contains("25–40%")), "{b:?}");
        assert!(b.iter().any(|x| x.contains("ratio is 1.18") && x.contains("0.90–1.10")), "{b:?}");
    }

    #[test]
    fn check_reads_the_limits_file_and_fails_a_worse_level() {
        let s = crate::testkit::Sandbox::empty();
        s.write(
            "content/lessons/id/bipa5.u01.l01.yaml",
            "id: bipa5.u01.l01\nexercises:\n  - {id: e01, type: listen, text: Satu dua tiga, options: [نعم, لا يا], answer: 1}\n",
        );
        s.write("content/lessons/id/notes.yaml", "title: no id\n");
        s.write(
            "content/lessons/jv/j1.u01.l01.yaml",
            "id: j1.u01.l01\nexercises:\n  - {id: e02, type: multiple_choice, options: [Inggih, Mboten], answer: 0}\n",
        );
        s.write(
            LIMITS,
            "# 12-column format\nbipa5\t100\t0\t3\t100\t-\t0\t0\t-\t-\t1.67\t-\nj1\t-\t-\t-\t-\t50\t-\t-\t0\t50\t-\t1.00\n",
        );
        let mut out = Vec::new();
        report_to(&s.root(), Args { check: true, longest: None }, &mut out).unwrap();
        let text = crate::testkit::text(out);
        assert!(text.contains("bipa5") && text.contains("j1") && text.contains("listen ratio"), "{text}");
        s.write(LIMITS, "bipa5\t50\t0\t3\t-\t-\n");
        let mut out = Vec::new();
        assert!(report_to(&s.root(), Args { check: true, longest: None }, &mut out).is_err());
        assert!(crate::testkit::text(out).contains("limit 50.0%"));
        s.write(LIMITS, "bipa5\t100\t0\t3\t90\t-\n");
        assert!(s.cli(&["demand", "--check"]).is_err(), "the longest option wins every listen item");
        s.write(LIMITS, "bipa5\t100\t0\t3\t100\t-\t0\t0\t-\t-\t1.10\t-\n");
        assert!(s.cli(&["demand", "--check"]).is_err(), "listen ratio 1.67 exceeds recorded 1.10");
        s.write(LIMITS, "bipa5\tmany\t0\t3\t-\t-\n");
        assert!(s.cli(&["demand", "--check"]).is_err());
        s.write(LIMITS, "bipa5\t100\t0\t3\n");
        assert!(s.cli(&["demand", "--check"]).is_err(), "every line has 6 or 12 columns");
        assert!(s.cli(&["demand"]).is_ok(), "the report alone never reads the limits");
        let mut out = Vec::new();
        report_to(&s.root(), Args { check: false, longest: Some("bipa5".into()) }, &mut out).unwrap();
        let text = crate::testkit::text(out);
        assert!(text.contains("bipa5.u01.l01\te01\tlisten\t5\t3"), "{text}");
        assert!(text.contains("bipa5: answer is the single longest option in 1 "), "{text}");
        assert!(s.cli(&["demand", "--longest", "j1"]).is_ok());
    }
}
