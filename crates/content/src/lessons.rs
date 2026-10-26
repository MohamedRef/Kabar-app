//! Checks for lesson files (`content/lessons/<lang>/<lesson-id>.yaml`,
//! `kabar.lesson/v1`, the shape of `kabar_protocol::lesson::LessonContent`)
//! and test files (`content/tests/<lang>/<id>.yaml`, `kabar.test/v1`).
//!
//! Shape is checked strictly (unknown fields are errors, so a typo never
//! silently drops content), then the content rules: ids and titles agree with
//! the curriculum, every exercise has a well-formed reference answer, every
//! unit's last lesson carries its souvenir.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::model::{Curriculum, Lesson, Unit};

pub const LESSON_SCHEMA_TAG: &str = "kabar.lesson/v1";
pub const TEST_SCHEMA_TAG: &str = "kabar.test/v1";

const SKILLS: &[&str] = &["listening", "reading", "writing", "speaking", "vocabulary", "grammar"];

/// Collects problems for one file.
struct Check<'a> {
    file: &'a str,
    out: &'a mut Vec<String>,
}

fn has_arabic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

impl Check<'_> {
    fn push(&mut self, msg: impl std::fmt::Display) {
        self.out.push(format!("{}: {msg}", self.file));
    }

    /// Requires `required` keys and rejects keys outside `required` + `optional`.
    fn fields<'v>(
        &mut self,
        at: &str,
        v: &'v Value,
        required: &[&str],
        optional: &[&str],
    ) -> Option<&'v Map<String, Value>> {
        let Some(map) = v.as_object() else {
            self.push(format_args!("{at}: expected a mapping"));
            return None;
        };
        for k in required {
            if !map.contains_key(*k) {
                self.push(format_args!("{at}: missing '{k}'"));
            }
        }
        for k in map.keys() {
            if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
                self.push(format_args!("{at}: unknown field '{k}'"));
            }
        }
        Some(map)
    }

    fn text<'v>(&mut self, at: &str, v: Option<&'v Value>) -> Option<&'v str> {
        match v.and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => Some(s),
            _ => {
                self.push(format_args!("{at}: expected non-empty text"));
                None
            }
        }
    }

    fn arabic(&mut self, at: &str, v: Option<&Value>) {
        if let Some(s) = self.text(at, v)
            && !has_arabic(s)
        {
            self.push(format_args!("{at}: must contain Arabic script"));
        }
    }

    /// `ar_eg` / `ar` / `en`, plus `extra` required keys (e.g. `text`).
    fn localized(&mut self, at: &str, v: Option<&Value>, extra: &[&str], optional: &[&str]) {
        let Some(v) = v else {
            self.push(format_args!("{at}: missing"));
            return;
        };
        let mut required = vec!["ar_eg", "ar", "en"];
        required.extend_from_slice(extra);
        if let Some(m) = self.fields(at, v, &required, optional) {
            self.arabic(&format!("{at}.ar_eg"), m.get("ar_eg"));
            self.arabic(&format!("{at}.ar"), m.get("ar"));
            self.text(&format!("{at}.en"), m.get("en"));
            for k in extra {
                self.text(&format!("{at}.{k}"), m.get(*k));
            }
        }
    }

    fn strings(&mut self, at: &str, v: Option<&Value>, min: usize) -> Vec<String> {
        let list: Option<Vec<String>> = v.and_then(Value::as_array).and_then(|a| {
            a.iter().map(|x| x.as_str().filter(|s| !s.trim().is_empty()).map(str::to_owned)).collect()
        });
        match list {
            Some(l) if l.len() >= min => l,
            _ => {
                self.push(format_args!("{at}: expected a list of at least {min} non-empty strings"));
                Vec::new()
            }
        }
    }

    fn index(&mut self, at: &str, v: Option<&Value>, len: usize) {
        if !v.and_then(Value::as_u64).is_some_and(|i| (i as usize) < len) {
            self.push(format_args!("{at}: answer must be an index into options"));
        }
    }

    fn unique(&mut self, at: &str, items: &[String]) {
        if items.iter().collect::<BTreeSet<_>>().len() != items.len() {
            self.push(format_args!("{at}: duplicate entries"));
        }
    }

    /// One exercise; returns its `type`.
    fn exercise(&mut self, at: &str, e: &Value, in_test: bool) -> Option<String> {
        let kind = e.get("type").and_then(Value::as_str).unwrap_or_default().to_owned();
        let (required, optional): (&[&str], &[&str]) = match kind.as_str() {
            "multiple_choice" => (&["options", "answer"], &["question", "question_meaning"]),
            "match_pairs" => (&["pairs"], &[]),
            "translate" => (&["direction"], &["source_meaning", "source_text", "answers", "answers_meaning"]),
            "build_sentence" => (&["source", "answer"], &["alternatives", "distractors"]),
            "listen" => (&["text"], &["audio", "options", "answer"]),
            "fill_blank" => (&["sentence", "meaning", "answers"], &["options"]),
            "dictation" => (&["text", "meaning"], &["audio"]),
            "speaking" => (&["text", "meaning"], &[]),
            "writing_task" | "speaking_task" if in_test => (&["prompt", "rubric", "sample"], &[]),
            other => {
                self.push(format_args!("{at}: unknown exercise type '{other}'"));
                return None;
            }
        };
        let mut req = vec!["id", "type"];
        req.extend_from_slice(required);
        let mut opt = vec!["explain", "words"];
        if in_test {
            opt.extend_from_slice(&["points", "level", "skill"]);
        }
        opt.extend_from_slice(optional);
        let m = self.fields(at, e, &req, &opt)?;
        if m.contains_key("explain") {
            self.localized(&format!("{at}.explain"), m.get("explain"), &[], &[]);
        }
        if m.contains_key("words") {
            self.strings(&format!("{at}.words"), m.get("words"), 0);
        }

        match kind.as_str() {
            "multiple_choice" => {
                let has_question =
                    m.get("question").and_then(Value::as_str).is_some_and(|q| !q.trim().is_empty());
                match m.get("question_meaning") {
                    Some(q) => self.localized(&format!("{at}.question_meaning"), Some(q), &[], &[]),
                    None if !has_question => {
                        self.push(format_args!("{at}: needs question or question_meaning"))
                    }
                    None => {}
                }
                let options = self.strings(&format!("{at}.options"), m.get("options"), 2);
                self.unique(&format!("{at}.options"), &options);
                self.index(at, m.get("answer"), options.len());
            }
            "match_pairs" => {
                let pairs = m.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default();
                if pairs.len() < 3 {
                    self.push(format_args!("{at}.pairs: need at least 3"));
                }
                for (i, p) in pairs.iter().enumerate() {
                    self.localized(&format!("{at}.pairs[{i}]"), Some(p), &["text"], &[]);
                }
                let texts: Vec<String> =
                    pairs.iter().filter_map(|p| p.get("text")?.as_str().map(str::to_owned)).collect();
                self.unique(&format!("{at}.pairs"), &texts);
            }
            "translate" => match m.get("direction").and_then(Value::as_str) {
                Some("to_target") => {
                    self.localized(&format!("{at}.source_meaning"), m.get("source_meaning"), &[], &[]);
                    self.strings(&format!("{at}.answers"), m.get("answers"), 1);
                    for k in ["source_text", "answers_meaning"] {
                        if m.contains_key(k) {
                            self.push(format_args!("{at}: to_target must not have {k}"));
                        }
                    }
                }
                Some("from_target") => {
                    self.text(&format!("{at}.source_text"), m.get("source_text"));
                    let am = m.get("answers_meaning").cloned().unwrap_or(Value::Null);
                    if let Some(am) =
                        self.fields(&format!("{at}.answers_meaning"), &am, &["ar_eg", "ar", "en"], &[])
                    {
                        for k in ["ar_eg", "ar", "en"] {
                            let list = self.strings(&format!("{at}.answers_meaning.{k}"), am.get(k), 1);
                            if k != "en" && list.iter().any(|s| !has_arabic(s)) {
                                self.push(format_args!("{at}.answers_meaning.{k}: answers must be Arabic"));
                            }
                        }
                    }
                    for k in ["source_meaning", "answers"] {
                        if m.contains_key(k) {
                            self.push(format_args!("{at}: from_target must not have {k}"));
                        }
                    }
                }
                _ => self.push(format_args!("{at}: direction must be to_target or from_target")),
            },
            "build_sentence" => {
                self.localized(&format!("{at}.source"), m.get("source"), &[], &[]);
                if let Some(answer) = self.text(&format!("{at}.answer"), m.get("answer")) {
                    let tiles = tiles(answer);
                    if tiles.len() < 3 {
                        self.push(format_args!("{at}: answer should have at least 3 words"));
                    }
                    let alternatives = self.strings(
                        &format!("{at}.alternatives"),
                        m.get("alternatives").or(Some(&Value::Array(vec![]))),
                        0,
                    );
                    for alt in alternatives {
                        let (mut a, mut b) = (tiles.clone(), self::tiles(&alt));
                        a.sort();
                        b.sort();
                        if a != b {
                            self.push(format_args!("{at}: alternative '{alt}' must use the answer's tiles"));
                        }
                    }
                    let distractors = self.strings(
                        &format!("{at}.distractors"),
                        m.get("distractors").or(Some(&Value::Array(vec![]))),
                        0,
                    );
                    for d in distractors {
                        if tiles.contains(&d.to_lowercase()) {
                            self.push(format_args!("{at}: distractor '{d}' is part of the answer"));
                        }
                    }
                }
            }
            "listen" => {
                let text = self.text(&format!("{at}.text"), m.get("text")).map(str::to_owned);
                match m.get("options") {
                    Some(o) if o.as_array().is_some_and(|a| !a.is_empty()) => {
                        let options = self.strings(&format!("{at}.options"), Some(o), 2);
                        self.index(at, m.get("answer"), options.len());
                        let picked =
                            m.get("answer").and_then(Value::as_u64).and_then(|i| options.get(i as usize));
                        if picked.is_some() && picked != text.as_ref() {
                            self.push(format_args!("{at}: options[answer] must equal text"));
                        }
                    }
                    _ if m.contains_key("answer") => self.push(format_args!("{at}: answer without options")),
                    _ => {}
                }
            }
            "fill_blank" => {
                if let Some(s) = self.text(&format!("{at}.sentence"), m.get("sentence"))
                    && s.matches("___").count() != 1
                {
                    self.push(format_args!("{at}.sentence: must contain exactly one ___"));
                }
                self.localized(&format!("{at}.meaning"), m.get("meaning"), &[], &[]);
                let answers = self.strings(&format!("{at}.answers"), m.get("answers"), 1);
                if m.get("options").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
                    let options = self.strings(&format!("{at}.options"), m.get("options"), 2);
                    self.unique(&format!("{at}.options"), &options);
                    if !answers.iter().all(|a| options.contains(a)) {
                        self.push(format_args!("{at}: answers must be among options"));
                    }
                }
            }
            "dictation" | "speaking" => {
                self.text(&format!("{at}.text"), m.get("text"));
                self.localized(&format!("{at}.meaning"), m.get("meaning"), &[], &[]);
            }
            _ => {
                self.localized(&format!("{at}.prompt"), m.get("prompt"), &[], &[]);
                self.strings(&format!("{at}.rubric"), m.get("rubric"), 2);
                self.text(&format!("{at}.sample"), m.get("sample"));
            }
        }
        Some(kind)
    }

    fn review(&mut self, v: Option<&Value>, language: &str) {
        let v = v.cloned().unwrap_or(Value::Null);
        if let Some(r) = self.fields("review", &v, &["status", "verified"], &["notes"]) {
            let status = r.get("status").and_then(Value::as_str).unwrap_or_default();
            if !["draft", "approved", "needs-changes"].contains(&status) {
                self.push("review.status must be draft, approved or needs-changes");
            }
            let verified = r.get("verified").and_then(Value::as_bool);
            if verified.is_none() {
                self.push("review.verified must be true or false");
            }
            if language == "jv" && verified == Some(true) && status != "approved" {
                self.push("Javanese stays unverified until approved");
            }
        }
    }
}

