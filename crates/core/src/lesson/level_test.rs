//! Level tests ("Take a flight") and the placement test (`kabar.test/v1`,
//! SPEC §6, §10): parsing, validation and the [`TestRunner`] step machine.
//!
//! Closed items reuse the lesson grader. Open tasks (writing/speaking) and
//! open translations that match no reference are self-assessed at the end
//! (none / partly / fully) until the AI `grade_answer` task grades them.
//! Nothing is ever locked: a failed level test only says what to practise.

use std::collections::{BTreeMap, HashSet};

use kabar_protocol::lesson::{Exercise, ExerciseKind};
use kabar_protocol::test::{
    PlacementResult, SectionScore, TEST_SCHEMA, TestContent, TestFinished, TestItem, TestItemKind, TestKind,
    TestSection, TestSkill,
};

use super::{Answer, Grade, Issue, ParseError, Validator, grade, pair_order, seed_of, tiles};
use crate::locale::UiLanguage;

/// A placement section scored below this is where the journey starts.
pub const PLACEMENT_THRESHOLD: f32 = 0.7;

/// Parses a `content/tests/<lang>/<id>.yaml` file. Run [`validate_test`] next.
pub fn parse_test(yaml: &str) -> Result<TestContent, ParseError> {
    serde_yaml_ng::from_str(yaml).map_err(|e| ParseError(e.to_string()))
}

/// Shape rules serde can't check. Empty = valid.
pub fn validate_test(t: &TestContent) -> Vec<Issue> {
    let mut v = Validator::default();
    if t.schema != TEST_SCHEMA {
        v.issue("schema", format!("expected {TEST_SCHEMA}, found {:?}", t.schema));
    }
    if t.id.trim().is_empty() {
        v.issue("id", "empty");
    }
    if !matches!(t.language.as_str(), "id" | "jv") {
        v.issue("language", format!("must be id or jv, found {:?}", t.language));
    }
    v.localized("title", &t.title);
    if let Some(i) = &t.intro {
        v.localized("intro", i);
    }
    if t.minutes == 0 {
        v.issue("minutes", "must be at least 1");
    }
    match (t.kind, t.pass_mark) {
        (TestKind::Level, Some(m)) if m > 0.0 && m <= 1.0 => {}
        (TestKind::Level, _) => v.issue("pass_mark", "a level test needs a pass mark in (0, 1]"),
        (TestKind::Placement, _) => {}
    }
    if t.sections.is_empty() {
        v.issue("sections", "a test needs sections");
    }
    let mut ids = HashSet::new();
    for (i, s) in t.sections.iter().enumerate() {
        let sp = format!("sections[{i}] ({})", s.id);
        v.localized(&format!("{sp}.title"), &s.title);
        if let Some(ins) = &s.instructions {
            v.localized(&format!("{sp}.instructions"), ins);
        }
        if s.items.is_empty() {
            v.issue(&sp, "a section needs items");
        }
        if t.kind == TestKind::Placement && s.level.is_none() {
            v.issue(&sp, "placement sections need a level");
        }
        for (j, item) in s.items.iter().enumerate() {
            let p = format!("{sp}.items[{j}] ({})", item.id);
            if item.id.trim().is_empty() {
                v.issue(&p, "empty id");
            } else if !ids.insert(item.id.as_str()) {
                v.issue(&p, "duplicate id");
            }
            if !item.points().is_finite() || item.points() <= 0.0 {
                v.issue(&p, "points must be positive");
            }
            if let Some(e) = &item.explain {
                v.localized(&format!("{p}.explain"), e);
            }
            match &item.kind {
                TestItemKind::Exercise(kind) => v.check_exercise(&p, kind),
                TestItemKind::WritingTask(task) | TestItemKind::SpeakingTask(task) => {
                    v.localized(&format!("{p}.prompt"), &task.prompt);
                    if task.rubric.is_empty() || task.rubric.iter().any(|r| r.trim().is_empty()) {
                        v.issue(&p, "an open task needs a rubric of non-empty criteria");
                    }
                    if task.sample.trim().is_empty() {
                        v.issue(&p, "an open task needs a sample answer");
                    }
                }
            }
        }
    }
    if !matches!(t.review.status.as_str(), "draft" | "approved" | "needs-changes") {
        v.issue("review.status", format!("unknown status {:?}", t.review.status));
    }
    if t.review.verified && t.review.status != "approved" {
        v.issue("review.verified", "only approved content can be verified");
    }
    v.issues
}

