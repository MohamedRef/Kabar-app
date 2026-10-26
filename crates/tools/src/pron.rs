//! The pronunciation data behind `kabar_core::pron` (Arabic-script
//! pronunciation): `content/pron/id.tsv`, cut from the g2p_id lexicon.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use kabar_core::lang::Lang;
use kabar_core::pron::{self, Ph, PronDict};

use crate::paths::Root;

#[derive(Subcommand)]
pub enum Cmd {
    /// Rebuild `content/pron/id.tsv` from g2p_id's `lexicon_id.tsv`
    /// (github.com/bookbot-kids/g2p_id, g2p_id/resources/lexicon_id.tsv, Apache-2.0).
    Import {
        /// Path to the downloaded `lexicon_id.tsv`.
        lexicon: PathBuf,
    },
    /// Print words in Arabic script, to eyeball the rules.
    Show {
        /// `id` or `jv`.
        #[arg(long, default_value = "id")]
        lang: String,
        words: Vec<String>,
    },
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Import { lexicon } => import(root, &lexicon),
        Cmd::Show { lang, words } => {
            let lang = match lang.as_str() {
                "id" => Lang::Indonesian,
                "jv" => Lang::Javanese,
                _ => bail!("--lang must be id or jv"),
            };
            let dict = PronDict::builtin();
            for w in words {
                println!("{w}\t{}\t{}", pron::codes(&dict.phones(&w, lang)), dict.text(&w, lang));
            }
            Ok(())
        }
    }
}

/// Keeps an entry when the rules get it wrong, or when it has an `e` (an
/// affixed form missing from the lexicon borrows its root's é/ə).
fn import(root: &Root, lexicon: &PathBuf) -> Result<()> {
    let text = fs::read_to_string(lexicon).with_context(|| format!("reading {}", lexicon.display()))?;
    let mut out = String::from(
        "# Indonesian pronunciations: word<TAB>phone codes (kabar_core::pron::Ph::code).\n\
         # Cut from g2p_id's lexicon_id.tsv (Apache-2.0, see ATTRIBUTION.md) by\n\
         # `kabar-tools pron import`: only words the rules get wrong or that contain an e.\n",
    );
    let (mut kept, mut skipped, mut total) = (0, 0, 0);
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let Some((word, ipa)) = line.split_once('\t') else { continue };
        total += 1;
        // A few words are listed twice (homographs); the first reading wins.
        if !seen.insert(word.to_string()) {
            skipped += 1;
            continue;
        }
        if !word.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c == '\'') {
            skipped += 1;
            continue;
        }
        let phones: Option<Vec<Ph>> = ipa.split(' ').map(Ph::from_ipa).collect();
        let Some(phones) = phones else {
            skipped += 1;
            continue;
        };
        if word.contains('e') || pron::rule_phones(word, Lang::Indonesian) != phones {
            writeln!(out, "{word}\t{}", pron::codes(&phones))?;
            kept += 1;
        }
    }
    let path = root.path().join("content/pron/id.tsv");
    fs::write(&path, out)?;
    println!("{} of {total} words kept ({skipped} unreadable or repeated) → {}", kept, path.display());
    Ok(())
}
