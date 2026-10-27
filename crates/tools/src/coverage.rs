//! `kabar-tools coverage`: unknown-word share of every Sinetron episode,
//! Radio script and lesson dialogue at the point the learner meets it (#138).
//! The measuring lives in `kabar_content::coverage`.

use anyhow::Result;
use clap::Args as ClapArgs;
use kabar_content::coverage::{Item, report};

use crate::paths::Root;

#[derive(ClapArgs)]
pub struct Args {
    /// Print a GitHub Actions warning for every Sinetron episode and Radio
    /// script over either limit (never fails).
    #[arg(long)]
    pub warn: bool,
    /// List every item, not only those over a limit.
    #[arg(long)]
    pub all: bool,
    /// List every unknown word of each item shown, not only the six most frequent.
    #[arg(long)]
    pub words: bool,
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn line(i: &Item, words: usize) -> String {
    let top: Vec<String> =
        i.top(words).into_iter().map(|(w, c)| if c > 1 { format!("{w}×{c}") } else { w }).collect();
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
    if args.warn {
        for i in items.iter().filter(|i| (i.over() || i.raw_over()) && i.kind != "dialogue") {
            println!(
                "::warning title=Reading coverage::{} has {:.1}% words not taught by {} (limit {}%), {:.1}% before glosses (limit {}%): {}",
                i.id,
                i.percent(),
                i.after,
                i.limit(),
                i.raw_percent(),
                i.raw_limit(),
                i.top(6).into_iter().map(|(w, _)| w).collect::<Vec<_>>().join(", ")
            );
        }
    }
    Ok(())
}
