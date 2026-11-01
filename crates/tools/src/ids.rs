//! `kabar-tools ids`: the content ids learner records point at
//! (`kabar_core::content_ids`, `docs/CONTENT-IDS.md`).
//!
//! - `ids check --base <git ref>` compares the working tree's content with
//!   the content at that commit (CI: the PR's base, `HEAD^1`), and fails on
//!   a removed or repurposed id that `content/ids-retired.txt` doesn't name.
//! - `ids check --base-bundle <file>` compares with a published content
//!   bundle instead (what devices actually have).
//! - `ids list` prints every id (for authors: which ids exist).

use std::collections::BTreeSet;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context as _, Result, bail, ensure};
use clap::Subcommand;
use kabar_core::content_ids::{Registry, compare, parse_retired, registry};
use kabar_protocol::bundle::{SOURCES, Source, source_files};

use crate::paths::Root;

pub const RETIRED: &str = "content/ids-retired.txt";

#[derive(Subcommand)]
pub enum Cmd {
    /// Fail when an id learner records point at was removed or repurposed.
    Check {
        /// The git commit to compare with (e.g. `origin/main`, `HEAD^1`).
        #[arg(long, conflicts_with = "base_bundle")]
        base: Option<String>,
        /// A published content bundle to compare with.
        #[arg(long)]
        base_bundle: Option<PathBuf>,
    },
    /// Print every content id and what it hangs under.
    List,
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            for (id, parent) in registry(&working_files(root.path())?) {
                println!("{id}\t{parent}");
            }
            Ok(())
        }
        Cmd::Check { base, base_bundle } => {
            let base = match (base, base_bundle) {
                (Some(r), None) => registry(&git_files(root.path(), &r)?),
                (None, Some(b)) => registry(&bundle_files(&b)?),
                _ => bail!("give --base <git ref> or --base-bundle <file>"),
            };
            let head = registry(&working_files(root.path())?);
            check(root.path(), &base, &head)
        }
    }
}

