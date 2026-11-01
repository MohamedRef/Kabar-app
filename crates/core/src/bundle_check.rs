//! Checks a content bundle's files before they are published or activated
//! (`docs/RELEASE.md` §5). Shared by `kabar-tools update bundle` (before a
//! bundle is published) and the app (when a downloaded bundle is taken),
//! so both apply the same rules. Pure: callers unpack the files.
//!
//! Two levels:
//! - [`check_keys`] needs only the bundle's keys, so it is cheap enough to
//!   run at every launch: no duplicate or empty keys, every single file the
//!   app needs, at least one lesson. A bundle missing a curriculum would mix
//!   its lessons with another generation's routes.
//! - [`check_files`] parses what the app parses: both curricula (unique
//!   level, unit and lesson ids), every lesson and test (schema version,
//!   id matching its file, the player's shape rules) and the cast.
//!
//! Keys this build doesn't know are allowed: a newer bundle may carry
//! content for a newer app, and this build simply never reads it.

use std::collections::{BTreeMap, BTreeSet};

use kabar_protocol::bundle::{CSS_PREFIX, SOURCES, Source};

/// What `content/templates/*.yaml` puts where the author still has to
/// write something (`kabar-tools content new-lesson`).
pub const TEMPLATE_MARK: &str = "TODO(author)";

/// One problem, naming the bundle key and, when there is one, the id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub key: String,
    pub reason: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.key, self.reason)
    }
}

fn issue(key: &str, reason: impl Into<String>) -> Issue {
    Issue { key: key.to_owned(), reason: reason.into() }
}

/// Keys every bundle must carry.
pub fn required_singles() -> impl Iterator<Item = &'static str> {
    SOURCES.iter().filter_map(|s| match s {
        Source::Single { name, .. } => Some(*name),
        _ => None,
    })
}

/// Whether this build reads `key`.
pub fn known_key(key: &str) -> bool {
    if let Some(path) = key.strip_prefix(CSS_PREFIX) {
        return path.ends_with(".css") && !path.split('/').any(|p| p.is_empty() || p == "..");
    }
    SOURCES.iter().any(|s| match s {
        Source::Single { name, .. } => key == *name,
        Source::Table { name, .. } | Source::List { name, .. } => key
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|id| !id.is_empty() && !id.contains('/')),
    })
}

/// The cheap check: keys only.
pub fn check_keys<'a>(keys: impl IntoIterator<Item = &'a str>) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    let mut lessons = 0usize;
    for key in keys {
        if key.is_empty() {
            issues.push(issue("(empty key)", "a file has no name"));
        } else if !seen.insert(key) {
            issues.push(issue(key, "appears twice"));
        }
        lessons += usize::from(key.starts_with("LESSONS/"));
    }
    for name in required_singles() {
        if !seen.contains(name) {
            issues.push(issue(name, "is missing"));
        }
    }
    if lessons == 0 {
        issues.push(issue("LESSONS", "the bundle has no lessons"));
    }
    issues
}

/// The full check over unpacked files `(key, text)`. Empty = valid.
pub fn check_files(files: &[(String, String)]) -> Vec<Issue> {
    let mut issues = check_keys(files.iter().map(|(k, _)| k.as_str()));
    let by_key: BTreeMap<&str, &str> = files.iter().map(|(k, t)| (k.as_str(), t.as_str())).collect();

    // Curricula: parse, unique ids across both routes.
    let mut route_ids: BTreeMap<String, &str> = BTreeMap::new();
    for key in ["CURRICULUM_ID", "CURRICULUM_JV"] {
        let Some(text) = by_key.get(key) else { continue };
        let curriculum = match crate::journey::parse_curriculum(text) {
            Ok(c) => c,
            Err(e) => {
                issues.push(issue(key, e.to_string()));
                continue;
            }
        };
        let mut claim = |id: &str, what: &str| {
            if id.trim().is_empty() {
                issues.push(issue(key, format!("a {what} has an empty id")));
            } else if let Some(other) = route_ids.insert(id.to_owned(), key) {
                issues.push(issue(key, format!("{what} id {id:?} is used twice (also in {other})")));
            }
        };
        for level in &curriculum.levels {
            claim(&level.id, "level");
            for unit in &level.units {
                claim(&unit.id, "unit");
                for lesson in &unit.lessons {
                    claim(&lesson.id, "lesson");
                }
            }
        }
    }

    for (key, text) in files {
        if (key.starts_with("LESSONS/") || key.starts_with("TESTS/")) && text.contains(TEMPLATE_MARK) {
            issues.push(issue(key, format!("unfinished: replace every {TEMPLATE_MARK} from the template")));
        }
        if let Some(id) = key.strip_prefix("LESSONS/") {
            match crate::lesson::parse_lesson(text) {
                Err(e) => issues.push(issue(key, e.to_string())),
                Ok(lesson) => {
                    if lesson.id != id {
                        issues.push(issue(key, format!("its id is {:?}", lesson.id)));
                    }
                    issues.extend(
                        crate::lesson::validate(&lesson).into_iter().map(|i| issue(key, i.to_string())),
                    );
                }
            }
        } else if let Some(id) = key.strip_prefix("TESTS/") {
            match crate::lesson::level_test::parse_test(text) {
                Err(e) => issues.push(issue(key, e.to_string())),
                Ok(test) => {
                    if test.id != id {
                        issues.push(issue(key, format!("its id is {:?}", test.id)));
                    }
                    issues.extend(
                        crate::lesson::level_test::validate_test(&test)
                            .into_iter()
                            .map(|i| issue(key, i.to_string())),
                    );
                }
            }
        } else if key == "CAST"
            && let Err(e) = crate::cast::Cast::parse(text)
        {
            issues.push(issue(key, e.to_string()));
        }
    }
    issues
}

