//! Verify-while-study: the first time Mohamed sees an item, after the
//! answer, he marks it ✓ correct, ✎ edit or ✗ wrong.
//!
//! - ✓ approves it. It never asks again.
//! - ✎ keeps his corrected text as an override that the app shows from then
//!   on ([`apply_edit`]).
//! - ✗ takes it out of review until the content is fixed.
//!
//! Each verdict remembers a fingerprint of the item it judged
//! ([`fingerprint`]). When a content thread later changes that item, the
//! verdict no longer matches and the item asks again, so a fix is checked
//! and a stale override never hides it.
//!
//! [`Verdicts`] is the export (`batna.verdicts/v1`) that `batna-tools` reads
//! to print a fix list per lecture and to add approvals to
//! `content/approved.txt`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::content::{Body, Item, Status};

pub const VERDICTS_SCHEMA: &str = "batna.verdicts/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// ✓ correct.
    Approved,
    /// ✎ corrected by Mohamed: the override is what he studies.
    Edited,
    /// ✗ wrong: out of review until the content is fixed.
    Wrong,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Edited => "edited",
            Self::Wrong => "wrong",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Approved, Self::Edited, Self::Wrong].into_iter().find(|v| v.as_str() == s)
    }
}

/// One field of an edit, keyed like the content YAML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    /// `answer` (0-based) on an MCQ.
    Index(usize),
    Text(String),
    /// mcq `options` / `why`, enumerate `points`, order `steps`.
    List(Vec<String>),
}

pub type Edit = BTreeMap<String, FieldValue>;

/// Mohamed's verdict on one item, as the app keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Judgement {
    pub verdict: Verdict,
    /// The overridden fields (✎ only).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub edit: Edit,
    /// What he said was wrong, in his words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// [`fingerprint`] of the item he judged.
    pub fingerprint: String,
    /// Unix ms.
    pub at_ms: i64,
}

impl Judgement {
    /// Still about the item as it is now (the content hasn't changed since).
    pub fn current(&self, item: &Item) -> bool {
        self.fingerprint == fingerprint(item)
    }
}

/// What the app does with an item, given its content status and his verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// Show ✓ / ✎ / ✗ after the answer.
    Ask,
    /// Approved in the content or by his current ✓ or ✎.
    Settled,
    /// His current ✗: hidden from review.
    Hidden,
}

pub fn check(item: &Item, judgement: Option<&Judgement>) -> Check {
    if item.status == Status::Approved {
        return Check::Settled;
    }
    match judgement.filter(|j| j.current(item)) {
        Some(j) if j.verdict == Verdict::Wrong => Check::Hidden,
        Some(_) => Check::Settled,
        None if item.status == Status::Rejected => Check::Hidden,
        None => Check::Ask,
    }
}

/// The item as he should see it: his current ✎ applied.
pub fn effective(item: &Item, judgement: Option<&Judgement>) -> Item {
    match judgement.filter(|j| j.verdict == Verdict::Edited && j.current(item)) {
        Some(j) => Item { body: apply_edit(&item.body, &j.edit), ..item.clone() },
        None => item.clone(),
    }
}