/// Lower-cased words of a sentence, punctuation stripped (build-sentence tiles).
fn tiles(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-').to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Curriculum lookups the lesson checks need.
pub struct Index<'a> {
    lessons: BTreeMap<&'a str, (&'a str, &'a Unit, &'a Lesson)>,
    levels: BTreeSet<&'a str>,
    grammar: BTreeSet<&'a str>,
}

impl<'a> Index<'a> {
    pub fn new(curricula: &'a [Curriculum], grammar: impl IntoIterator<Item = &'a str>) -> Self {
        let mut lessons = BTreeMap::new();
        let mut levels = BTreeSet::new();
        for c in curricula {
            for level in &c.levels {
                levels.insert(level.id.as_str());
                for unit in &level.units {
                    for lesson in &unit.lessons {
                        lessons.insert(lesson.id.as_str(), (c.language.as_str(), unit, lesson));
                    }
                }
            }
        }
        Self { lessons, levels, grammar: grammar.into_iter().collect() }
    }

    /// Ids of all curriculum lessons for `level`.
    pub fn lessons_of(&self, level: &str) -> Vec<&'a str> {
        let prefix = format!("{level}.");
        self.lessons.keys().copied().filter(|id| id.starts_with(&prefix)).collect()
    }
}

/// Checks one lesson file. `stem` is the file name without `.yaml`, `dir` the
/// name of its parent directory (the language).
pub fn check_lesson(file: &str, stem: &str, dir: &str, v: &Value, index: &Index, out: &mut Vec<String>) {
    let mut c = Check { file, out };
    let Some(m) = c.fields(
        "lesson",
        v,
        &["schema", "id", "language", "title", "can_do", "exercises", "review"],
        &["slides", "dialogue", "new_words", "souvenir"],
    ) else {
        return;
    };
    if m.get("schema").and_then(Value::as_str) != Some(LESSON_SCHEMA_TAG) {
        c.push(format_args!("schema must be {LESSON_SCHEMA_TAG}"));
    }
    if m.get("id").and_then(Value::as_str) != Some(stem) {
        c.push("id must equal the file name");
    }
    let Some(&(language, unit, lesson)) = index.lessons.get(stem) else {
        c.push("lesson id is not in the curriculum");
        return;
    };
    if m.get("language").and_then(Value::as_str) != Some(language) || dir != language {
        c.push(format_args!(
            "language must be '{language}' and the file must live in content/lessons/{language}/"
        ));
    }
    if m.get("title").and_then(Value::as_str) != Some(lesson.title.as_str()) {
        c.push(format_args!("title must equal the curriculum title '{}'", lesson.title));
    }
    c.localized("can_do", m.get("can_do"), &[], &[]);

    // Slides.
    let slides = m.get("slides").and_then(Value::as_array).cloned().unwrap_or_default();
    let learn = lesson.kind == "learn";
    let (lo, hi) = if learn { (3, 6) } else { (0, 2) };
    if !(lo..=hi).contains(&slides.len()) {
        c.push(format_args!("a {} lesson needs {lo}-{hi} slides", lesson.kind));
    }
    let allowed: BTreeSet<&str> =
        if learn { lesson.grammar.iter() } else { unit.grammar.iter() }.map(String::as_str).collect();
    let mut taught = BTreeSet::new();
    for (i, s) in slides.iter().enumerate() {
        let at = format!("slides[{i}]");
        let Some(sm) = c.fields(&at, s, &["body"], &["title", "examples", "grammar"]) else { continue };
        c.localized(&format!("{at}.body"), sm.get("body"), &[], &[]);
        if sm.contains_key("title") {
            c.text(&format!("{at}.title"), sm.get("title"));
        }
        for (j, ex) in sm.get("examples").and_then(Value::as_array).into_iter().flatten().enumerate() {
            c.localized(&format!("{at}.examples[{j}]"), Some(ex), &["text"], &[]);
        }
        if let Some(g) = sm.get("grammar").and_then(Value::as_str) {
            if !index.grammar.contains(g) {
                c.push(format_args!("{at}.grammar: {g} is not in the grammar library"));
            }
            if !allowed.contains(g) {
                c.push(format_args!("{at}.grammar: {g} is not taught by this lesson's unit"));
            }
            taught.insert(g);
        }
    }
    if learn {
        for g in allowed.difference(&taught) {
            c.push(format_args!("no slide teaches the lesson's grammar point {g}"));
        }
    }

    // Exercises.
    let exercises = m.get("exercises").and_then(Value::as_array).cloned().unwrap_or_default();
    if !(8..=15).contains(&exercises.len()) {
        c.push("a lesson needs 8-15 exercises");
    }
    let mut kinds = BTreeSet::new();
    for (i, e) in exercises.iter().enumerate() {
        let id = format!("e{:02}", i + 1);
        if e.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            c.push(format_args!("exercises[{i}].id must be {id}"));
        }
        kinds.extend(c.exercise(&format!("exercises[{i}]"), e, false));
    }
    if kinds.len() < 4 {
        c.push("use at least 4 exercise types");
    }

    // Dialogue, new words, souvenir, review.
    match m.get("dialogue") {
        None => c.push("missing dialogue"),
        Some(d) => {
            if let Some(dm) = c.fields("dialogue", d, &["lines"], &["title"]) {
                let lines = dm.get("lines").and_then(Value::as_array).cloned().unwrap_or_default();
                if lines.len() < 4 {
                    c.push("dialogue needs at least 4 lines");
                }
                for (i, l) in lines.iter().enumerate() {
                    c.localized(&format!("dialogue.lines[{i}]"), Some(l), &["speaker", "text"], &[]);
                }
            }
        }
    }
    let words = m.get("new_words").and_then(Value::as_array).cloned().unwrap_or_default();
    if learn && words.len() < 3 {
        c.push("a learn lesson needs at least 3 new_words");
    }
    for (i, w) in words.iter().enumerate() {
        let at = format!("new_words[{i}]");
        c.localized(&at, Some(w), &["word"], &["pos", "arabic"]);
        if w.get("arabic").is_some() {
            c.arabic(&format!("{at}.arabic"), w.get("arabic"));
        }
    }
    let last = unit.lessons.last().is_some_and(|l| l.id == lesson.id);
    match (last, m.get("souvenir")) {
        (true, None) => c.push("the unit's last lesson must carry the unit souvenir"),
        (false, Some(_)) => c.push("only the unit's last lesson carries a souvenir"),
        (true, Some(s)) => {
            if let Some(sm) = c.fields("souvenir", s, &["id", "title", "note"], &[]) {
                let id = format!("{}.souvenir", unit.id);
                if sm.get("id").and_then(Value::as_str) != Some(id.as_str()) {
                    c.push(format_args!("souvenir.id must be {id}"));
                }
                c.text("souvenir.title", sm.get("title"));
                c.localized("souvenir.note", sm.get("note"), &[], &[]);
            }
        }
        (false, None) => {}
    }
    c.review(m.get("review"), language);
}

