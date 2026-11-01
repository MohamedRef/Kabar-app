//! `kabar-tools content`: the authoring workflow (`docs/AUTHORING.md`).
//!
//! - `content new-lesson <id>` writes `content/lessons/<lang>/<id>.yaml`
//!   from `content/templates/lesson.yaml`. Every `TODO(author)` in it must
//!   be replaced: the bundle check refuses a lesson that still has one.
//! - `content diff --base REF` lists which content files a change adds,
//!   edits or removes compared with a commit (or `--base-bundle FILE`, a
//!   published bundle), as devices will see them.
//! - `content preflight --base REF` runs every check a content change has
//!   to pass, in order, and prints one table: the content checks
//!   (`validate`), the schema and relationship checks (`kabar-content`), the
//!   bundle checks devices run, the content-id check, and the diff.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail, ensure};
use clap::Subcommand;

use crate::ids;
use crate::paths::Root;

pub const TEMPLATE: &str = "content/templates/lesson.yaml";

#[derive(Subcommand)]
pub enum Cmd {
    /// Start a lesson from the template: `content new-lesson bipa1.u01.l09`.
    NewLesson {
        /// The new lesson's id (`<level>.uNN.lNN`); its level says the language.
        id: String,
    },
    /// Which content files differ from a commit or a published bundle.
    Diff {
        #[arg(long, conflicts_with = "base_bundle")]
        base: Option<String>,
        #[arg(long)]
        base_bundle: Option<PathBuf>,
        /// Compare this bundle (from `update bundle`, stylesheets included)
        /// instead of the working tree's content files.
        #[arg(long)]
        head_bundle: Option<PathBuf>,
    },
    /// Every check a content change must pass, in one run.
    Preflight {
        /// The commit to compare ids and files with (default `origin/main`).
        #[arg(long, default_value = "origin/main")]
        base: String,
    },
}

pub fn run(root: &Root, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::NewLesson { id } => {
            let path = new_lesson(root.path(), &id)?;
            println!("wrote {}", path.strip_prefix(root.path()).unwrap_or(&path).display());
            println!(
                "next: 1. add `- {{ id: {id}, title: …, minutes: … }}` under its unit in the curriculum"
            );
            println!("      2. replace every TODO(author)");
            println!(
                "      3. `kabar-tools packs build` (offline pack manifests), then `kabar-tools content preflight`"
            );
            Ok(())
        }
        Cmd::Diff { base, base_bundle, head_bundle } => {
            let base = match (base, base_bundle) {
                (Some(r), None) => ids::git_files(root.path(), &r)?,
                (None, Some(b)) => ids::bundle_files(&b)?,
                _ => bail!("give --base <git ref> or --base-bundle <file>"),
            };
            let head = match head_bundle {
                Some(b) => ids::bundle_files(&b)?,
                None => ids::working_files(root.path())?,
            };
            for line in diff(&base, &head).lines() {
                println!("{line}");
            }
            Ok(())
        }
        Cmd::Preflight { base } => preflight(root, &base),
    }
}

/// The language of a lesson id: Javanese levels are `j1`…, Indonesian `bipa1`….
fn language_of(id: &str) -> Option<&'static str> {
    let level = id.split('.').next()?;
    if level.starts_with("bipa") {
        Some("id")
    } else if level.strip_prefix('j').is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        Some("jv")
    } else {
        None
    }
}

pub fn new_lesson(root: &Path, id: &str) -> Result<PathBuf> {
    let parts: Vec<&str> = id.split('.').collect();
    ensure!(
        parts.len() == 3
            && parts[1].starts_with('u')
            && parts[2].starts_with('l')
            && parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())),
        "a lesson id looks like bipa1.u01.l09 or j2.u03.l04, not {id:?}"
    );
    let lang = language_of(id).with_context(|| format!("{id}: the level must be bipaN or jN"))?;
    let known = kabar_core::content_ids::registry(&ids::working_files(root)?);
    let retired = std::fs::read_to_string(root.join(ids::RETIRED)).unwrap_or_default();
    // The curriculum may already list it (planned, not written yet); a
    // lesson with exercises is written.
    ensure!(
        !known.keys().any(|k| k.starts_with(&format!("exercise {id}#"))),
        "{id} already has a lesson file"
    );
    ensure!(
        !kabar_core::content_ids::parse_retired(&retired).contains(&format!("lesson {id}")),
        "{id} was retired; retired ids are never reused"
    );
    let path = root.join(format!("content/lessons/{lang}/{id}.yaml"));
    ensure!(!path.exists(), "{} already exists", path.display());
    let template = std::fs::read_to_string(root.join(TEMPLATE)).context(TEMPLATE)?;
    let body: String = template
        .lines()
        .skip_while(|l| l.starts_with('#'))
        .map(|l| format!("{l}\n"))
        .collect::<String>()
        .replace("{{id}}", id)
        .replace("{{language}}", lang);
    std::fs::write(&path, body).with_context(|| path.display().to_string())?;
    Ok(path)
}