/// The skill an item counts toward (its own, else its section's).
pub fn item_skill(section: &TestSection, item: &TestItem) -> TestSkill {
    item.skill.unwrap_or(section.skill)
}

/// The level after `level` (`bipa4` → `bipa5`, `j2` → `j3`).
pub fn next_level(level: &str) -> String {
    let digits = level.len() - level.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (prefix, n) = level.split_at(level.len() - digits);
    match n.parse::<u32>() {
        Ok(n) => format!("{prefix}{}", n + 1),
        Err(_) => level.to_owned(),
    }
}

/// How the learner rates an open answer against the rubric and sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelfGrade {
    NotYet,
    Partly,
    Fully,
}

impl SelfGrade {
    pub fn share(self) -> f32 {
        match self {
            Self::NotYet => 0.0,
            Self::Partly => 0.5,
            Self::Fully => 1.0,
        }
    }
}

/// One screen of the test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestStep {
    Intro,
    Item {
        section: usize,
        item: usize,
    },
    /// Open answers waiting for the learner's honest self-assessment.
    SelfCheck,
    Results,
}

/// What happened to one item.
#[derive(Clone, Debug, PartialEq)]
pub enum Scored {
    /// Points earned out of the item's points.
    Earned(f32),
    /// Needs self-assessment; holds what the learner wrote (may be empty for
    /// a spoken task).
    Pending { answer: String },
    /// Not scored: speaking practice, or "can't listen now".
    Unscored,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemRecord {
    pub section: usize,
    pub item: usize,
    pub id: String,
    pub skill: TestSkill,
    /// The curriculum unit the item tests, when the content says.
    pub unit: Option<String>,
    pub points: f32,
    pub scored: Scored,
    /// The learner's own grade, for items they assessed.
    pub self_grade: Option<SelfGrade>,
}

impl ItemRecord {
    /// (earned, available); pending and unscored items count for nothing.
    fn tally(&self) -> (f32, f32) {
        match self.scored {
            Scored::Earned(e) => (e, self.points),
            _ => (0.0, 0.0),
        }
    }

