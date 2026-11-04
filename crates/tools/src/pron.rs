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
            print!("{}", show(&lang, &words)?);
            Ok(())
        }
    }
}

/// `word<TAB>phone codes<TAB>Arabic script` per word.
fn show(lang: &str, words: &[String]) -> Result<String> {
    let lang = match lang {
        "id" => Lang::Indonesian,
        "jv" => Lang::Javanese,
        _ => bail!("--lang must be id or jv"),
    };
    let dict = PronDict::builtin();
    Ok(words
        .iter()
        .map(|w| format!("{w}\t{}\t{}\n", pron::codes(&dict.phones(w, lang)), dict.text(w, lang)))
        .collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_keeps_words_with_e_or_that_the_rules_get_wrong() {
        let s = crate::testkit::Sandbox::empty();
        let lexicon = s.write(
            "lexicon_id.tsv",
            "besar\tb ə s a r\nbesar\tb e s a r\nJakarta\tdʒ a k a r t a\nmakan\tm a k a n\naneh\ta n e !\nno tab here\nsatu\ts a t u\n",
        );
        s.write("content/pron/id.tsv", "old");
        s.cli(&["pron", "import", lexicon.to_str().unwrap()]).unwrap();
        let text = s.read("content/pron/id.tsv");
        let rows: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        assert!(rows.contains(&"besar\tbəsar"), "{rows:?}: the first reading of a homograph wins");
        assert!(
            !rows.iter().any(|r| r.starts_with("Jakarta") || r.starts_with("aneh")),
            "unreadable rows go"
        );
        assert!(
            !rows.contains(&"makan\tmakan") && !rows.contains(&"satu\tsatu"),
            "the rules already say these"
        );
        assert!(text.starts_with("# Indonesian pronunciations"));
        assert!(s.cli(&["pron", "import", "missing.tsv"]).is_err());
    }

    #[test]
    fn show_prints_codes_and_arabic_script() {
        let out = show("id", &["besar".into(), "makan".into()]).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("besar\tbəsar\t"), "{out}");
        assert!(
            lines[1]
                .split('\t')
                .nth(2)
                .is_some_and(|a| a.chars().any(|c| ('\u{600}'..='\u{6ff}').contains(&c))),
            "{out}"
        );
        assert!(show("jv", &["mangan".into()]).unwrap().starts_with("mangan\t"));
        assert!(show("ms", &[]).is_err());
        crate::testkit::Sandbox::empty().cli(&["pron", "show", "kabar"]).unwrap();
    }
}
