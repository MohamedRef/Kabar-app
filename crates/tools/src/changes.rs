//! "What's new" for each build (`docs/RELEASE.md` §5).
//!
//! A pull request with a change the learner can see adds `changes/<topic>.yaml`:
//!
//! ```yaml
//! - kind: new          # new | improved | fixed
//!   ar_eg: كبار بيبعتلك إشعار لما نسخة جديدة تنزل
//!   en: Kabar tells you when a new version is out
//! ```
//!
//! When CI builds a feed it gathers the files added since the build already
//! on the feed (one entry per merged pull request), and puts that build's
//! entry on top of the feed's history. Lines a pull request appends to an
//! existing file count too. A file holding `[]` says "nothing to tell". CI
//! fails a pull request that adds no lines (`.github/workflows/ci.yml`).
//!
//! A commit without a file (pushed straight to main, or merged before the
//! check) is looked up by title in [`RETRO`], which also rewrites the
//! English-only lines older feeds carry; only then does it fall back to its
//! title in English. CI, docs, refactor and test titles are skipped.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use kabar_protocol::update::{BuildChanges, Change, ChangeKind, HISTORY_BUILDS, UpdateManifest, UpdateNotes};
use serde::Deserialize;

/// Where the files live, from the repository root.
pub const DIR: &str = "changes";
/// What pull requests merged without a file said, by title: Arabic and
/// English lines, or `[]` for nothing to tell.
pub const RETRO: &str = "changes/retro/titles.yaml";
/// A line longer than this doesn't fit the notification or the sheet.
const MAX_CHARS: usize = 160;
/// The feed stays well under the app's 256 KB limit.
const MAX_HISTORY_BYTES: usize = 160 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Line {
    kind: ChangeKind,
    ar_eg: String,
    en: String,
    #[serde(default)]
    ar: String,
}

/// Parses and checks one `changes/*.yaml`.
pub fn parse(text: &str) -> Result<Vec<Change>> {
    let lines: Vec<Line> = serde_yaml_ng::from_str(text)?;
    convert(lines)
}

/// Parses and checks [`RETRO`] (missing: empty).
pub fn retro(root: &Path) -> Result<BTreeMap<String, Vec<Change>>> {
    let Ok(text) = std::fs::read_to_string(root.join(RETRO)) else { return Ok(BTreeMap::new()) };
    parse_retro(&text).context(RETRO)
}

fn parse_retro(text: &str) -> Result<BTreeMap<String, Vec<Change>>> {
    let map: BTreeMap<String, Vec<Line>> = serde_yaml_ng::from_str(text)?;
    map.into_iter()
        .map(|(title, lines)| Ok((title.clone(), convert(lines).with_context(|| format!("{title:?}"))?)))
        .collect()
}

fn convert(lines: Vec<Line>) -> Result<Vec<Change>> {
    lines
        .into_iter()
        .map(|l| {
            for (lang, s) in [("ar_eg", &l.ar_eg), ("en", &l.en), ("ar", &l.ar)] {
                let n = s.trim().chars().count();
                ensure!(lang == "ar" || n > 0, "{lang} is empty");
                ensure!(n <= MAX_CHARS, "{lang} is {n} characters (at most {MAX_CHARS})");
                ensure!(!s.contains('\n'), "{lang} is more than one line");
            }
            ensure!(l.ar_eg.chars().any(is_arabic), "ar_eg has no Arabic: {:?}", l.ar_eg);
            Ok(Change {
                kind: l.kind,
                text: UpdateNotes {
                    en: l.en.trim().into(),
                    ar_eg: l.ar_eg.trim().into(),
                    ar: l.ar.trim().into(),
                },
            })
        })
        .collect()
}

fn is_arabic(c: char) -> bool {
    ('\u{0600}'..='\u{06FF}').contains(&c)
}

/// A merged pull request without a changes file: its title, unless it's
/// housekeeping the learner never sees.
pub fn from_title(title: &str) -> Option<Change> {
    let t = clean_title(title);
    let t = t.as_str();
    let lower = t.to_lowercase();
    let first = lower.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("");
    const NOISE: [&str; 20] = [
        "ci", "build", "chore", "refactor", "docs", "doc", "test", "tests", "style", "deps", "bump",
        "release", "lint", "fmt", "clippy", "critic", "critique", "merge", "revert", "wip",
    ];
    if t.is_empty() || NOISE.contains(&first) || lower.contains("readme") || lower.contains("github action") {
        return None;
    }
    let kind = match first {
        "fix" | "fixes" | "fixed" => ChangeKind::Fixed,
        "add" | "adds" | "new" | "introduce" | "support" => ChangeKind::New,
        _ => ChangeKind::Improved,
    };
    let en: String = t.chars().take(MAX_CHARS).collect();
    Some(Change { kind, text: UpdateNotes { en, ..UpdateNotes::default() } })
}

