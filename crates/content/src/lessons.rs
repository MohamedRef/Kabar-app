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
pub const PERSONA_SCHEMA_TAG: &str = "kabar.persona/v1";
pub const STORY_SCHEMA_TAG: &str = "kabar.story/v1";
pub const RADIO_SCHEMA_TAG: &str = "kabar.radio/v1";

/// Azure neural voices the personas, stories and radio may use (SPEC §15).
pub const VOICES: &[&str] =
    &["id-ID-ArdiNeural", "id-ID-GadisNeural", "jv-ID-DimasNeural", "jv-ID-SitiNeural"];

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
            "listen" => (&["text"], &["audio", "options", "answer", "meaning", "question"]),
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
                if m.contains_key("meaning") {
                    self.localized(&format!("{at}.meaning"), m.get("meaning"), &[], &[]);
                }
                // Comprehension: a question about the text; the answer need not be the text.
                let question = m.contains_key("question");
                if question {
                    self.localized(&format!("{at}.question"), m.get("question"), &[], &[]);
                }
                match m.get("options") {
                    Some(o) if o.as_array().is_some_and(|a| !a.is_empty()) => {
                        let options = self.strings(&format!("{at}.options"), Some(o), 2);
                        self.unique(&format!("{at}.options"), &options);
                        self.index(at, m.get("answer"), options.len());
                        let picked =
                            m.get("answer").and_then(Value::as_u64).and_then(|i| options.get(i as usize));
                        if !question && picked.is_some() && picked != text.as_ref() {
                            self.push(format_args!("{at}: options[answer] must equal text"));
                        }
                    }
                    _ if question => self.push(format_args!("{at}: a comprehension question needs options")),
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

impl Check<'_> {
    fn tag(&mut self, m: &Map<String, Value>, tag: &str) {
        if m.get("schema").and_then(Value::as_str) != Some(tag) {
            self.push(format_args!("schema must be {tag}"));
        }
    }

    fn level(&mut self, at: &str, v: Option<&Value>, index: &Index) {
        if !v.and_then(Value::as_str).is_some_and(|l| index.levels.contains(l)) {
            self.push(format_args!("{at}: must be a curriculum level id"));
        }
    }

    /// A spoken line: `speaker`, `text` and its three meanings, an optional
    /// `note`, and (persona dialogues) an optional inline `correction`.
    fn line(&mut self, at: &str, v: &Value, speakers: &BTreeSet<&str>, corrections: bool) {
        let optional: &[&str] = if corrections { &["note", "correction"] } else { &["note"] };
        self.localized(at, Some(v), &["speaker", "text"], optional);
        let speaker = v.get("speaker").and_then(Value::as_str).unwrap_or_default();
        if !speakers.contains(speaker) {
            self.push(format_args!("{at}.speaker '{speaker}' must be one of {speakers:?}"));
        }
        if v.get("note").is_some() {
            self.localized(&format!("{at}.note"), v.get("note"), &[], &[]);
        }
        if let Some(c) = v.get("correction")
            && let Some(cm) = self.fields(&format!("{at}.correction"), c, &["wrong", "right", "why"], &[])
        {
            self.text(&format!("{at}.correction.wrong"), cm.get("wrong"));
            self.text(&format!("{at}.correction.right"), cm.get("right"));
            self.localized(&format!("{at}.correction.why"), cm.get("why"), &[], &[]);
        }
    }

    fn new_words(&mut self, v: Option<&Value>) {
        for (i, w) in v.and_then(Value::as_array).into_iter().flatten().enumerate() {
            let at = format!("new_words[{i}]");
            self.localized(&at, Some(w), &["word"], &["pos", "arabic"]);
            if w.get("arabic").is_some() {
                self.arabic(&format!("{at}.arabic"), w.get("arabic"));
            }
        }
    }
}

fn array(v: Option<&Value>) -> Vec<Value> {
    v.and_then(Value::as_array).cloned().unwrap_or_default()
}

