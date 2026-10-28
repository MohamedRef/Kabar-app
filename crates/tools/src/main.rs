//! `kabar-tools`: the build-time content pipeline (`docs/SPEC.md` §18).
//!
//! Every command is idempotent and resumable: outputs are derived from inputs
//! in `data/raw/` (downloaded by `scripts/fetch-sources.sh`) and `content/`,
//! and generation work is split into numbered batches that can be done in any
//! order, across sessions.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod bakeoff;
mod bridge;
mod changes;
mod coverage;
mod dialect;
mod explains;
mod freq;
mod gloss;
mod golden;
mod hashscan;
mod kaikki;
mod levels;
mod packs;
mod paths;
mod pron;
mod readme;
mod schemas;
mod text;
mod update;
mod wiki;

#[derive(Parser)]
#[command(name = "kabar-tools", about = "Kabar content pipeline")]
struct Cli {
    /// Repository root (defaults to the workspace this binary was built from).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Frequency lists.
    #[command(subcommand)]
    Freq(freq::Cmd),
    /// Import kaikki.org Wiktionary JSONL into normalized dictionary entries.
    Kaikki(kaikki::Args),
    /// Prebuilt glosses for the most frequent words.
    #[command(subcommand)]
    Gloss(gloss::Cmd),
    /// The Arabic Bridge dataset.
    #[command(subcommand)]
    Bridge(bridge::Cmd),
    /// The Javanese speech-level table.
    #[command(subcommand)]
    Levels(levels::Cmd),
    /// Arabic-script pronunciation data (content/pron).
    #[command(subcommand)]
    Pron(pron::Cmd),
    /// Golden sets for the runtime AI tasks.
    #[command(subcommand)]
    Golden(golden::Cmd),
    /// The engine bake-off: run the golden sets, blind-judge, report routes.
    #[command(subcommand)]
    Bakeoff(bakeoff::Cmd),
    /// Offline packs per unit: manifests, Azure audio within the free quota, release tars.
    #[command(subcommand)]
    Packs(packs::Cmd),
    /// App update feeds: the signing key, `update.json` and its signature.
    #[command(subcommand)]
    Update(update::Cmd),
    /// Unknown-word share of Sinetron, Radio and lesson dialogues against the curriculum.
    Coverage(coverage::Args),
    /// Share of closed exercises with an `explain`, per level (#140).
    Explains(explains::Args),
    /// Surabaya and Semarang forms left in the Javanese track (Jogja + Madiun only).
    Dialect(dialect::Args),
    /// Regenerate `schemas/*.schema.json` from the Rust content types.
    Schemas {
        /// Fail instead of writing when a schema is out of date.
        #[arg(long)]
        check: bool,
    },
    /// Refresh the generated numbers in `README.md` from the content.
    Readme {
        /// Fail instead of writing when the README is out of date.
        #[arg(long)]
        check: bool,
    },
    /// Validate all content (schemas, glosses, bridge). Run before committing content.
    Validate,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = paths::Root::new(cli.root);
    match cli.command {
        Command::Freq(cmd) => freq::run(&root, cmd),
        Command::Kaikki(args) => kaikki::run(&root, args),
        Command::Gloss(cmd) => gloss::run(&root, cmd),
        Command::Bridge(cmd) => bridge::run(&root, cmd),
        Command::Levels(cmd) => levels::run(&root, cmd),
        Command::Pron(cmd) => pron::run(&root, cmd),
        Command::Golden(cmd) => golden::run(&root, cmd),
        Command::Bakeoff(cmd) => bakeoff::run(&root, cmd),
        Command::Packs(cmd) => packs::run(&root, cmd),
        Command::Update(cmd) => update::run(&root, cmd),
        Command::Coverage(args) => coverage::run(&root, args),
        Command::Explains(args) => explains::run(&root, args),
        Command::Dialect(args) => dialect::run(&root, args),
        Command::Schemas { check } => schemas::run(&root, check),
        Command::Readme { check } => readme::run(&root, check),
        Command::Validate => {
            schemas::run(&root, true)?;
            gloss::run(&root, gloss::Cmd::Validate { batch: None, lang: None })?;
            bridge::run(&root, bridge::Cmd::Validate)?;
            let mut issues = gloss::crosscheck_bridge(&root, kabar_protocol::content::Lang::Id)?;
            issues.extend(gloss::crosscheck_bridge(&root, kabar_protocol::content::Lang::Jv)?);
            for i in &issues {
                println!("gloss↔bridge: {i}");
            }
            anyhow::ensure!(issues.is_empty(), "{} gloss↔bridge issue(s)", issues.len());
            let issues = bridge::check_lessons(&root)?;
            for i in &issues {
                println!("lesson↔bridge: {i}");
            }
            anyhow::ensure!(issues.is_empty(), "{} lesson↔bridge issue(s)", issues.len());
            let issues = hashscan::check(&root)?;
            for i in &issues {
                println!("yaml: {i}");
            }
            anyhow::ensure!(issues.is_empty(), "{} YAML value(s) cut by a \" #\" comment", issues.len());
            levels::run(&root, levels::Cmd::Validate)?;
            golden::run(&root, golden::Cmd::Validate)?;
            packs::run(&root, packs::Cmd::Build { check: true })?;
            println!("all content ok");
            Ok(())
        }
    }
}
