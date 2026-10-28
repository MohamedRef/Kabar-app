//! `kabar-tools coverage`: unknown-word share of every Sinetron episode,
//! Radio script and lesson dialogue at the point the learner meets it (#138).
//! The measuring lives in `kabar_content::coverage`.
//!
//! Lesson dialogues ratchet (#371): `content/freq/dialogue-over.tsv` lists the
//! dialogues already over a limit. `--warn` flags any other dialogue that goes
//! over, and any listed one that is now under (drop its row); `--baseline`
//! rewrites the list.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use anyhow::Result;
use clap::Args as ClapArgs;
use kabar_content::coverage::{Item, report};

use crate::paths::Root;

#[derive(ClapArgs)]
pub struct Args {
    /// Print a GitHub Actions warning for every Sinetron episode and Radio
    /// script over either limit, and for every lesson dialogue whose state
    /// differs from content/freq/dialogue-over.tsv (never fails).
    #[arg(long)]
    pub warn: bool,
    /// List every item, not only those over a limit.
    #[arg(long)]
    pub all: bool,
    /// List every unknown word of each item shown, not only the six most frequent.
    #[arg(long)]
    pub words: bool,
    /// Rewrite content/freq/dialogue-over.tsv from the dialogues over a limit now.
    #[arg(long)]
    pub baseline: bool,
}

const BASELINE: &str = "content/freq/dialogue-over.tsv";

fn warning(i: &Item, extra: &str) -> String {
    format!(
        "::warning title=Reading coverage::{} has {:.1}% words not taught by {} (limit {}%), {:.1}% before glosses (limit {}%): {}{extra}",
        i.id,
        i.percent(),
        i.after,
        i.limit(),
        i.raw_percent(),
        i.raw_limit(),
        i.top(6).into_iter().map(|(w, _)| w).collect::<Vec<_>>().join(", ")
    )
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn line(i: &Item, words: usize) -> String {
    // An item over only the raw cap is fixed by teaching or rewording, so
    // show the words its notes gloss as well.
    let top = if i.raw_over() && !i.over() { i.top_raw(words) } else { i.top(words) };
    let top: Vec<String> = top.into_iter().map(|(w, c)| if c > 1 { format!("{w}×{c}") } else { w }).collect();
    format!(
        "{:<20} {:<9} {:<14} {:>5.1}% / {:>2}%  raw {:>5.1}% / {:>2}%  {}",
        i.id,
        i.kind,
        i.after,
        i.percent(),
        i.limit(),
        i.raw_percent(),
        i.raw_limit(),
        top.join(", ")
    )
}

pub fn run(root: &Root, args: Args) -> Result<()> {
    let items = report(root.path())?;
    println!("glossed limit 8% (levels 1-2) / 12% later; raw (notes ignored) limit 15% / 18%");
    println!("{:<9} {:>6} {:>8} {:>6} {:>8} {:>6}", "kind", "items", "median", "over", "raw med", "over");
    for kind in ["sinetron", "radio", "dialogue"] {
        let of: Vec<&Item> = items.iter().filter(|i| i.kind == kind).collect();
        let over = of.iter().filter(|i| i.over()).count();
        let raw_over = of.iter().filter(|i| i.raw_over()).count();
        println!(
            "{kind:<9} {:>6} {:>7.1}% {over:>6} {:>7.1}% {raw_over:>6}",
            of.len(),
            median(of.iter().map(|i| i.percent()).collect()),
            median(of.iter().map(|i| i.raw_percent()).collect())
        );
    }
    println!();
    let mut shown: Vec<&Item> = items.iter().filter(|i| args.all || i.over() || i.raw_over()).collect();
    shown.sort_by(|a, b| b.percent().total_cmp(&a.percent()));
    for i in &shown {
        println!("{}", line(i, if args.words { usize::MAX } else { 6 }));
    }
    let over_dialogues: Vec<&Item> =
        items.iter().filter(|i| i.kind == "dialogue" && (i.over() || i.raw_over())).collect();
    let path = root.path().join(BASELINE);
    if args.baseline {
        let mut out = String::from(
            "# Lesson dialogues over the reading-coverage limit (#371). id<TAB>glossed%<TAB>raw%.\n\
             # Teach or reword to drop a row; `kabar-tools coverage --baseline` rewrites this list.\n",
        );
        for i in &over_dialogues {
            writeln!(out, "{}\t{:.1}\t{:.1}", i.id, i.percent(), i.raw_percent())?;
        }
        std::fs::write(&path, out)?;
        println!("wrote {} rows to {BASELINE}", over_dialogues.len());
    }
    if args.warn {
        for i in items.iter().filter(|i| (i.over() || i.raw_over()) && i.kind != "dialogue") {
            println!("{}", warning(i, ""));
        }
        let known: BTreeSet<String> = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split('\t').next())
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .collect();
        for i in over_dialogues.iter().filter(|i| !known.contains(&i.id)) {
            println!("{}", warning(i, &format!(" (new: not in {BASELINE})")));
        }
        let now: BTreeSet<&str> = over_dialogues.iter().map(|i| i.id.as_str()).collect();
        for id in known.iter().filter(|id| !now.contains(id.as_str())) {
            println!(
                "::warning title=Reading coverage::{id} is now within the limit; drop its row from {BASELINE}"
            );
        }
        println!(
            "{} lesson dialogues over a limit ({} listed in {BASELINE})",
            over_dialogues.len(),
            known.len()
        );
    }
    Ok(())
}