/// Checks a conversation persona (`content/personas/<id>.yaml`).
pub fn check_persona(file: &str, stem: &str, v: &Value, index: &Index, out: &mut Vec<String>) {
    let mut c = Check { file, out };
    let Some(m) = c.fields(
        "persona",
        v,
        &[
            "schema",
            "id",
            "name",
            "language",
            "voice",
            "home",
            "role",
            "personality",
            "style",
            "openers",
            "example_dialogues",
            "review",
        ],
        &["topics"],
    ) else {
        return;
    };
    c.tag(m, PERSONA_SCHEMA_TAG);
    if m.get("id").and_then(Value::as_str) != Some(stem) {
        c.push("id must equal the file name");
    }
    let language = m.get("language").and_then(Value::as_str).unwrap_or_default();
    if !["id", "jv"].contains(&language) {
        c.push("language must be id or jv");
    }
    if !m.get("voice").and_then(Value::as_str).is_some_and(|x| VOICES.contains(&x)) {
        c.push(format_args!("voice must be one of {VOICES:?}"));
    }
    let name = c.text("name", m.get("name")).unwrap_or_default().to_owned();
    c.text("home", m.get("home"));
    c.localized("role", m.get("role"), &[], &[]);
    c.localized("personality", m.get("personality"), &[], &[]);
    if let Some(style) = m.get("style")
        && let Some(sm) = c.fields("style", style, &["register", "do", "dont", "corrections", "levels"], &[])
    {
        c.localized("style.register", sm.get("register"), &[], &[]);
        c.localized("style.corrections", sm.get("corrections"), &[], &[]);
        c.strings("style.do", sm.get("do"), 3);
        c.strings("style.dont", sm.get("dont"), 3);
        let levels = array(sm.get("levels"));
        if levels.len() < 2 {
            c.push("style.levels needs at least 2 bands");
        }
        for (i, l) in levels.iter().enumerate() {
            let at = format!("style.levels[{i}]");
            if let Some(lm) = c.fields(&at, l, &["level", "guide"], &["vocabulary"]) {
                c.level(&format!("{at}.level"), lm.get("level"), index);
                c.text(&format!("{at}.guide"), lm.get("guide"));
            }
        }
    }
    if m.contains_key("topics") {
        c.strings("topics", m.get("topics"), 1);
    }
    let openers = array(m.get("openers"));
    if openers.len() < 3 {
        c.push("openers needs at least 3");
    }
    for (i, o) in openers.iter().enumerate() {
        c.localized(&format!("openers[{i}]"), Some(o), &["text"], &["level"]);
    }
    let dialogues = array(m.get("example_dialogues"));
    if dialogues.len() < 4 {
        c.push("example_dialogues needs at least 4");
    }
    let speakers = BTreeSet::from([name.as_str(), "learner"]);
    for (i, d) in dialogues.iter().enumerate() {
        let at = format!("example_dialogues[{i}]");
        let Some(dm) = c.fields(&at, d, &["id", "level", "situation", "lines"], &[]) else { continue };
        if dm.get("id").and_then(Value::as_str) != Some(format!("d{:02}", i + 1).as_str()) {
            c.push(format_args!("{at}.id must be d{:02}", i + 1));
        }
        c.level(&format!("{at}.level"), dm.get("level"), index);
        c.localized(&format!("{at}.situation"), dm.get("situation"), &[], &[]);
        let lines = array(dm.get("lines"));
        if lines.len() < 6 {
            c.push(format_args!("{at} needs at least 6 lines"));
        }
        for (j, l) in lines.iter().enumerate() {
            c.line(&format!("{at}.lines[{j}]"), l, &speakers, true);
        }
        if i < 2 && !lines.iter().any(|l| l.get("correction").is_some()) {
            c.push(format_args!("{at}: the first two dialogues must show an inline correction"));
        }
    }
    c.review(m.get("review"), language);
}

/// Checks a Sinetron episode (`content/stories/sinetron/sinetron.eNN.yaml`).
pub fn check_story(file: &str, stem: &str, v: &Value, index: &Index, out: &mut Vec<String>) {
    let mut c = Check { file, out };
    let Some(m) = c.fields(
        "story",
        v,
        &[
            "schema",
            "id",
            "series",
            "episode",
            "language",
            "level",
            "unlock_after",
            "title",
            "summary",
            "cast",
            "scenes",
            "questions",
            "review",
        ],
        &["new_words", "next"],
    ) else {
        return;
    };
    c.tag(m, STORY_SCHEMA_TAG);
    if m.get("series").and_then(Value::as_str) != Some("sinetron") {
        c.push("series must be sinetron");
    }
    let expected = m.get("episode").and_then(Value::as_u64).map(|n| format!("sinetron.e{n:02}"));
    if expected.as_deref() != Some(stem) || m.get("id").and_then(Value::as_str) != Some(stem) {
        c.push("id must be sinetron.eNN (the episode number) and equal the file name");
    }
    let language = m.get("language").and_then(Value::as_str).unwrap_or_default();
    c.level("level", m.get("level"), index);
    if !m.get("unlock_after").and_then(Value::as_str).is_some_and(|u| index.units.contains(u)) {
        c.push("unlock_after must be a curriculum unit id");
    }
    c.localized("title", m.get("title"), &["target"], &[]);
    c.localized("summary", m.get("summary"), &[], &[]);
    if m.contains_key("next") {
        c.localized("next", m.get("next"), &[], &[]);
    }
    let cast = c.strings("cast", m.get("cast"), 2);
    let mut speakers: BTreeSet<&str> = cast.iter().map(String::as_str).collect();
    speakers.insert("Narrator");
    let scenes = array(m.get("scenes"));
    if !(1..=4).contains(&scenes.len()) {
        c.push("an episode has 1-4 scenes");
    }
    let mut lines = 0;
    for (i, s) in scenes.iter().enumerate() {
        let at = format!("scenes[{i}]");
        let Some(sm) = c.fields(&at, s, &["setting", "lines"], &[]) else { continue };
        c.localized(&format!("{at}.setting"), sm.get("setting"), &[], &[]);
        for (j, l) in array(sm.get("lines")).iter().enumerate() {
            c.line(&format!("{at}.lines[{j}]"), l, &speakers, false);
            lines += 1;
        }
    }
    if lines < 8 {
        c.push("an episode needs at least 8 lines");
    }
    c.new_words(m.get("new_words"));
    let questions = array(m.get("questions"));
    if !(3..=6).contains(&questions.len()) {
        c.push("an episode has 3-6 questions");
    }
    for (i, q) in questions.iter().enumerate() {
        if q.get("id").and_then(Value::as_str) != Some(format!("q{:02}", i + 1).as_str()) {
            c.push(format_args!("questions[{i}].id must be q{:02}", i + 1));
        }
        c.exercise(&format!("questions[{i}]"), q, false);
    }
    c.review(m.get("review"), language);
}

