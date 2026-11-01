//! The content ids learner records point at, and what changed about them
//! between two versions of the content (`docs/CONTENT-IDS.md`).
//!
//! Progress, answers and reviews are stored against ids written in the
//! content: lesson events by lesson id, the Lanjut position and saved
//! answers by exercise id inside a lesson, plan items by unit id, review
//! cards by lesson id and word, test results by test and item id. An id
//! that disappears or starts meaning something else silently cuts a
//! learner's history loose. So every content change is compared with the
//! last version (`kabar-tools ids check`), and:
//!
//! - an id added is fine;
//! - an id removed is **breaking**, unless `content/ids-retired.txt` names
//!   it (an explicit, reviewed decision);
//! - an exercise whose `type` changed is **breaking** (repurposed: saved
//!   answers would be graded against a different exercise);
//! - an id that moved (a lesson to another unit, a test item to another
//!   section) is reported but allowed: records keep pointing at it.
//!
//! Pure: callers pass the unpacked files `(bundle key, text)`, the same
//! shape `bundle_check` takes, so a git checkout and a published bundle are
//! read alike.

use std::collections::{BTreeMap, BTreeSet};

/// Every persistent id → what it is attached to (its parent, or for an
/// exercise its type). Keys read like `lesson bipa1.u01.l01`.
pub type Registry = BTreeMap<String, String>;

/// Reads the ids out of a content bundle's files. Files that don't parse
/// are skipped here: `bundle_check` reports them.
pub fn registry(files: &[(String, String)]) -> Registry {
    let mut ids = Registry::new();
    for (key, text) in files {
        match key.as_str() {
            "CURRICULUM_ID" | "CURRICULUM_JV" => {
                let Ok(c) = crate::journey::parse_curriculum(text) else { continue };
                for level in &c.levels {
                    ids.insert(format!("level {}", level.id), format!("curriculum {}", c.language));
                    if let Some(t) = &level.level_test {
                        ids.insert(format!("level-test {}", t.id), format!("level {}", level.id));
                    }
                    for unit in &level.units {
                        ids.insert(format!("unit {}", unit.id), format!("level {}", level.id));
                        for lesson in &unit.lessons {
                            ids.insert(format!("lesson {}", lesson.id), format!("unit {}", unit.id));
                        }
                    }
                }
            }
            _ => {}
        }
        if let Some(id) = key.strip_prefix("LESSONS/") {
            let Ok(lesson) = crate::lesson::parse_lesson(text) else { continue };
            for e in &lesson.exercises {
                let kind = serde_json::to_value(&e.kind)
                    .ok()
                    .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_owned))
                    .unwrap_or_default();
                ids.insert(format!("exercise {id}#{}", e.id), format!("type {kind}"));
            }
            if let Some(s) = &lesson.souvenir {
                ids.insert(format!("souvenir {}", s.id), format!("lesson {id}"));
            }
        } else if let Some(id) = key.strip_prefix("TESTS/") {
            let Ok(test) = crate::lesson::level_test::parse_test(text) else { continue };
            ids.insert(format!("test {id}"), "tests".into());
            for section in &test.sections {
                for item in &section.items {
                    ids.insert(format!("test-item {id}#{}", item.id), format!("section {}", section.id));
                }
            }
        } else {
            for table in ["STORIES", "RADIO", "PERSONAS", "GRAMMAR"] {
                if let Some(id) = key.strip_prefix(table).and_then(|r| r.strip_prefix('/')) {
                    ids.insert(format!("{} {id}", table.to_ascii_lowercase()), "file".into());
                }
            }
        }
    }
    ids
}

/// What changed between two registries.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub added: Vec<String>,
    /// Gone, and not retired: breaking.
    pub removed: Vec<(String, String)>,
    /// Gone, and listed in `content/ids-retired.txt`.
    pub retired: Vec<String>,
    /// Same id, a different exercise type: breaking.
    pub repurposed: Vec<(String, String, String)>,
    /// Same id under another parent: allowed.
    pub moved: Vec<(String, String, String)>,
}

impl Report {
    pub fn breaking(&self) -> bool {
        !self.removed.is_empty() || !self.repurposed.is_empty()
    }

    /// A short report, one line per finding, breaking ones first.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (id, was) in &self.removed {
            out.push(format!("BREAKING removed: {id} ({was}); records that point at it lose it"));
        }
        for (id, old, new) in &self.repurposed {
            out.push(format!("BREAKING repurposed: {id} was {old}, now {new}; saved answers would mismatch"));
        }
        for (id, old, new) in &self.moved {
            out.push(format!("moved: {id} from {old} to {new}"));
        }
        for id in &self.retired {
            out.push(format!("retired: {id} (content/ids-retired.txt)"));
        }
        if !self.added.is_empty() {
            out.push(format!("added: {} id(s)", self.added.len()));
        }
        out
    }
}