/// Compares and prints the report; Err when it is breaking.
pub fn check(root: &Path, base: &Registry, head: &Registry) -> Result<()> {
    let retired = match std::fs::read_to_string(root.join(RETIRED)) {
        Ok(t) => parse_retired(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeSet::new(),
        Err(e) => return Err(e).context(RETIRED),
    };
    let report = compare(base, head, &retired);
    for line in report.lines() {
        println!("ids: {line}");
    }
    ensure!(
        !report.breaking(),
        "content ids learner records point at were removed or repurposed. Keep the id (fix the text \
         instead), or, after a deliberate migration decision, list it in {RETIRED}"
    );
    println!("ids: {} checked, no breaking change", head.len());
    Ok(())
}

/// The bundle files of the working tree, unpacked.
pub fn working_files(root: &Path) -> Result<Vec<(String, String)>> {
    let mut files = Vec::new();
    for s in SOURCES {
        for (key, path) in source_files(root, *s) {
            match std::fs::read_to_string(&path) {
                Ok(text) => files.push((key, text)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).with_context(|| path.display().to_string()),
            }
        }
    }
    Ok(files)
}

/// A published bundle's files, unpacked.
pub fn bundle_files(path: &Path) -> Result<Vec<(String, String)>> {
    let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
    let entries =
        kabar_protocol::bundle::decode(&bytes).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    entries
        .into_iter()
        .map(|(key, packed)| {
            let raw = miniz_oxide::inflate::decompress_to_vec_with_limit(
                &packed,
                kabar_protocol::bundle::MAX_FILE_UNPACKED,
            )
            .map_err(|_| anyhow::anyhow!("{key} doesn't unpack"))?;
            let text = String::from_utf8(raw).map_err(|_| anyhow::anyhow!("{key} isn't UTF-8"))?;
            Ok((key, text))
        })
        .collect()
}

/// The bundle files as they were at `rev`, read straight from git (no
/// checkout): the same keys `source_files` gives a working tree.
pub fn git_files(root: &Path, rev: &str) -> Result<Vec<(String, String)>> {
    let git = |args: &[&str]| -> Result<String> {
        let out = Command::new("git").arg("-C").arg(root).args(args).output().context("running git")?;
        ensure!(
            out.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
        Ok(String::from_utf8(out.stdout)?)
    };
    let stem = |p: &str| Path::new(p).file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_owned();
    let mut wanted: Vec<(String, String)> = Vec::new();
    for s in SOURCES {
        match *s {
            Source::Single { name, path } => wanted.push((name.to_owned(), path.to_owned())),
            Source::Table { name, dir } => {
                let mut paths: Vec<String> = git(&["ls-tree", "-r", "--name-only", rev, "--", dir])?
                    .lines()
                    .filter(|p| p.ends_with(".yaml"))
                    .map(str::to_owned)
                    .collect();
                paths.sort();
                wanted.extend(paths.into_iter().map(|p| (format!("{name}/{}", stem(&p)), p)));
            }
            Source::List { name, dir } => {
                let prefix = format!("{dir}/");
                let mut paths: Vec<String> = git(&["ls-tree", "--name-only", rev, "--", &prefix])?
                    .lines()
                    .filter(|p| p.ends_with(".jsonl"))
                    .map(str::to_owned)
                    .collect();
                paths.sort();
                wanted.extend(paths.into_iter().map(|p| (format!("{name}/{}", stem(&p)), p)));
            }
        }
    }
    // One `git cat-file --batch` for every file.
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("running git cat-file")?;
    let mut stdin = child.stdin.take().context("git stdin")?;
    let request: String = wanted.iter().map(|(_, p)| format!("{rev}:{p}\n")).collect();
    let writer = std::thread::spawn(move || stdin.write_all(request.as_bytes()));
    let mut out = BufReader::new(child.stdout.take().context("git stdout")?);
    let mut files = Vec::with_capacity(wanted.len());
    for (key, path) in wanted {
        let mut header = String::new();
        out.read_line(&mut header)?;
        if header.trim_end().ends_with(" missing") {
            continue; // not in that commit (a single file added since)
        }
        let size: usize = header
            .split_whitespace()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .with_context(|| format!("git cat-file {path}: {header:?}"))?;
        let mut body = vec![0; size + 1]; // and the newline after it
        out.read_exact(&mut body)?;
        body.pop();
        files.push((key, String::from_utf8(body).with_context(|| format!("{rev}:{path} isn't UTF-8"))?));
    }
    writer.join().map_err(|_| anyhow::anyhow!("git writer panicked"))??;
    child.wait()?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_and_working_tree_read_the_same_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // HEAD's committed files against the working tree: same keys.
        let Ok(at_head) = git_files(&root, "HEAD") else { return }; // not a git checkout
        let here = working_files(&root).unwrap();
        let keys = |f: &[(String, String)]| f.iter().map(|(k, _)| k.clone()).collect::<BTreeSet<_>>();
        let (a, b) = (keys(&at_head), keys(&here));
        assert!(
            a.len() > 100 && a.difference(&b).count() < 5,
            "{:?}",
            a.symmetric_difference(&b).collect::<Vec<_>>()
        );
        assert!(check(&root, &registry(&at_head), &registry(&at_head)).is_ok());
    }

    #[test]
    fn a_removed_id_fails_until_it_is_retired() {
        let dir = tempfile::tempdir().unwrap();
        let mut base = Registry::new();
        base.insert("lesson x.u01.l01".into(), "unit x.u01".into());
        let head = Registry::new();
        let err = check(dir.path(), &base, &head).unwrap_err();
        assert!(err.to_string().contains(RETIRED), "{err}");
        std::fs::create_dir_all(dir.path().join("content")).unwrap();
        std::fs::write(dir.path().join(RETIRED), "# why: merged\nlesson x.u01.l01\n").unwrap();
        assert!(check(dir.path(), &base, &head).is_ok());
    }
}