/// A short, stable fingerprint of what an item says (its kind's fields, its
/// page and its update), so a verdict can tell when the content changed.
/// FNV-1a over the JSON form.
pub fn fingerprint(item: &Item) -> String {
    let json = serde_json::to_string(&(&item.body, &item.src, &item.update)).unwrap_or_default();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in json.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The fields the ✎ editor offers for each kind, with their current values.
/// Nested structures (essay outlines, case steps, table grids) aren't edited
/// field by field: his note says what to change there.
pub fn editable_fields(body: &Body) -> Vec<(&'static str, FieldValue)> {
    let t = |s: &str| FieldValue::Text(s.to_string());
    let l = |v: &[String]| FieldValue::List(v.to_vec());
    match body {
        Body::Card { front, back } => vec![("front", t(front)), ("back", t(back))],
        Body::Cloze { text, extra } => {
            vec![("text", t(text)), ("extra", t(extra.as_deref().unwrap_or("")))]
        }
        Body::Mcq { stem, options, answer, why } => vec![
            ("stem", t(stem)),
            ("options", l(options)),
            ("answer", FieldValue::Index(*answer)),
            ("why", l(why)),
        ],
        Body::Enumerate { prompt, points, .. } => vec![("prompt", t(prompt)), ("points", l(points))],
        Body::Essay { prompt, .. } => vec![("prompt", t(prompt))],
        Body::Case { stem, .. } => vec![("stem", t(stem))],
        Body::Table { title, .. } => vec![("title", t(title))],
        Body::Order { prompt, steps } => vec![("prompt", t(prompt)), ("steps", l(steps))],
        Body::Calc { stem, question, expect, interpret, .. } => vec![
            ("stem", t(stem)),
            ("question", t(question.as_deref().unwrap_or(""))),
            ("expect", t(expect)),
            ("interpret", t(interpret)),
        ],
    }
}

/// Only the fields that differ from the item: what a ✎ stores.
pub fn diff(body: &Body, edited: &[(&str, FieldValue)]) -> Edit {
    let before: BTreeMap<&str, FieldValue> = editable_fields(body).into_iter().collect();
    edited
        .iter()
        .filter(|(k, v)| before.get(k).is_some_and(|b| b != v))
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

/// Why an edit can't be saved, in Egyptian Arabic for the editor.
pub fn edit_problem(body: &Body, edit: &Edit) -> Option<&'static str> {
    let edited = apply_edit(body, edit);
    let empty = |s: &str| s.trim().is_empty();
    match &edited {
        Body::Card { front, back } if empty(front) || empty(back) => Some("الوش والضهر لازم يكون فيهم كلام."),
        Body::Cloze { text, .. } => match crate::content::cloze_blanks(text) {
            Some(b) if !b.is_empty() => None,
            _ => Some("الفراغات لازم تكون بالشكل ده: {{c1::الإجابة}}"),
        },
        Body::Mcq { options, answer, why, .. } => {
            if options.len() < 2 || options.iter().any(|o| empty(o)) {
                Some("كل اختيار في سطر، ومفيش سطر فاضي.")
            } else if why.len() != options.len() {
                Some("لازم سطر شرح لكل اختيار.")
            } else if *answer >= options.len() {
                Some("رقم الإجابة الصح أكبر من عدد الاختيارات.")
            } else {
                None
            }
        }
        Body::Enumerate { points, .. } | Body::Order { steps: points, .. }
            if points.is_empty() || points.iter().any(|p| empty(p)) =>
        {
            Some("كل نقطة في سطر، ومفيش سطر فاضي.")
        }
        _ => None,
    }
}

/// The body with an edit applied. Keys that don't fit the kind, or values
/// of the wrong shape, are ignored.
pub fn apply_edit(body: &Body, edit: &Edit) -> Body {
    let mut out = body.clone();
    for (key, value) in edit {
        match (&mut out, key.as_str(), value) {
            (Body::Card { front: f, .. }, "front", FieldValue::Text(v))
            | (Body::Card { back: f, .. }, "back", FieldValue::Text(v))
            | (Body::Cloze { text: f, .. }, "text", FieldValue::Text(v))
            | (Body::Mcq { stem: f, .. }, "stem", FieldValue::Text(v))
            | (Body::Enumerate { prompt: f, .. }, "prompt", FieldValue::Text(v))
            | (Body::Essay { prompt: f, .. }, "prompt", FieldValue::Text(v))
            | (Body::Case { stem: f, .. }, "stem", FieldValue::Text(v))
            | (Body::Table { title: f, .. }, "title", FieldValue::Text(v))
            | (Body::Order { prompt: f, .. }, "prompt", FieldValue::Text(v))
            | (Body::Calc { stem: f, .. }, "stem", FieldValue::Text(v))
            | (Body::Calc { expect: f, .. }, "expect", FieldValue::Text(v))
            | (Body::Calc { interpret: f, .. }, "interpret", FieldValue::Text(v)) => *f = v.clone(),
            (Body::Cloze { extra: f, .. }, "extra", FieldValue::Text(v))
            | (Body::Calc { question: f, .. }, "question", FieldValue::Text(v)) => {
                *f = Some(v.clone()).filter(|v| !v.trim().is_empty());
            }
            (Body::Mcq { options: f, .. }, "options", FieldValue::List(v))
            | (Body::Mcq { why: f, .. }, "why", FieldValue::List(v))
            | (Body::Enumerate { points: f, .. }, "points", FieldValue::List(v))
            | (Body::Order { steps: f, .. }, "steps", FieldValue::List(v)) => *f = v.clone(),
            (Body::Mcq { answer, .. }, "answer", FieldValue::Index(i)) => *answer = *i,
            _ => {}
        }
    }
    // Keep an edited enumerate passable.
    if let Body::Enumerate { points, pass, .. } = &mut out {
        *pass = (*pass).min(points.len()).max(1);
    }
    out
}

/// The export: every verdict, so content threads can fix the source and
/// approvals can go into `content/approved.txt`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdicts {
    pub schema: String,
    /// RFC 3339, UTC.
    pub exported: String,
    pub verdicts: Vec<VerdictEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerdictEntry {
    pub id: String,
    pub verdict: Verdict,
    /// RFC 3339, UTC.
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub edit: Edit,
}

impl Verdicts {
    /// The export from current judgements, by item id.
    pub fn from_judgements<'a>(
        judgements: impl IntoIterator<Item = (&'a str, &'a Judgement)>,
        exported_ms: i64,
    ) -> Self {
        let mut verdicts: Vec<VerdictEntry> = judgements
            .into_iter()
            .map(|(id, j)| VerdictEntry {
                id: id.to_string(),
                verdict: j.verdict,
                at: rfc3339(j.at_ms),
                note: j.note.clone(),
                edit: j.edit.clone(),
            })
            .collect();
        verdicts.sort_by(|a, b| a.id.cmp(&b.id));
        Self { schema: VERDICTS_SCHEMA.into(), exported: rfc3339(exported_ms), verdicts }
    }
}