/// Files added, changed and removed, grouped by kind (`LESSONS`, `CAST`, …).
pub fn diff(base: &[(String, String)], head: &[(String, String)]) -> String {
    let base: BTreeMap<&str, &str> = base.iter().map(|(k, t)| (k.as_str(), t.as_str())).collect();
    let head: BTreeMap<&str, &str> = head.iter().map(|(k, t)| (k.as_str(), t.as_str())).collect();
    let kind = |k: &str| k.split('/').next().unwrap_or(k).to_owned();
    let mut groups: BTreeMap<String, [Vec<&str>; 3]> = BTreeMap::new();
    for (k, t) in &head {
        match base.get(k) {
            None => groups.entry(kind(k)).or_default()[0].push(k),
            Some(old) if old != t => groups.entry(kind(k)).or_default()[1].push(k),
            Some(_) => {}
        }
    }
    for k in base.keys().filter(|k| !head.contains_key(*k)) {
        groups.entry(kind(k)).or_default()[2].push(k);
    }
    if groups.is_empty() {
        return "no content file differs".into();
    }
    let mut out = String::from("| Kind | Added | Changed | Removed |\n|---|---|---|---|\n");
    for (k, [a, c, r]) in &groups {
        out.push_str(&format!("| {k} | {} | {} | {} |\n", a.len(), c.len(), r.len()));
    }
    for (k, [a, c, r]) in &groups {
        for (label, keys) in [("added", a), ("changed", c), ("removed", r)] {
            for key in keys.iter().take(20) {
                out.push_str(&format!("{label}: {key}\n"));
            }
            if keys.len() > 20 {
                out.push_str(&format!("{label}: … {} more {k}\n", keys.len() - 20));
            }
        }
    }
    out
}

fn preflight(root: &Root, base: &str) -> Result<()> {
    let mut rows: Vec<(&str, Result<String>)> = Vec::new();
    rows.push(("content checks (validate)", crate::validate(root).map(|()| "ok".into())));
    rows.push((
        "schemas and relationships (kabar-content)",
        kabar_content::check_repo(root.path()).map_err(|e| anyhow::anyhow!("{e}")).and_then(|r| {
            for p in &r.problems {
                println!("content: {p}");
            }
            ensure!(r.problems.is_empty(), "{} problem(s)", r.problems.len());
            Ok("ok".into())
        }),
    ));
    let scratch = std::env::temp_dir().join(format!("kabar-preflight-{}", std::process::id()));
    rows.push((
        "bundle devices take",
        crate::update::bundle(root.path(), &scratch).map(|(p, n)| {
            let size = std::fs::metadata(&p).map_or(0, |m| m.len());
            format!("{n} files, {:.1} MB", size as f64 / 1e6)
        }),
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    let head = ids::working_files(root.path());
    let base_files = ids::git_files(root.path(), base);
    match (&head, &base_files) {
        (Ok(head), Ok(base_files)) => {
            let (b, h) =
                (kabar_core::content_ids::registry(base_files), kabar_core::content_ids::registry(head));
            rows.push((
                "content ids (records keep their targets)",
                ids::check(root.path(), &b, &h).map(|()| format!("{} ids", h.len())),
            ));
            println!("\nContent compared with {base}:\n{}", diff(base_files, head));
        }
        (Err(e), _) | (_, Err(e)) => {
            rows.push(("content ids (records keep their targets)", Err(anyhow::anyhow!("{e:#}"))))
        }
    }
    println!("\n| Check | Result |\n|---|---|");
    let mut failed = 0;
    for (name, result) in &rows {
        match result {
            Ok(note) => println!("| {name} | ✓ {note} |"),
            Err(e) => {
                failed += 1;
                println!("| {name} | ✗ {} |", format!("{e:#}").replace('\n', " "));
            }
        }
    }
    ensure!(failed == 0, "{failed} check(s) failed; see above");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_copy() -> tempfile::TempDir {
        // Only what new_lesson reads: the template, and empty content dirs.
        let dir = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::create_dir_all(dir.path().join("content/templates")).unwrap();
        std::fs::create_dir_all(dir.path().join("content/lessons/id")).unwrap();
        std::fs::create_dir_all(dir.path().join("content/lessons/jv")).unwrap();
        std::fs::copy(root.join(TEMPLATE), dir.path().join(TEMPLATE)).unwrap();
        dir
    }

    #[test]
    fn a_new_lesson_starts_from_the_template_and_cannot_ship_unfinished() {
        let dir = repo_copy();
        let path = new_lesson(dir.path(), "j2.u03.l09").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(path.ends_with("content/lessons/jv/j2.u03.l09.yaml"));
        assert!(text.starts_with("schema: kabar.lesson/v1\nid: \"j2.u03.l09\"\nlanguage: \"jv\""), "{text}");
        let lesson = kabar_core::lesson::parse_lesson(&text).unwrap();
        assert_eq!((lesson.id.as_str(), lesson.exercises.len()), ("j2.u03.l09", 6));
        assert!(text.contains(kabar_core::bundle_check::TEMPLATE_MARK));
        // Never over an existing file, a bad id, or a retired id.
        assert!(new_lesson(dir.path(), "j2.u03.l09").is_err());
        assert!(new_lesson(dir.path(), "BIPA1.u01").is_err());
        std::fs::write(dir.path().join(ids::RETIRED), "lesson bipa1.u01.l40  # gone\n").unwrap();
        assert!(new_lesson(dir.path(), "bipa1.u01.l40").unwrap_err().to_string().contains("retired"));
    }

    #[test]
    fn the_diff_counts_files_by_kind() {
        let f = |k: &str, t: &str| (k.to_owned(), t.to_owned());
        let base = vec![f("LESSONS/a", "1"), f("LESSONS/b", "1"), f("CAST", "x")];
        let head = vec![f("LESSONS/a", "2"), f("LESSONS/c", "1"), f("CAST", "x")];
        let out = diff(&base, &head);
        assert!(out.contains("| LESSONS | 1 | 1 | 1 |"), "{out}");
        assert!(out.contains("changed: LESSONS/a") && out.contains("removed: LESSONS/b"));
        assert_eq!(diff(&base, &base), "no content file differs");
    }
}
