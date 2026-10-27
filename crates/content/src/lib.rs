//! Build-time content for Kabar: the curriculum (`curriculum/*.yaml`), the
//! grammar library (`content/grammar/**.yaml`), lessons
//! (`content/lessons/**.yaml`), level/placement tests (`content/tests/**.yaml`),
//! conversation personas, Sinetron episodes and Kabar Radio scripts.
//!
//! Every file is checked twice: structurally against its JSON Schema in
//! `schemas/`, then semantically (unique ids, ids nested under their parents,
//! every grammar point a lesson teaches exists in the library, ...).
//! `cargo test -p kabar-content` runs both over the whole repository.

pub mod cast;
pub mod core_words;
pub mod lessons;
pub mod model;
pub mod retrieval;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use model::{Curriculum, GrammarPoint};

/// Levels whose grammar points must all exist in the library. Grows as the
/// content pipeline covers more levels (BIPA 1–7 and J1–J6).
pub const GRAMMAR_LIBRARY_LEVELS: &[&str] =
    &["bipa1", "bipa2", "bipa3", "bipa4", "bipa5", "bipa6", "bipa7", "j1", "j2", "j3", "j4", "j5", "j6"];

/// Levels whose every lesson must be written (BIPA 1–2, J1–J3).
pub const LESSON_LEVELS: &[&str] = &["bipa1", "bipa2", "j1", "j2", "j3"];

pub const CURRICULUM_SCHEMA: &str = "schemas/curriculum.schema.json";
pub const GRAMMAR_SCHEMA: &str = "schemas/grammar-point.schema.json";
/// Owned by the Lesson Player; applied when present, on top of [`lessons`].
pub const LESSON_SCHEMA: &str = "schemas/lesson.schema.json";

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("{path}: invalid YAML: {source}")]
    Yaml { path: PathBuf, source: serde_yaml_ng::Error },
    #[error("{path}: invalid JSON: {source}")]
    Json { path: PathBuf, source: serde_json::Error },
    #[error("{path}: bad schema: {message}")]
    Schema { path: PathBuf, message: String },
}

/// Everything wrong with the content, as human-readable lines.
#[derive(Debug, Default)]
pub struct Report {
    pub problems: Vec<String>,
    pub curricula: Vec<Curriculum>,
    pub grammar: Vec<GrammarPoint>,
    /// Ids of the lesson files found (`bipa1.u01.l01`, ...).
    pub lessons: Vec<String>,
    /// Ids of the test files found (`bipa1.test`, `placement`, ...).
    pub tests: Vec<String>,
    /// Ids of the persona, story and radio files found.
    pub stories: Vec<String>,
}

impl Report {
    fn push(&mut self, problem: impl Into<String>) {
        self.problems.push(problem.into());
    }
}

fn read(path: &Path) -> Result<String, LoadError> {
    fs::read_to_string(path).map_err(|source| LoadError::Io { path: path.to_owned(), source })
}

/// Parses a YAML file into a JSON value (for schema validation).
pub fn yaml_value(path: &Path) -> Result<serde_json::Value, LoadError> {
    serde_yaml_ng::from_str(&read(path)?).map_err(|source| LoadError::Yaml { path: path.to_owned(), source })
}

fn validator(path: &Path) -> Result<jsonschema::Validator, LoadError> {
    let schema: serde_json::Value = serde_json::from_str(&read(path)?)
        .map_err(|source| LoadError::Json { path: path.to_owned(), source })?;
    jsonschema::validator_for(&schema)
        .map_err(|e| LoadError::Schema { path: path.to_owned(), message: e.to_string() })
}

fn schema_problems(validator: &jsonschema::Validator, value: &serde_json::Value, file: &Path) -> Vec<String> {
    validator
        .iter_errors(value)
        .map(|e| format!("{}: {} at {}", file.display(), e, e.instance_path()))
        .collect()
}

fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), LoadError> {
    let entries = fs::read_dir(dir).map_err(|source| LoadError::Io { path: dir.to_owned(), source })?;
    for entry in entries {
        let path = entry.map_err(|source| LoadError::Io { path: dir.to_owned(), source })?.path();
        if path.is_dir() {
            yaml_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "yaml") {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}

/// Loads and checks all curriculum and grammar content under `root` (the
/// repository root). I/O and schema-file errors are returned as `Err`;
/// content problems are collected in [`Report::problems`].
pub fn check_repo(root: &Path) -> Result<Report, LoadError> {
    let mut report = Report::default();

    let curriculum_validator = validator(&root.join(CURRICULUM_SCHEMA))?;
    let mut curriculum_files = Vec::new();
    yaml_files(&root.join("curriculum"), &mut curriculum_files)?;
    for file in &curriculum_files {
        let value = yaml_value(file)?;
        let problems = schema_problems(&curriculum_validator, &value, file);
        if problems.is_empty() {
            match serde_json::from_value::<Curriculum>(value) {
                Ok(c) => report.curricula.push(c),
                Err(e) => report.push(format!("{}: {e}", file.display())),
            }
        }
        report.problems.extend(problems);
    }

    let grammar_validator = validator(&root.join(GRAMMAR_SCHEMA))?;
    let mut grammar_files = Vec::new();
    yaml_files(&root.join("content/grammar"), &mut grammar_files)?;
    for file in &grammar_files {
        let value = yaml_value(file)?;
        let problems = schema_problems(&grammar_validator, &value, file);
        if problems.is_empty() {
            match serde_json::from_value::<GrammarPoint>(value) {
                Ok(g) => {
                    if stem(file) != g.id {
                        report.push(format!("{}: file name must be the id ({}.yaml)", file.display(), g.id));
                    }
                    report.grammar.push(g);
                }
                Err(e) => report.push(format!("{}: {e}", file.display())),
            }
        }
        report.problems.extend(problems);
    }

    check_curricula(&mut report);
    check_grammar(&mut report);
    check_lessons_and_tests(root, &mut report)?;
    Ok(report)
}

fn stem(file: &Path) -> &str {
    file.file_stem().and_then(|s| s.to_str()).unwrap_or("")
}

fn check_lessons_and_tests(root: &Path, report: &mut Report) -> Result<(), LoadError> {
    let mut problems = Vec::new();
    let index = lessons::Index::new(&report.curricula, report.grammar.iter().map(|g| g.id.as_str()));
    let schema_path = root.join(LESSON_SCHEMA);
    let lesson_validator = if schema_path.exists() { Some(validator(&schema_path)?) } else { None };

    let mut files = Vec::new();
    let dir = root.join("content/lessons");
    if dir.exists() {
        yaml_files(&dir, &mut files)?;
    }
    let mut lesson_values = BTreeMap::new();
    let mut seen = cast::Appearances::default();
    for file in &files {
        let value = yaml_value(file)?;
        if let Some(v) = &lesson_validator {
            problems.extend(schema_problems(v, &value, file));
        }
        let lang = file.parent().and_then(Path::file_name).and_then(|s| s.to_str()).unwrap_or("");
        lessons::check_lesson(&file.display().to_string(), stem(file), lang, &value, &index, &mut problems);
        retrieval::check(stem(file), &value, &mut problems);
        seen.lesson(&file.display().to_string(), stem(file), &value);
        report.lessons.push(stem(file).to_owned());
        lesson_values.insert(stem(file).to_owned(), value);
    }
    check_forward_words(&report.curricula, &lesson_values, &mut problems);
    check_retaught_words(&report.curricula, &lesson_values, &mut problems);
    core_words::check(root, &report.curricula, &lesson_values, &mut problems);
    for level in LESSON_LEVELS {
        for id in index.lessons_of(level) {
            if !report.lessons.iter().any(|l| l == id) {
                problems.push(format!("{id}: lesson not written yet (content/lessons/)"));
            }
        }
    }

    let mut files = Vec::new();
    let dir = root.join("content/tests");
    if dir.exists() {
        yaml_files(&dir, &mut files)?;
    }
    for file in &files {
        let value = yaml_value(file)?;
        lessons::check_test(&file.display().to_string(), stem(file), &value, &index, &mut problems);
        seen.test(&file.display().to_string(), &value);
        report.tests.push(stem(file).to_owned());
    }

    type Checker = fn(&str, &str, &serde_json::Value, &lessons::Index, &mut Vec<String>);
    let story_dirs: [(&str, Checker); 3] = [
        ("content/personas", lessons::check_persona),
        ("content/stories", lessons::check_story),
        ("content/radio", lessons::check_radio),
    ];
    let mut personas = Vec::new();
    let mut radios = Vec::new();
    for (dir, check) in story_dirs {
        let mut files = Vec::new();
        if root.join(dir).exists() {
            yaml_files(&root.join(dir), &mut files)?;
        }
        for file in &files {
            let value = yaml_value(file)?;
            let name = file.display().to_string();
            check(&name, stem(file), &value, &index, &mut problems);
            report.stories.push(stem(file).to_owned());
            match dir {
                "content/personas" => {
                    seen.persona(&name, &value);
                    personas.push((name, value));
                }
                "content/stories" => seen.story(&name, &value),
                _ => {
                    seen.radio(&name, &value);
                    radios.push((name, value));
                }
            }
        }
    }
    check_radio_words(&report.curricula, &lesson_values, &radios, &mut problems);
    let games = root.join("content/games/unggah-ungguh.toml");
    if games.exists() {
        seen.unggah_ungguh("content/games/unggah-ungguh.toml", &read(&games)?);
    }
    let optional = |path: &str| -> Result<Option<serde_json::Value>, LoadError> {
        let path = root.join(path);
        if path.exists() { yaml_value(&path).map(Some) } else { Ok(None) }
    };
    let (cast_file, pending) = (optional("content/cast.yaml")?, optional("content/pending.yaml")?);
    cast::check(cast_file.as_ref(), pending.as_ref(), &personas, &seen, &index, &mut problems);
    report.problems.extend(problems);
    Ok(())
}

/// An exercise's `words:` tags are the words it practises, so none may be a
/// word that a later lesson (in curriculum order, any level) first teaches in
/// its `new_words`: the learner would be drilled on it before meeting it.
fn check_forward_words(
    curricula: &[Curriculum],
    lessons: &BTreeMap<String, serde_json::Value>,
    problems: &mut Vec<String>,
) {
    use serde_json::Value;
    let words = |v: Option<&Value>, key: &str| -> Vec<String> {
        v.and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|w| if key.is_empty() { w.as_str() } else { w.get(key).and_then(Value::as_str) })
            .map(str::to_lowercase)
            .collect()
    };
    for c in curricula {
        let order: Vec<&str> = c
            .levels
            .iter()
            .flat_map(|l| &l.units)
            .flat_map(|u| &u.lessons)
            .map(|l| l.id.as_str())
            .filter(|id| lessons.contains_key(*id))
            .collect();
        let mut first: BTreeMap<String, usize> = BTreeMap::new();
        for (pos, id) in order.iter().enumerate() {
            for w in words(lessons[*id].get("new_words"), "word") {
                first.entry(w).or_insert(pos);
            }
        }
        for (pos, id) in order.iter().enumerate() {
            for e in lessons[*id].get("exercises").and_then(Value::as_array).into_iter().flatten() {
                let ex = e.get("id").and_then(Value::as_str).unwrap_or("?");
                for w in words(e.get("words"), "") {
                    if let Some(&later) = first.get(&w).filter(|&&p| p > pos) {
                        problems.push(format!(
                            "{id} {ex}: practises '{w}', which is first taught later in {} (teach it earlier or drop the tag)",
                            order[later]
                        ));
                    }
                }
            }
        }
    }
}

/// A Radio episode spotlights unit words on its end card, so each of its
/// `words` must be taught (a lesson's `new_words`) in its unit or an earlier
/// one of the same curriculum: the show reviews, it never introduces (#198).
fn check_radio_words(
    curricula: &[Curriculum],
    lessons: &BTreeMap<String, serde_json::Value>,
    radios: &[(String, serde_json::Value)],
    problems: &mut Vec<String>,
) {
    use serde_json::Value;
    // Unit id -> every word taught up to and including that unit.
    let mut taught_by: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for c in curricula {
        let mut taught = BTreeSet::new();
        for unit in c.levels.iter().flat_map(|l| &l.units) {
            for lesson in unit.lessons.iter().filter_map(|l| lessons.get(&l.id)) {
                let new_words = lesson.get("new_words").and_then(Value::as_array).into_iter().flatten();
                // Every form counts: "apa / opo" teaches apa and opo.
                taught.extend(
                    new_words
                        .filter_map(|w| w.get("word").and_then(Value::as_str))
                        .flat_map(retrieval::forms),
                );
            }
            taught_by.insert(unit.id.as_str(), taught.clone());
        }
    }
    for (file, radio) in radios {
        let unit = radio.get("unit").and_then(Value::as_str).unwrap_or_default();
        let Some(taught) = taught_by.get(unit) else { continue };
        let words =
            radio.get("words").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
        for w in words.filter(|w| !taught.contains(&w.to_lowercase())) {
            problems.push(format!(
                "{file}: spotlight word '{w}' is not taught by {unit} or earlier (use a lesson's new_words, or teach it first)"
            ));
        }
    }
}

/// Words with two unrelated senses, each taught once (minggu = Sunday and
/// week; melamar = propose and apply; dalem = humble "I" and krama inggil
/// "home"; papan = place and board).
const TWO_SENSES: [&str; 4] = ["minggu", "melamar", "dalem", "papan"];

/// A word is taught once, in the first lesson that lists it in `new_words`;
/// later lessons recycle it in exercises and texts but don't present it as
/// new again.
fn check_retaught_words(
    curricula: &[Curriculum],
    lessons: &BTreeMap<String, serde_json::Value>,
    problems: &mut Vec<String>,
) {
    use serde_json::Value;
    for c in curricula {
        let mut first: BTreeMap<String, &str> = BTreeMap::new();
        let ids = c.levels.iter().flat_map(|l| &l.units).flat_map(|u| &u.lessons).map(|l| l.id.as_str());
        for id in ids.filter(|id| lessons.contains_key(*id)) {
            let new_words = lessons[id].get("new_words").and_then(Value::as_array).into_iter().flatten();
            for w in new_words.filter_map(|w| w.get("word").and_then(Value::as_str)).map(str::to_lowercase) {
                match first.get(&w) {
                    Some(&earlier) if earlier != id && !TWO_SENSES.contains(&w.as_str()) => problems.push(format!(
                        "{id}: '{w}' is already taught in {earlier}; drop it from new_words (recycling it in exercises is fine)"
                    )),
                    Some(_) => {}
                    None => {
                        first.insert(w, id);
                    }
                }
            }
        }
    }
}

fn check_curricula(report: &mut Report) {
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    let level_ids: BTreeSet<&str> =
        report.curricula.iter().flat_map(|c| c.levels.iter().map(|l| l.id.as_str())).collect();

    for c in &report.curricula {
        for level in &c.levels {
            let prefix = if c.language == "id" { "bipa" } else { "j" };
            if !level.id.starts_with(prefix) {
                problems.push(format!("{}: level does not belong to '{}'", level.id, c.language));
            }
            if !ids.insert(level.id.clone()) {
                problems.push(format!("duplicate id {}", level.id));
            }
            for req in &level.requires {
                if !level_ids.contains(req.as_str()) {
                    problems.push(format!("{}: requires unknown level {req}", level.id));
                }
            }
            if level.level_test.id != format!("{}.test", level.id) {
                problems.push(format!("{}: level test id must be {}.test", level.id, level.id));
            }
            for (n, unit) in level.units.iter().enumerate() {
                let expected = format!("{}.u{:02}", level.id, n + 1);
                if unit.id != expected {
                    problems.push(format!("{}: expected unit id {expected}", unit.id));
                }
                if !ids.insert(unit.id.clone()) {
                    problems.push(format!("duplicate id {}", unit.id));
                }
                let unit_grammar: BTreeSet<&str> = unit.grammar.iter().map(String::as_str).collect();
                let mut taught = BTreeSet::new();
                for (m, lesson) in unit.lessons.iter().enumerate() {
                    let expected = format!("{}.l{:02}", unit.id, m + 1);
                    if lesson.id != expected {
                        problems.push(format!("{}: expected lesson id {expected}", lesson.id));
                    }
                    if !ids.insert(lesson.id.clone()) {
                        problems.push(format!("duplicate id {}", lesson.id));
                    }
                    match kabar_core::lesson::pacing::caps(&lesson.kind) {
                        Some(caps) if lesson.grammar.len() > caps.max_grammar => problems.push(format!(
                            "{}: {} grammar points; a {} lesson teaches at most {} (docs/PEDAGOGY.md)",
                            lesson.id,
                            lesson.grammar.len(),
                            lesson.kind,
                            caps.max_grammar
                        )),
                        Some(_) => {}
                        None => {
                            problems.push(format!("{}: unknown lesson kind '{}'", lesson.id, lesson.kind))
                        }
                    }
                    if lesson.minutes > kabar_core::lesson::pacing::MAX_LESSON_SECONDS / 60 {
                        problems.push(format!(
                            "{}: {} minutes; a lesson fits in {} (docs/PEDAGOGY.md)",
                            lesson.id,
                            lesson.minutes,
                            kabar_core::lesson::pacing::MAX_LESSON_SECONDS / 60
                        ));
                    }
                    for g in &lesson.grammar {
                        if !unit_grammar.contains(g.as_str()) {
                            problems.push(format!("{}: grammar {g} is not listed on its unit", lesson.id));
                        }
                        taught.insert(g.as_str());
                    }
                }
                for g in unit_grammar.difference(&taught) {
                    problems.push(format!("{}: grammar {g} is not taught in any lesson", unit.id));
                }
                if unit.lessons.last().is_none_or(|l| l.kind != "review") {
                    problems.push(format!("{}: last lesson must be a review", unit.id));
                }
            }
        }
    }
    report.problems.extend(problems);
}

fn check_grammar(report: &mut Report) {
    let mut problems = Vec::new();

    // First level (in curriculum order) that teaches each grammar id.
    let mut first_level: BTreeMap<&str, &str> = BTreeMap::new();
    for c in &report.curricula {
        for level in &c.levels {
            for unit in &level.units {
                for g in &unit.grammar {
                    first_level.entry(g.as_str()).or_insert(level.id.as_str());
                }
            }
        }
    }

    let mut library = BTreeSet::new();
    for g in &report.grammar {
        if !library.insert(g.id.as_str()) {
            problems.push(format!("duplicate grammar id {}", g.id));
        }
        if !g.id.starts_with(&format!("{}.", g.language)) {
            problems.push(format!("{}: id must start with '{}.'", g.id, g.language));
        }
        match first_level.get(g.id.as_str()) {
            None => problems.push(format!("{}: not used by any unit", g.id)),
            Some(level) if *level != g.level => {
                problems.push(format!("{}: level is {} but it is first taught in {level}", g.id, g.level))
            }
            Some(_) => {}
        }
        if g.language == "jv" && g.review.verified && g.review.status != "approved" {
            problems.push(format!("{}: Javanese stays unverified until approved", g.id));
        }
    }

    for g in &report.grammar {
        for r in g.prerequisites.iter().chain(&g.see_also) {
            if !library.contains(r.as_str()) && !first_level.contains_key(r.as_str()) {
                problems.push(format!("{}: refers to unknown grammar point {r}", g.id));
            }
        }
    }

    for (g, level) in &first_level {
        if GRAMMAR_LIBRARY_LEVELS.contains(level) && !library.contains(g) {
            problems.push(format!("{g}: taught in {level} but missing from content/grammar"));
        }
    }
    report.problems.extend(problems);
}

/// Grammar ids referenced by the curriculum but not yet written, per level.
pub fn planned_grammar(report: &Report) -> BTreeMap<String, Vec<String>> {
    let library: BTreeSet<&str> = report.grammar.iter().map(|g| g.id.as_str()).collect();
    let mut seen = BTreeSet::new();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in &report.curricula {
        for level in &c.levels {
            for unit in &level.units {
                for g in &unit.grammar {
                    if !library.contains(g.as_str()) && seen.insert(g.clone()) {
                        out.entry(level.id.clone()).or_default().push(g.clone());
                    }
                }
            }
        }
    }
    out
}
