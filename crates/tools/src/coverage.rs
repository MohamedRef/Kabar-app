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
    report_to(root, args, &mut std::io::stdout().lock())
}

pub fn report_to(root: &Root, args: Args, out: &mut dyn std::io::Write) -> Result<()> {
    let items = report(root.path())?;
    writeln!(out, "glossed limit 8% (levels 1-2) / 12% later; raw (notes ignored) limit 15% / 18%")?;
    writeln!(
        out,
        "{:<9} {:>6} {:>8} {:>6} {:>8} {:>6}",
        "kind", "items", "median", "over", "raw med", "over"
    )?;
    for kind in ["sinetron", "radio", "dialogue"] {
        let of: Vec<&Item> = items.iter().filter(|i| i.kind == kind).collect();
        let over = of.iter().filter(|i| i.over()).count();
        let raw_over = of.iter().filter(|i| i.raw_over()).count();
        writeln!(
            out,
            "{kind:<9} {:>6} {:>7.1}% {over:>6} {:>7.1}% {raw_over:>6}",
            of.len(),
            median(of.iter().map(|i| i.percent()).collect()),
            median(of.iter().map(|i| i.raw_percent()).collect())
        )?;
    }
    writeln!(out)?;
    let mut shown: Vec<&Item> = items.iter().filter(|i| args.all || i.over() || i.raw_over()).collect();
    shown.sort_by(|a, b| b.percent().total_cmp(&a.percent()));
    for i in &shown {
        writeln!(out, "{}", line(i, if args.words { usize::MAX } else { 6 }))?;
    }
    let over_dialogues: Vec<&Item> =
        items.iter().filter(|i| i.kind == "dialogue" && (i.over() || i.raw_over())).collect();
    let path = root.path().join(BASELINE);
    if args.baseline {
        let mut tsv = String::from(
            "# Lesson dialogues over the reading-coverage limit (#371). id<TAB>glossed%<TAB>raw%.\n\
             # Teach or reword to drop a row; `kabar-tools coverage --baseline` rewrites this list.\n",
        );
        for i in &over_dialogues {
            writeln!(tsv, "{}\t{:.1}\t{:.1}", i.id, i.percent(), i.raw_percent())?;
        }
        std::fs::write(&path, tsv)?;
        writeln!(out, "wrote {} rows to {BASELINE}", over_dialogues.len())?;
    }
    if args.warn {
        for i in items.iter().filter(|i| (i.over() || i.raw_over()) && i.kind != "dialogue") {
            writeln!(out, "{}", warning(i, ""))?;
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
            writeln!(out, "{}", warning(i, &format!(" (new: not in {BASELINE})")))?;
        }
        let now: BTreeSet<&str> = over_dialogues.iter().map(|i| i.id.as_str()).collect();
        for id in known.iter().filter(|id| !now.contains(id.as_str())) {
            writeln!(
                out,
                "::warning title=Reading coverage::{id} is now within the limit; drop its row from {BASELINE}"
            )?;
        }
        writeln!(
            out,
            "{} lesson dialogues over a limit ({} listed in {BASELINE})",
            over_dialogues.len(),
            known.len()
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{Sandbox, text};

    fn content() -> Sandbox {
        // The first units of each track: lessons the curriculum lists but
        // the tree lacks are skipped, and so are episodes after them.
        let s = Sandbox::with(&["curriculum", "content/freq", "content/cast.yaml"]);
        s.copy_matching("content/lessons/id", &["bipa1.u01", "bipa1.u02", "bipa1.u03"]);
        s.copy_matching("content/lessons/jv", &["j1.u01"]);
        s.copy_matching("content/radio/id", &["radio.bipa1.u01", "radio.bipa1.u02", "radio.bipa1.u03"]);
        s.copy_matching("content/stories/sinetron", &["sinetron.e0"]);
        s
    }

    fn report(s: &Sandbox, args: Args) -> String {
        let mut out = Vec::new();
        report_to(&s.root(), args, &mut out).unwrap();
        text(out)
    }

    fn args() -> Args {
        Args { warn: false, all: false, words: false, baseline: false }
    }

    #[test]
    fn the_baseline_ratchet_names_new_and_fixed_dialogues() {
        let s = content();
        s.write(BASELINE, "# header\nbipa9.u01.l01\t40.0\t40.0\n\n");
        let warned = report(&s, Args { warn: true, ..args() });
        assert!(
            warned.contains(
                "::warning title=Reading coverage::bipa9.u01.l01 is now within the limit; drop its row from content/freq/dialogue-over.tsv"
            ),
            "{warned}"
        );

        let wrote = report(&s, Args { baseline: true, ..args() });
        let rows: Vec<String> =
            s.read(BASELINE).lines().filter(|l| !l.starts_with('#')).map(str::to_owned).collect();
        assert!(wrote.contains(&format!("wrote {} rows to {BASELINE}", rows.len())), "{wrote}");
        for row in &rows {
            let cols: Vec<&str> = row.split('\t').collect();
            assert_eq!(cols.len(), 3, "{row}");
            assert!(cols[1].parse::<f64>().is_ok() && cols[2].parse::<f64>().is_ok(), "{row}");
        }
        // Right after a baseline, every over dialogue is listed and none is stale.
        let warned = report(&s, Args { warn: true, ..args() });
        assert!(!warned.contains("(new: not in") && !warned.contains("now within"), "{warned}");
        assert!(
            warned.contains(&format!(
                "{} lesson dialogues over a limit ({} listed in",
                rows.len(),
                rows.len()
            )),
            "{warned}"
        );
        // A dialogue missing from the list is new.
        if let Some(first) = rows.first() {
            let id = first.split('\t').next().unwrap();
            s.write(BASELINE, "");
            let warned = report(&s, Args { warn: true, ..args() });
            assert!(warned.contains(&format!("{id} has")) && warned.contains("(new: not in"), "{warned}");
        }
    }

    #[test]
    fn lists_every_item_with_all_and_every_word_with_words() {
        let s = content();
        let summary = report(&s, args());
        assert!(summary.starts_with("glossed limit 8% (levels 1-2) / 12% later"), "{summary}");
        for kind in ["sinetron", "radio", "dialogue"] {
            assert!(summary.lines().any(|l| l.starts_with(kind)), "{kind} row in\n{summary}");
        }
        let all = report(&s, Args { all: true, words: true, ..args() });
        let items = all.lines().filter(|l| l.contains("% / ") && l.contains("raw")).count();
        assert!(items > 20, "every Sinetron, Radio and dialogue item: {items}");
        assert!(all.lines().count() > summary.lines().count());
        s.cli(&["coverage"]).unwrap();
    }

    #[test]
    fn median_and_lines_read_well() {
        assert_eq!(median(vec![]), 0.0);
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        let item = Item {
            id: "e01".into(),
            kind: "sinetron",
            after: "bipa1.u02".into(),
            level: 0,
            tokens: 100,
            unknown: vec!["pasar".into(), "pasar".into(), "ramai".into()],
            raw: vec!["pasar".into(), "pasar".into(), "ramai".into()],
        };
        let l = line(&item, 6);
        assert!(l.starts_with("e01") && l.contains("pasar×2, ramai"), "{l}");
        let w = warning(&item, " (x)");
        assert!(
            w.starts_with("::warning title=Reading coverage::e01 has 3.0% words not taught by bipa1.u02"),
            "{w}"
        );
        assert!(w.ends_with("pasar, ramai (x)"), "{w}");
    }
}