    pub fn missed(&self) -> bool {
        matches!(self.scored, Scored::Earned(e) if e + 1e-4 < self.points)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tally {
    pub earned: f32,
    pub available: f32,
}

impl Tally {
    /// 0–1, or None when nothing here was scored.
    pub fn share(&self) -> Option<f32> {
        (self.available > 0.0).then(|| (self.earned / self.available).clamp(0.0, 1.0))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestResults {
    pub total: Tally,
    /// Level tests: whether the share reached the pass mark.
    pub passed: Option<bool>,
    /// Sections taken, in order.
    pub sections: Vec<(usize, Tally)>,
    /// Skills in [`TestSkill::ALL`] order, only those the test scored.
    pub skills: Vec<(TestSkill, Tally)>,
    /// Records of items not (fully) right, in test order.
    pub missed: Vec<ItemRecord>,
    /// The units behind the missed points, most missed first (ties in
    /// route order), with the points missed in each.
    pub weak_units: Vec<(String, f32)>,
    /// Items not scored because the learner couldn't listen.
    pub skipped: usize,
    /// Placement: where the journey starts.
    pub start_level: Option<String>,
    /// Placement: the first unit of `start_level` with missed points, when
    /// the items are tagged with units.
    pub start_unit: Option<String>,
    pub duration_ms: u64,
}

#[derive(Clone, Debug)]
pub struct TestRunner {
    test: TestContent,
    seed: u64,
    step: TestStep,
    records: Vec<ItemRecord>,
    elapsed_ms: u64,
    /// Placement: the section the test stopped at.
    stopped_at: Option<usize>,
}

impl TestRunner {
    pub fn new(test: TestContent) -> Self {
        let seed = seed_of(&test.id);
        Self { test, seed, step: TestStep::Intro, records: Vec::new(), elapsed_ms: 0, stopped_at: None }
    }

    pub fn test(&self) -> &TestContent {
        &self.test
    }

    pub fn step(&self) -> TestStep {
        self.step
    }

    pub fn records(&self) -> &[ItemRecord] {
        &self.records
    }

    pub fn add_elapsed(&mut self, ms: u64) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(ms);
    }

    pub fn section(&self) -> Option<&TestSection> {
        match self.step {
            TestStep::Item { section, .. } => self.test.sections.get(section),
            _ => None,
        }
    }

    pub fn item(&self) -> Option<&TestItem> {
        match self.step {
            TestStep::Item { section, item } => self.test.sections.get(section)?.items.get(item),
            _ => None,
        }
    }

    /// The current item as a lesson exercise (closed items only).
    pub fn exercise(&self) -> Option<Exercise> {
        self.item().and_then(TestItem::exercise)
    }

    pub fn tiles(&self, ex: &Exercise) -> Vec<String> {
        tiles(ex, self.seed)
    }

    pub fn pair_order(&self, ex: &Exercise) -> Vec<usize> {
        pair_order(ex, self.seed)
    }

    /// Intro → first item.
    pub fn start(&mut self) {
        if self.step == TestStep::Intro {
            self.step = self.first_item_from(0).unwrap_or(TestStep::Results);
        }
    }

    fn first_item_from(&self, section: usize) -> Option<TestStep> {
        (section..self.test.sections.len())
            .find(|&s| !self.test.sections[s].items.is_empty())
            .map(|section| TestStep::Item { section, item: 0 })
    }

    /// Answers the current closed item and moves on (no feedback in a test).
    pub fn answer(&mut self, answer: &Answer, lang: UiLanguage) {
        let Some(ex) = self.exercise() else { return };
        let points = self.item().map_or(1.0, TestItem::points);
        let scored = match (&ex.kind, answer) {
            (ExerciseKind::MatchPairs { pairs }, Answer::Matched { missed }) => {
                let n = pairs.len().max(1) as f32;
                let wrong = missed.iter().collect::<HashSet<_>>().len().min(pairs.len()) as f32;
                Scored::Earned(points * (n - wrong) / n)
            }
            _ => match grade(&ex, answer, lang, &self.test.language) {
                Grade::Correct | Grade::Almost { .. } => Scored::Earned(points),
                Grade::Wrong => Scored::Earned(0.0),
                Grade::NeedsReview => Scored::Pending {
                    answer: match answer {
                        Answer::Text(t) => t.clone(),
                        _ => String::new(),
                    },
                },
                Grade::Practiced => Scored::Unscored,
            },
        };
        self.record(scored);
    }

    /// Answers the current open task (the written text, or empty after
    /// speaking) and moves on; it is self-assessed at the end.
    pub fn answer_open(&mut self, text: &str) {
        if self.item().and_then(TestItem::open_task).is_some() {
            self.record(Scored::Pending { answer: text.to_owned() });
        }
    }

    /// "Can't listen now": not scored either way.
    pub fn skip(&mut self) {
        if self.item().is_some() {
            self.record(Scored::Unscored);
        }
    }

    fn record(&mut self, scored: Scored) {
        let TestStep::Item { section, item } = self.step else { return };
        let s = &self.test.sections[section];
        let it = &s.items[item];
        self.records.retain(|r| r.id != it.id);
        self.records.push(ItemRecord {
            section,
            item,
            id: it.id.clone(),
            skill: item_skill(s, it),
            unit: it.unit.clone(),
            points: it.points(),
            scored,
            self_grade: None,
        });
        self.step = self.next_step(section, item);
    }

    fn next_step(&mut self, section: usize, item: usize) -> TestStep {
        if item + 1 < self.test.sections[section].items.len() {
            return TestStep::Item { section, item: item + 1 };
        }
        if self.test.kind == TestKind::Placement
            && self.section_tally(section).share().is_some_and(|s| s < PLACEMENT_THRESHOLD)
        {
            self.stopped_at = Some(section);
            return self.after_items();
        }
        self.first_item_from(section + 1).unwrap_or_else(|| self.after_items())
    }

    fn after_items(&self) -> TestStep {
        if self.open_answers().next().is_some() { TestStep::SelfCheck } else { TestStep::Results }
    }

    /// Open answers the learner assesses (waiting, or already assessed).
    pub fn open_answers(&self) -> impl Iterator<Item = &ItemRecord> {
        self.records.iter().filter(|r| matches!(r.scored, Scored::Pending { .. }) || r.self_grade.is_some())
    }

    /// The learner's honest grade for an open answer (can be changed until
    /// the results are shown).
    pub fn self_assess(&mut self, id: &str, grade: SelfGrade) {
        if self.step != TestStep::SelfCheck {
            return;
        }
        if let Some(r) =
            self.records.iter_mut().find(|r| r.id == id && matches!(r.scored, Scored::Pending { .. }))
        {
            r.self_grade = Some(grade);
        }
    }

    /// Whether every open answer has a self-grade.
    pub fn self_check_done(&self) -> bool {
        self.open_answers().all(|r| r.self_grade.is_some())
    }

    /// SelfCheck → Results, once every open answer is assessed.
    pub fn finish_self_check(&mut self) {
        if self.step == TestStep::SelfCheck && self.self_check_done() {
            for r in &mut self.records {
                if let (Some(g), Scored::Pending { .. }) = (r.self_grade, &r.scored) {
                    r.scored = Scored::Earned(r.points * g.share());
                }
            }
            self.step = TestStep::Results;
        }
    }

    /// What the learner wrote for a pending item.
    pub fn pending_answer(&self, id: &str) -> Option<&str> {
        self.records.iter().find(|r| r.id == id).and_then(|r| match &r.scored {
            Scored::Pending { answer } => Some(answer.as_str()),
            _ => None,
        })
    }

    fn section_tally(&self, section: usize) -> Tally {
        tally(self.records.iter().filter(|r| r.section == section))
    }

    /// 0.0–1.0 for the progress bar (placement may finish early).
    pub fn fraction(&self) -> f32 {
        let total: usize = self.test.sections.iter().map(|s| s.items.len()).sum();
        match self.step {
            TestStep::Intro => 0.0,
            TestStep::Item { section, item } => {
                let before: usize = self.test.sections[..section].iter().map(|s| s.items.len()).sum();
                (before + item) as f32 / total.max(1) as f32
            }
            TestStep::SelfCheck | TestStep::Results => 1.0,
        }
    }

    pub fn results(&self) -> TestResults {
        let total = tally(self.records.iter());
        let passed = match (self.test.kind, self.test.pass_mark) {
            (TestKind::Level, Some(mark)) => Some(total.share().is_some_and(|s| s + 1e-4 >= mark)),
            _ => None,
        };
        let mut sections: Vec<(usize, Tally)> = Vec::new();
        for r in &self.records {
            if !sections.iter().any(|(s, _)| *s == r.section) {
                sections.push((r.section, self.section_tally(r.section)));
            }
        }
        let mut by_skill: BTreeMap<TestSkill, Vec<&ItemRecord>> = BTreeMap::new();
        for r in &self.records {
            by_skill.entry(r.skill).or_default().push(r);
        }
        let skills = TestSkill::ALL
            .iter()
            .filter_map(|s| {
                let t = tally(by_skill.get(s)?.iter().copied());
                (t.available > 0.0).then_some((*s, t))
            })
            .collect();
        let start_level = (self.test.kind == TestKind::Placement).then(|| self.start_level()).flatten();
        let missed: Vec<ItemRecord> = self.records.iter().filter(|r| r.missed()).cloned().collect();
        let mut by_unit: Vec<(String, f32)> = Vec::new();
        for r in &missed {
            let (Some(unit), &Scored::Earned(e)) = (&r.unit, &r.scored) else { continue };
            match by_unit.iter_mut().find(|(u, _)| u == unit) {
                Some((_, m)) => *m += r.points - e,
                None => by_unit.push((unit.clone(), r.points - e)),
            }
        }
        let start_unit = start_level.as_ref().and_then(|level| {
            by_unit
                .iter()
                .map(|(u, _)| u)
                .filter(|u| u.split_once('.').is_some_and(|(l, _)| l == level))
                .min_by(|a, b| unit_order(a).cmp(&unit_order(b)))
                .cloned()
        });
        let mut weak_units = by_unit;
        weak_units.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| unit_order(&a.0).cmp(&unit_order(&b.0))));
        TestResults {
            total,
            passed,
            sections,
            skills,
            missed,
            weak_units,
            skipped: self
                .records
                .iter()
                .filter(|r| {
                    r.scored == Scored::Unscored
                        && !matches!(
                            self.test.sections[r.section].items[r.item].kind,
                            TestItemKind::Exercise(ExerciseKind::Speaking { .. })
                        )
                })
                .count(),
            start_level,
            start_unit,
            duration_ms: self.elapsed_ms,
        }
    }

    /// Placement: the level of the first section below the threshold, else
    /// the level after the last section.
    fn start_level(&self) -> Option<String> {
        let level = |s: usize| {
            let sec = &self.test.sections[s];
            sec.level.clone().or_else(|| sec.items.first().and_then(|i| i.level.clone()))
        };
        if let Some(s) = self.stopped_at {
            return level(s);
        }
        let last = self.test.sections.len().checked_sub(1)?;
        level(last).map(|l| next_level(&l))
    }

    /// The `placed` event payload (placement tests, once at Results).
    pub fn placement(&self) -> Option<PlacementResult> {
        let r = self.results();
        Some(PlacementResult {
            start_level: r.start_level?,
            start_unit: r.start_unit,
            sections: r
                .sections
                .iter()
                .map(|(s, t)| SectionScore {
                    id: self.test.sections[*s].id.clone(),
                    score: t.share().unwrap_or(1.0),
                })
                .collect(),
        })
    }

    /// The `test_finished` event payload.
    pub fn finished(&self) -> TestFinished {
        let r = self.results();
        TestFinished {
            score: r.total.share().unwrap_or(0.0),
            passed: r.passed.unwrap_or(false),
            duration_ms: r.duration_ms,
            skills: r.skills.iter().filter_map(|(s, t)| Some((*s, t.share()?))).collect(),
            missed: r.missed.iter().map(|m| m.id.clone()).collect(),
        }
    }
}

/// Sort key for unit ids in route order: `bipa2.u10` after `bipa2.u09`.
fn unit_order(unit: &str) -> (String, u32) {
    let (level, n) = unit.rsplit_once(".u").unwrap_or((unit, ""));
    (level.to_owned(), n.parse().unwrap_or(0))
}

fn tally<'a>(records: impl Iterator<Item = &'a ItemRecord>) -> Tally {
    let (earned, available) =
        records.map(ItemRecord::tally).fold((0.0, 0.0), |(e, a), (x, y)| (e + x, a + y));
    Tally { earned, available }
}

#[cfg(test)]
mod tests;