/// Checks one level or placement test file.
pub fn check_test(file: &str, stem: &str, v: &Value, index: &Index, out: &mut Vec<String>) {
    let mut c = Check { file, out };
    let Some(m) = c.fields(
        "test",
        v,
        &["schema", "id", "language", "kind", "title", "minutes", "sections", "review"],
        &["pass_mark", "intro", "scoring"],
    ) else {
        return;
    };
    if m.get("schema").and_then(Value::as_str) != Some(TEST_SCHEMA_TAG) {
        c.push(format_args!("schema must be {TEST_SCHEMA_TAG}"));
    }
    if m.get("id").and_then(Value::as_str) != Some(stem) {
        c.push("id must equal the file name");
    }
    let language = m.get("language").and_then(Value::as_str).unwrap_or_default();
    if !["id", "jv"].contains(&language) {
        c.push("language must be id or jv");
    }
    let placement = match m.get("kind").and_then(Value::as_str) {
        Some("level") => {
            let level = stem.strip_suffix(".test").unwrap_or_default();
            if !index.levels.contains(level) {
                c.push("a level test must be named <level>.test");
            }
            false
        }
        Some("placement") => true,
        _ => {
            c.push("kind must be level or placement");
            false
        }
    };
    c.localized("title", m.get("title"), &[], &[]);
    if m.contains_key("intro") {
        c.localized("intro", m.get("intro"), &[], &[]);
    }
    if !m.get("minutes").and_then(Value::as_u64).is_some_and(|n| (5..=90).contains(&n)) {
        c.push("minutes must be 5-90");
    }
    let sections = m.get("sections").and_then(Value::as_array).cloned().unwrap_or_default();
    if sections.is_empty() {
        c.push("a test needs sections");
    }
    let mut ids = BTreeSet::new();
    for (i, s) in sections.iter().enumerate() {
        let at = format!("sections[{i}]");
        let Some(sm) = c.fields(&at, s, &["id", "skill", "title", "items"], &["level", "instructions"])
        else {
            continue;
        };
        if !sm.get("skill").and_then(Value::as_str).is_some_and(|k| SKILLS.contains(&k)) {
            c.push(format_args!("{at}.skill must be one of {SKILLS:?}"));
        }
        c.localized(&format!("{at}.title"), sm.get("title"), &[], &[]);
        if sm.contains_key("instructions") {
            c.localized(&format!("{at}.instructions"), sm.get("instructions"), &[], &[]);
        }
        let section_level = sm.get("level").and_then(Value::as_str);
        for (j, item) in sm.get("items").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let at = format!("{at}.items[{j}]");
            let id = item.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            if id.is_empty() || !ids.insert(id.clone()) {
                c.push(format_args!("{at}: item ids must be present and unique ('{id}')"));
            }
            c.exercise(&at, item, true);
            let level = item.get("level").and_then(Value::as_str).or(section_level);
            if placement && !level.is_some_and(|l| index.levels.contains(l)) {
                c.push(format_args!("{at}: placement items need a level (on the item or its section)"));
            }
        }
    }
    c.review(m.get("review"), language);
}