/// A pull request's title as the feed shows it: no "(#123)", "(Fixes
/// #402)" or "(Part of #140)", no final full stop.
fn clean_title(title: &str) -> String {
    let mut t = title.trim().to_owned();
    while let Some(open) = t.rfind(" (")
        && t.ends_with(')')
        && t[open..].contains('#')
    {
        t.truncate(open);
    }
    t.trim().trim_end_matches('.').to_owned()
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().context("running git")?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The title a merge commit or a squashed commit carries.
fn title_of(subject: &str, body: &str) -> String {
    if subject.starts_with("Merge pull request") {
        body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_owned()
    } else {
        subject.to_owned()
    }
}

/// Everything merged into `HEAD` after `since` (just `HEAD` itself when
/// `since` is unknown or not in this history), oldest first.
pub fn collect(root: &Path, since: Option<&str>) -> Result<Vec<Change>> {
    let retro = retro(root)?;
    let head = git(root, &["rev-parse", "HEAD"])?.trim().to_owned();
    let since = since.filter(|s| git(root, &["merge-base", "--is-ancestor", s, "HEAD"]).is_ok());
    let commits: Vec<String> = match since {
        Some(s) => git(root, &["rev-list", "--first-parent", "--reverse", &format!("{s}..HEAD")])?
            .lines()
            .map(str::to_owned)
            .collect(),
        None => vec![head],
    };
    let mut out = Vec::new();
    for c in &commits {
        let parent = git(root, &["rev-parse", "--verify", "--quiet", &format!("{c}^1")]).ok();
        let parent = parent.as_deref().map(str::trim);
        let touched = match parent {
            Some(p) => git(root, &["diff", "--name-status", "--diff-filter=AM", p, c, "--", DIR])?,
            None => git(root, &["ls-tree", "-r", "--name-only", c, "--", DIR])?
                .lines()
                .map(|f| format!("A\t{f}\n"))
                .collect(),
        };
        // Fragments only: `changes/<topic>.yaml`, not the retro folder.
        let files: Vec<(&str, &str)> = touched
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .filter(|(_, f)| f.ends_with(".yaml") && f.matches('/').count() == 1)
            .collect();
        if files.is_empty() {
            let log = git(root, &["log", "-1", "--format=%s%x00%b", c])?;
            let (subject, body) = log.split_once('\0').unwrap_or((&log, ""));
            let title = title_of(subject.trim(), body);
            match retro.get(clean_title(&title).as_str()) {
                Some(lines) => out.extend(lines.iter().cloned()),
                None => out.extend(from_title(&title)),
            }
            continue;
        }
        for (status, f) in files {
            let at = || format!("{f} at {}", &c[..7.min(c.len())]);
            let now = parse(&git(root, &["show", &format!("{c}:{f}")])?).with_context(at)?;
            // A file that grew: only the lines this commit added.
            let before = match (status, parent) {
                ("M", Some(p)) => parse(&git(root, &["show", &format!("{p}:{f}")])?).unwrap_or_default(),
                _ => Vec::new(),
            };
            out.extend(now.into_iter().filter(|l| !before.contains(l)));
        }
    }
    Ok(out)
}

/// This build's entry on top of the previous feed's history (same channel),
/// without repeats, cut to [`HISTORY_BUILDS`] and the size limit.
///
/// English-only lines that [`RETRO`] names are rewritten on the way, so
/// older builds catch up with their Arabic; a build left with no lines
/// goes.
pub fn history(
    previous: Option<&UpdateManifest>,
    entry: Option<BuildChanges>,
    retro: &BTreeMap<String, Vec<Change>>,
) -> Vec<BuildChanges> {
    let mut all: Vec<BuildChanges> = entry.into_iter().collect();
    for old in previous.map(|m| m.history.as_slice()).unwrap_or_default() {
        if !all.iter().any(|e| e.key() == old.key()) {
            let mut old = old.clone();
            old.changes = old
                .changes
                .into_iter()
                .flat_map(|c| {
                    match retro.get(c.text.en.as_str()).filter(|_| c.text.ar_eg.trim().is_empty()) {
                        Some(lines) => lines.clone(),
                        None => vec![c],
                    }
                })
                .collect();
            // A build whose every line was housekeeping says nothing.
            if !old.changes.is_empty() {
                all.push(old);
            }
        }
    }
    all.truncate(HISTORY_BUILDS);
    while all.len() > 1 && serde_json::to_vec(&all).map_or(0, |b| b.len()) > MAX_HISTORY_BYTES {
        all.pop();
    }
    all
}

/// The one-paragraph notes older apps show: the first few lines, per language.
pub fn notes_from(changes: &[Change]) -> UpdateNotes {
    let pick = |f: fn(&UpdateNotes) -> &str| {
        changes
            .iter()
            .take(3)
            .map(|c| {
                let s = f(&c.text);
                if s.is_empty() { c.text.en.as_str() } else { s }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    UpdateNotes {
        en: pick(|n| &n.en),
        ar_eg: pick(|n| &n.ar_eg),
        ar: pick(|n| if n.ar.is_empty() { &n.ar_eg } else { &n.ar }),
    }
}

/// Checks every file under `changes/`.
pub fn check_all(root: &Path) -> Result<usize> {
    let dir = root.join(DIR);
    let mut n = 0;
    for e in std::fs::read_dir(&dir).with_context(|| dir.display().to_string())? {
        let path = e?.path();
        if path.extension().is_some_and(|x| x == "yaml") {
            parse(&std::fs::read_to_string(&path)?).with_context(|| path.display().to_string())?;
            n += 1;
        }
    }
    retro(root)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_changes_file_checks_out() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(check_all(&root).unwrap() > 0);
    }

    #[test]
    fn a_file_needs_egyptian_arabic_and_english() {
        let ok = "- kind: fixed\n  ar_eg: الصوت مبقاش بيقطع\n  en: Audio no longer cuts out\n";
        let c = parse(ok).unwrap();
        assert_eq!((c[0].kind, c[0].text.en.as_str()), (ChangeKind::Fixed, "Audio no longer cuts out"));
        assert!(parse("- kind: fixed\n  ar_eg: Audio\n  en: Audio\n").is_err());
        assert!(parse("- kind: new\n  ar_eg: صوت\n").is_err());
        assert!(parse("- kind: moved\n  ar_eg: صوت\n  en: x\n").is_err());
        assert!(parse("[]").unwrap().is_empty());
    }

    #[test]
    fn titles_fall_back_without_the_noise() {
        let k = |t: &str| from_title(t).map(|c| (c.kind, c.text.en));
        assert_eq!(
            k("Journey bar fits a 360 px phone in English (Fixes #402) (#408)"),
            Some((ChangeKind::Improved, "Journey bar fits a 360 px phone in English".into()))
        );
        assert_eq!(k("Fix the Reader popover on RTL").unwrap().0, ChangeKind::Fixed);
        assert_eq!(k("Add Radio episodes for J3").unwrap().0, ChangeKind::New);
        for noise in [
            "CI: cache dx",
            "Refactor the router",
            "docs: RUN.md",
            "Refresh README numbers",
            "Merge branch 'main'",
        ] {
            assert_eq!(k(noise), None, "{noise}");
        }
        assert_eq!(title_of("Merge pull request #9 from a/b", "\nNew Radio host\n"), "New Radio host");
    }

    fn entry(build: u64) -> BuildChanges {
        BuildChanges {
            version: "0.1.0".into(),
            build,
            published_at: "2026-10-28T10:00:00Z".into(),
            changes: vec![from_title(&format!("Better thing {build}")).unwrap()],
            content_only: false,
        }
    }

    #[test]
    fn history_puts_the_new_build_first_and_stays_short() {
        let mut prev: Option<UpdateManifest> =
            serde_json::from_str(include_str!("../../../fixtures/update/update.json")).ok();
        let m = prev.as_mut().unwrap();
        m.history = (1..=HISTORY_BUILDS as u64).rev().map(entry).collect();
        let none = BTreeMap::new();
        let h = history(prev.as_ref(), Some(entry(HISTORY_BUILDS as u64 + 1)), &none);
        assert_eq!(h.len(), HISTORY_BUILDS);
        assert_eq!(h[0].build, HISTORY_BUILDS as u64 + 1);
        assert_eq!(h.last().unwrap().build, 2);
        // The same build published twice keeps one entry.
        let again = history(prev.as_ref(), Some(entry(HISTORY_BUILDS as u64)), &none);
        assert_eq!(again.iter().filter(|e| e.build == HISTORY_BUILDS as u64).count(), 1);
        assert!(history(None, None, &none).is_empty());
    }

    #[test]
    fn retro_gives_old_english_lines_their_arabic() {
        let retro = parse_retro(
            "\"Better thing 2\":\n  - kind: fixed\n    ar_eg: حاجة اتصلحت\n    en: A fix\n\"Better thing 1\": []\n",
        )
        .unwrap();
        let mut prev: UpdateManifest =
            serde_json::from_str(include_str!("../../../fixtures/update/update.json")).unwrap();
        prev.history = vec![entry(2), entry(1)];
        let h = history(Some(&prev), Some(entry(3)), &retro);
        assert_eq!(h.len(), 2, "build 1 only had housekeeping");
        let lines: Vec<(u64, ChangeKind, &str)> = h
            .iter()
            .flat_map(|b| b.changes.iter().map(move |c| (b.build, c.kind, c.text.en.as_str())))
            .collect();
        assert_eq!(lines, [(3, ChangeKind::Improved, "Better thing 3"), (2, ChangeKind::Fixed, "A fix")]);
        assert_eq!(h[1].changes[0].text.ar_eg, "حاجة اتصلحت");
        assert!(parse_retro("\"Title\":\n  - kind: new\n    ar_eg: English only\n    en: x\n").is_err());
        assert_eq!(clean_title("Reader: tables (Fixes #9) (#287)"), "Reader: tables");
    }

    #[test]
    fn notes_for_older_apps_take_the_first_lines() {
        let c = parse("- kind: new\n  ar_eg: حاجة جديدة\n  en: A new thing\n- kind: fixed\n  ar_eg: حاجة اتصلحت\n  en: A fix\n")
            .unwrap();
        let n = notes_from(&c);
        assert_eq!(n.en, "A new thing\nA fix");
        assert_eq!(n.ar, "حاجة جديدة\nحاجة اتصلحت");
    }
}