/// The smallest set of files that passes [`check_files`]: both curricula
/// with no levels, an empty cast, one real lesson, and the other single
/// files empty. For tests of code that takes bundles (the app's updater,
/// `kabar-tools`), with `cast` as the CAST text so tests can tell bundles
/// apart.
#[doc(hidden)]
pub fn sample_files(cast: &str) -> Vec<(String, String)> {
    const LESSON: &str = include_str!("../../../content/lessons/id/bipa1.u01.l01.yaml");
    let curriculum = |lang: &str| {
        format!(
            "schema: kabar.curriculum/v1\nlanguage: {lang}\ntitle: {{ en: Sample, ar: عينة }}\nlevels: []\n"
        )
    };
    let mut files: Vec<(String, String)> = required_singles()
        .map(|name| {
            let text = match name {
                "CURRICULUM_ID" => curriculum("id"),
                "CURRICULUM_JV" => curriculum("jv"),
                "CAST" => cast.to_owned(),
                _ => String::new(),
            };
            (name.to_owned(), text)
        })
        .collect();
    files.push(("LESSONS/bipa1.u01.l01".to_owned(), LESSON.to_owned()));
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_files() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        SOURCES
            .iter()
            .flat_map(|s| kabar_protocol::bundle::source_files(&root, *s))
            .map(|(k, p)| (k, std::fs::read_to_string(p).unwrap()))
            .collect()
    }

    #[test]
    fn the_repo_content_passes() {
        let issues = check_files(&repo_files());
        assert!(issues.is_empty(), "{}", issues.iter().map(Issue::to_string).collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn broken_bundles_are_named_by_file_and_reason() {
        let files = repo_files();
        let replace = |key: &str, text: &str| -> Vec<(String, String)> {
            files
                .iter()
                .map(|(k, t)| (k.clone(), if k == key { text.to_owned() } else { t.clone() }))
                .collect()
        };
        let lesson = "LESSONS/bipa1.u01.l01";
        let original = &files.iter().find(|(k, _)| k == lesson).unwrap().1;

        // Schema failure: the lesson no longer parses.
        let issues = check_files(&replace(lesson, "id: [unclosed"));
        assert!(issues.iter().any(|i| i.key == lesson), "{issues:?}");

        // A lesson file carrying another lesson's id.
        let renamed = original.replacen("bipa1.u01.l01", "bipa1.u01.l99", 1);
        let issues = check_files(&replace(lesson, &renamed));
        assert!(issues.iter().any(|i| i.key == lesson && i.reason.contains("bipa1.u01.l99")), "{issues:?}");

        // Unsupported content version.
        let future = original.replacen("kabar.lesson/v1", "kabar.lesson/v9", 1);
        let issues = check_files(&replace(lesson, &future));
        assert!(issues.iter().any(|i| i.key == lesson && i.reason.contains("v9")), "{issues:?}");

        // Duplicate lesson id in a curriculum.
        let curriculum = &files.iter().find(|(k, _)| k == "CURRICULUM_ID").unwrap().1;
        let doubled = curriculum.replacen("id: bipa1.u01.l02", "id: bipa1.u01.l01", 1);
        assert_ne!(&doubled, curriculum, "fixture assumption");
        let issues = check_files(&replace("CURRICULUM_ID", &doubled));
        assert!(
            issues.iter().any(|i| i.key == "CURRICULUM_ID" && i.reason.contains("bipa1.u01.l01")),
            "{issues:?}"
        );

        // A missing curriculum and a duplicate key.
        let mut short: Vec<_> = files.iter().filter(|(k, _)| k != "CURRICULUM_JV").cloned().collect();
        short.push(files[0].clone());
        let issues = check_files(&short);
        assert!(issues.iter().any(|i| i.key == "CURRICULUM_JV" && i.reason == "is missing"));
        assert!(issues.iter().any(|i| i.key == files[0].0 && i.reason == "appears twice"));
    }

    #[test]
    fn a_lesson_from_the_template_parses_but_cannot_ship_until_filled() {
        let template = include_str!("../../../content/templates/lesson.yaml")
            .replace("{{id}}", "bipa1.u01.l01")
            .replace("{{language}}", "id");
        let mut files = sample_files("characters: []\n");
        files.iter_mut().find(|(k, _)| k == "LESSONS/bipa1.u01.l01").unwrap().1 = template.clone();
        let issues = check_files(&files);
        assert_eq!(issues.len(), 1, "only the unfinished marker: {issues:?}");
        assert!(issues[0].reason.starts_with("unfinished"));
        // Filled in, it ships.
        files.iter_mut().find(|(k, _)| k == "LESSONS/bipa1.u01.l01").unwrap().1 =
            template.replace(TEMPLATE_MARK, "Isi");
        assert_eq!(check_files(&files), Vec::new());
    }

    #[test]
    fn the_sample_passes() {
        assert_eq!(check_files(&sample_files("characters: []\n")), Vec::new());
        assert!(!check_files(&sample_files("characters: [")).is_empty(), "a broken cast is caught");
    }

    #[test]
    fn keys_from_a_newer_app_are_allowed_but_not_known() {
        let mut keys: Vec<&str> = required_singles().collect();
        keys.extend(["LESSONS/bipa1.u01.l01", "MAPS/new-thing", "css/assets/main.css"]);
        assert!(check_keys(keys.iter().copied()).is_empty());
        assert!(!known_key("MAPS/new-thing"));
        assert!(known_key("css/assets/main.css") && !known_key("css/../etc.css"));
        assert!(known_key("LESSONS/bipa1.u01.l01") && !known_key("LESSONS/"));
        // Only CAST: every other single and the lessons are missing.
        assert_eq!(check_keys(["CAST"]).len(), required_singles().count());
    }
}
