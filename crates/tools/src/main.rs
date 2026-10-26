//! `kabar-tools`: the build-time content pipeline (`docs/SPEC.md` §18).
//!
//! Every command is idempotent and resumable: outputs are derived from inputs
//! in `data/raw/` (downloaded by `scripts/fetch-sources.sh`) and `content/`,
//! and generation work is split into numbered batches that can be done in any
//! order, across sessions.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod bridge;
mod freq;
mod gloss;
mod golden;
mod kaikki;
mod levels;
mod packs;
mod paths;
mod schemas;
mod text;
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
    /// Golden sets for the runtime AI tasks.
    #[command(subcommand)]
    Golden(golden::Cmd),
    /// Offline packs per unit: manifests, Azure audio within the free quota, release tars.
    #[command(subcommand)]
    Packs(packs::Cmd),
    /// Regenerate `schemas/*.schema.json` from the Rust content types.
    Schemas {
        /// Fail instead of writing when a schema is out of date.
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
        Command::Golden(cmd) => golden::run(&root, cmd),
        Command::Packs(cmd) => packs::run(&root, cmd),
        Command::Schemas { check } => schemas::run(&root, check),
        Command::Validate => {
            schemas::run(&root, true)?;
            gloss::run(&root, gloss::Cmd::Validate { batch: None })?;
            bridge::run(&root, bridge::Cmd::Validate)?;
            let mut issues = gloss::crosscheck_bridge(&root, kabar_protocol::content::Lang::Id)?;
            issues.extend(gloss::crosscheck_bridge(&root, kabar_protocol::content::Lang::Jv)?);
            for i in &issues {
                println!("gloss↔bridge: {i}");
            }
            anyhow::ensure!(issues.is_empty(), "{} gloss↔bridge issue(s)", issues.len());
            levels::run(&root, levels::Cmd::Validate)?;
            golden::run(&root, golden::Cmd::Validate)?;
            packs::run(&root, packs::Cmd::Build { check: true })?;
            println!("all content ok");
            Ok(())
        }
    }
}