/// Checks a Kabar Radio script (`content/radio/<lang>/radio.<unit-id>.yaml`).
pub fn check_radio(file: &str, stem: &str, v: &Value, index: &Index, out: &mut Vec<String>) {
    let mut c = Check { file, out };
    let Some(m) = c.fields(
        "radio",
        v,
        &[
            "schema", "id", "language", "level", "unit", "title", "hosts", "minutes", "segments", "words",
            "review",
        ],
        &[],
    ) else {
        return;
    };
    c.tag(m, RADIO_SCHEMA_TAG);
    let unit = m.get("unit").and_then(Value::as_str).unwrap_or_default();
    if !index.units.contains(unit) {
        c.push("unit must be a curriculum unit id");
    }
    if m.get("id").and_then(Value::as_str) != Some(format!("radio.{unit}").as_str())
        || stem != format!("radio.{unit}")
    {
        c.push("id must be radio.<unit> and equal the file name");
    }
    let language = m.get("language").and_then(Value::as_str).unwrap_or_default();
    c.level("level", m.get("level"), index);
    c.localized("title", m.get("title"), &["target"], &[]);
    let hosts = array(m.get("hosts"));
    if hosts.len() != 2 {
        c.push("a radio episode has exactly 2 hosts");
    }
    let mut names = BTreeSet::new();
    for (i, h) in hosts.iter().enumerate() {
        if let Some(hm) = c.fields(&format!("hosts[{i}]"), h, &["name", "voice"], &[]) {
            names.extend(hm.get("name").and_then(Value::as_str));
            if !hm.get("voice").and_then(Value::as_str).is_some_and(|x| VOICES.contains(&x)) {
                c.push(format_args!("hosts[{i}].voice must be one of {VOICES:?}"));
            }
        }
    }
    if !m.get("minutes").and_then(Value::as_u64).is_some_and(|n| (3..=8).contains(&n)) {
        c.push("minutes must be 3-8");
    }
    let mut kinds = BTreeSet::new();
    let mut words = 0;
    for (i, s) in array(m.get("segments")).iter().enumerate() {
        let at = format!("segments[{i}]");
        let Some(sm) = c.fields(&at, s, &["kind", "lines"], &["title"]) else { continue };
        let kind = sm.get("kind").and_then(Value::as_str).unwrap_or_default().to_owned();
        if !["intro", "dialogue", "word_spotlight", "recap", "outro"].contains(&kind.as_str()) {
            c.push(format_args!("{at}.kind '{kind}' is not a segment kind"));
        }
        for (j, l) in array(sm.get("lines")).iter().enumerate() {
            c.line(&format!("{at}.lines[{j}]"), l, &names, false);
            words += l.get("text").and_then(Value::as_str).map_or(0, |t| t.split_whitespace().count());
        }
        kinds.insert(kind);
    }
    for k in ["intro", "dialogue", "recap", "outro"] {
        if !kinds.contains(k) {
            c.push(format_args!("missing a {k} segment"));
        }
    }
    if words < 250 {
        c.push(format_args!("{words} spoken words is too short for an episode (at least 250)"));
    }
    c.strings("words", m.get("words"), 5);
    c.review(m.get("review"), language);
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
    units: BTreeSet<&'a str>,
    grammar: BTreeSet<&'a str>,
}

impl<'a> Index<'a> {
    pub fn new(curricula: &'a [Curriculum], grammar: impl IntoIterator<Item = &'a str>) -> Self {
        let mut lessons = BTreeMap::new();
        let mut levels = BTreeSet::new();
        let mut units = BTreeSet::new();
        for c in curricula {
            for level in &c.levels {
                levels.insert(level.id.as_str());
                for unit in &level.units {
                    units.insert(unit.id.as_str());
                    for lesson in &unit.lessons {
                        lessons.insert(lesson.id.as_str(), (c.language.as_str(), unit, lesson));
                    }
                }
            }
        }
        Self { lessons, levels, units, grammar: grammar.into_iter().collect() }
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
        &["slides", "dialogue", "new_words", "souvenir", "game"],
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

    // Optional link to a mini-game scene: uu.<scene> (Unggah-Ungguh) or mp.<pair> (Minimal Pairs).
    if let Some(game) = m.get("game") {
        let ok = game.as_str().and_then(|g| g.split_once('.')).is_some_and(|(kind, id)| {
            ["uu", "mp"].contains(&kind)
                && !id.is_empty()
                && id.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        });
        if !ok {
            c.push("game must look like uu.<scene> or mp.<pair>");
        }
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