/// The ids in `content/ids-retired.txt`: one per line. A line starting
/// with `#` is a comment, and so is anything after ` #` (ids themselves
/// contain `#`, as in `exercise bipa1.u01.l01#e01`).
pub fn parse_retired(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|l| l.split(" #").next().unwrap_or_default().trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

pub fn compare(base: &Registry, head: &Registry, retired: &BTreeSet<String>) -> Report {
    let mut r = Report::default();
    for (id, was) in base {
        match head.get(id) {
            None if retired.contains(id) => r.retired.push(id.clone()),
            None => r.removed.push((id.clone(), was.clone())),
            Some(now) if now != was => {
                if id.starts_with("exercise ") {
                    r.repurposed.push((id.clone(), was.clone(), now.clone()));
                } else {
                    r.moved.push((id.clone(), was.clone(), now.clone()));
                }
            }
            Some(_) => {}
        }
    }
    r.added = head.keys().filter(|id| !base.contains_key(*id)).cloned().collect();
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle_check::sample_files;

    fn with(files: &[(String, String)], key: &str, edit: impl Fn(&str) -> String) -> Vec<(String, String)> {
        files.iter().map(|(k, t)| (k.clone(), if k == key { edit(t) } else { t.clone() })).collect()
    }

    const LESSON: &str = "LESSONS/bipa1.u01.l01";

    #[test]
    fn the_repo_has_every_kind_of_id() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let files: Vec<_> = kabar_protocol::bundle::SOURCES
            .iter()
            .flat_map(|s| kabar_protocol::bundle::source_files(&root, *s))
            .map(|(k, p)| (k, std::fs::read_to_string(p).unwrap()))
            .collect();
        let ids = registry(&files);
        for kind in ["level ", "unit ", "lesson ", "exercise ", "test ", "test-item ", "stories ", "grammar "]
        {
            assert!(ids.keys().any(|k| k.starts_with(kind)), "{kind}");
        }
        assert_eq!(ids.get("lesson bipa1.u01.l01").map(String::as_str), Some("unit bipa1.u01"));
    }

    #[test]
    fn safe_changes_pass_and_breaking_ones_are_named() {
        let base_files = sample_files("characters: []\n");
        let base = registry(&base_files);
        let none = BTreeSet::new();
        assert!(base.contains_key("exercise bipa1.u01.l01#e01"));

        // A text correction changes no id.
        let fixed = with(&base_files, LESSON, |t| t.replacen("title:", "title: ", 1));
        assert_eq!(compare(&base, &registry(&fixed), &none), Report::default());

        // A new exercise is an addition.
        let mut head = base.clone();
        head.insert("exercise bipa1.u01.l01#e99".into(), "type choice".into());
        let r = compare(&base, &head, &none);
        assert!(!r.breaking() && r.added == ["exercise bipa1.u01.l01#e99"]);

        // Renaming an exercise id: the old one is removed.
        let renamed = with(&base_files, LESSON, |t| t.replacen("id: e01", "id: e01b", 1));
        let r = compare(&base, &registry(&renamed), &none);
        assert!(r.breaking());
        assert_eq!(r.removed[0].0, "exercise bipa1.u01.l01#e01");
        assert_eq!(r.added, ["exercise bipa1.u01.l01#e01b"]);

        // ...unless it was retired on purpose.
        let retired = parse_retired("# retired on purpose\nexercise bipa1.u01.l01#e01  # merged into e01b\n");
        let r = compare(&base, &registry(&renamed), &retired);
        assert!(!r.breaking() && r.retired == ["exercise bipa1.u01.l01#e01"], "{r:?}");

        // The same id for a different kind of exercise.
        let mut head = base.clone();
        head.insert("exercise bipa1.u01.l01#e01".into(), "type something_else".into());
        let r = compare(&base, &head, &none);
        assert!(r.breaking() && r.repurposed.len() == 1);
        assert!(r.lines()[0].starts_with("BREAKING repurposed"));

        // A lesson moved to another unit keeps its records: reported only.
        let mut base = Registry::new();
        base.insert("lesson x.u01.l01".into(), "unit x.u01".into());
        let mut head = base.clone();
        head.insert("lesson x.u01.l01".into(), "unit x.u02".into());
        let r = compare(&base, &head, &none);
        assert!(!r.breaking() && r.moved.len() == 1);
    }
}