/// Unix ms → `2026-11-02T09:00:00Z`.
pub fn rfc3339(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's algorithm.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Lecture;

    fn sample() -> Lecture {
        let text = include_str!("../../../../fixtures/content/sample.yaml");
        serde_yaml_ng::from_str(text).unwrap()
    }

    fn item(kind: &str) -> Item {
        sample().items.into_iter().find(|i| i.body.kind() == kind).unwrap()
    }

    fn judged(item: &Item, verdict: Verdict, edit: Edit) -> Judgement {
        Judgement { verdict, edit, note: None, fingerprint: fingerprint(item), at_ms: 0 }
    }

    #[test]
    fn unjudged_items_ask_and_verdicts_settle_or_hide() {
        let card = item("card");
        assert_eq!(check(&card, None), Check::Ask);
        assert_eq!(check(&card, Some(&judged(&card, Verdict::Approved, Edit::new()))), Check::Settled);
        assert_eq!(check(&card, Some(&judged(&card, Verdict::Wrong, Edit::new()))), Check::Hidden);
        let approved = Item { status: Status::Approved, ..card.clone() };
        assert_eq!(check(&approved, None), Check::Settled);
    }

    #[test]
    fn a_content_change_asks_again() {
        let card = item("card");
        let j = judged(&card, Verdict::Wrong, Edit::new());
        let fixed = Item { body: Body::Card { front: "fixed".into(), back: "fixed".into() }, ..card.clone() };
        assert_eq!(check(&fixed, Some(&j)), Check::Ask);
        // A stale override is dropped too.
        let e =
            judged(&card, Verdict::Edited, Edit::from([("back".into(), FieldValue::Text("mine".into()))]));
        assert_eq!(effective(&fixed, Some(&e)), fixed);
        // Status alone doesn't change the fingerprint.
        let approved = Item { status: Status::Approved, ..card.clone() };
        assert_eq!(fingerprint(&approved), fingerprint(&card));
    }

    #[test]
    fn edits_apply_and_diff_round_trips() {
        let mcq = item("mcq");
        let mut fields = editable_fields(&mcq.body);
        for (k, v) in &mut fields {
            if *k == "answer" {
                *v = FieldValue::Index(2);
            }
        }
        let edit = diff(&mcq.body, &fields);
        assert_eq!(edit.len(), 1);
        let j = judged(&mcq, Verdict::Edited, edit);
        let Body::Mcq { answer, .. } = effective(&mcq, Some(&j)).body else { panic!() };
        assert_eq!(answer, 2);

        let card = item("card");
        let edit = Edit::from([("back".into(), FieldValue::Text("Fibrosis + nodules".into()))]);
        assert_eq!(
            apply_edit(&card.body, &edit),
            Body::Card {
                front: "Anatomical definition of cirrhosis?".into(),
                back: "Fibrosis + nodules".into()
            }
        );
        // Wrong shapes are ignored rather than breaking the item.
        let junk = Edit::from([
            ("back".into(), FieldValue::Index(3)),
            ("nope".into(), FieldValue::Text("x".into())),
        ]);
        assert_eq!(apply_edit(&card.body, &junk), card.body);
    }

    #[test]
    fn edit_problems_are_caught_before_saving() {
        let mcq = item("mcq");
        assert_eq!(edit_problem(&mcq.body, &Edit::new()), None);
        let bad = Edit::from([("answer".into(), FieldValue::Index(9))]);
        assert!(edit_problem(&mcq.body, &bad).is_some());
        let cloze = item("cloze");
        let bad = Edit::from([("text".into(), FieldValue::Text("no blanks".into()))]);
        assert!(edit_problem(&cloze.body, &bad).is_some());
    }

    #[test]
    fn export_matches_the_agreed_yaml_shape() {
        let card = item("card");
        let mut e =
            judged(&card, Verdict::Edited, Edit::from([("back".into(), FieldValue::Text("new".into()))]));
        e.note = Some("too long".into());
        e.at_ms = 1_793_610_000_000;
        let w = Judgement { at_ms: 1_793_610_000_000, ..judged(&card, Verdict::Wrong, Edit::new()) };
        let v = Verdicts::from_judgements([("hb.L01.005", &e), ("hb.L01.004", &w)], 1_793_613_600_000);
        let yaml = serde_yaml_ng::to_string(&v).unwrap();
        assert!(yaml.starts_with("schema: batna.verdicts/v1\nexported: 2026-11-02T10:00:00Z\n"), "{yaml}");
        assert!(yaml.contains("- id: hb.L01.004\n  verdict: wrong\n  at: 2026-11-02T09:00:00Z\n"), "{yaml}");
        assert!(yaml.contains("edit:\n    back: new"), "{yaml}");
        let back: Verdicts = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(back, v);
        // Lists and indexes read back in their own shape.
        let v: Edit = serde_yaml_ng::from_str("answer: 2\nwhy: [a, b]\nstem: '3'").unwrap();
        assert_eq!(v["answer"], FieldValue::Index(2));
        assert_eq!(v["why"], FieldValue::List(vec!["a".into(), "b".into()]));
        assert_eq!(v["stem"], FieldValue::Text("3".into()));
    }

    #[test]
    fn dates() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_709_164_800_000), "2024-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_798_761_599_000), "2026-12-31T23:59:59Z");
    }
}
