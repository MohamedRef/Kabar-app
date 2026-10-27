//! Checks a lecture against the content rules (SPLIT.md §4–5). Pure: the
//! tools crate reads the files and passes the parsed lecture plus the raw
//! keys of each item.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::content::{self, Body, Lecture, Status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The item id, or the lecture id for file-level problems.
    pub at: String,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.message)
    }
}

/// What the validator needs besides the lecture itself.
#[derive(Debug, Default)]
pub struct Context<'a> {
    /// The keys written on each item, by item id (from the raw YAML).
    pub raw_keys: BTreeMap<String, Vec<String>>,
    /// Item ids Mohamed approved (`content/approved.txt`).
    pub approved: Option<&'a BTreeSet<String>>,
}

pub const MAX_QUOTE_WORDS: usize = 25;

pub fn lecture(lec: &Lecture, cx: &Context<'_>) -> Vec<Problem> {
    let mut out = Vec::new();
    let mut bad = |at: &str, message: String| out.push(Problem { at: at.to_string(), message });
    let lid = lec.lecture.as_str();

    if lec.schema != content::SCHEMA {
        bad(lid, format!("schema is {:?}, expected {:?}", lec.schema, content::SCHEMA));
    }
    if !is_lecture_id(lid) {
        bad(lid, "lecture id must look like hb.L01".into());
    }
    let [first, last] = lec.book.pages;
    if first == 0 || first > last {
        bad(lid, format!("book pages {first}-{last} are not a range"));
    }
    let topics: BTreeSet<&str> = lec.topics.iter().map(|t| t.id.as_str()).collect();
    if topics.len() != lec.topics.len() {
        bad(lid, "topic ids repeat".into());
    }

    let mut seen = BTreeSet::new();
    for item in &lec.items {
        let at = item.id.as_str();
        if !seen.insert(at) {
            bad(at, "id is used twice".into());
        }
        match at.strip_prefix(lid).and_then(|r| r.strip_prefix('.')) {
            Some(n) if n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()) => {}
            _ => bad(at, format!("id must be {lid}.NNN (three digits)")),
        }
        if let Some(keys) = cx.raw_keys.get(at) {
            let own = content::allowed_keys(item.body.kind()).unwrap_or(&[]);
            for k in keys {
                if !own.contains(&k.as_str()) && !content::SHARED_KEYS.contains(&k.as_str()) {
                    bad(at, format!("unknown key `{k}` for kind {}", item.body.kind()));
                }
            }
        }
        match &item.topic {
            Some(t) if !topics.is_empty() && !topics.contains(t.as_str()) => {
                bad(at, format!("topic `{t}` is not in the lecture's topics"));
            }
            None if !topics.is_empty() => bad(at, "topic is missing".into()),
            _ => {}
        }
        if item.src.page < first || item.src.page > last {
            bad(at, format!("src.page {} is outside the lecture (pages {first}-{last})", item.src.page));
        }
        if let Some(q) = &item.src.quote
            && q.split_whitespace().count() > MAX_QUOTE_WORDS
        {
            bad(at, format!("quote is longer than {MAX_QUOTE_WORDS} words (book text stays out of git)"));
        }
        if item.status == Status::Approved && !cx.approved.is_some_and(|a| a.contains(at)) {
            bad(
                at,
                "status approved: only Mohamed approves (add the id to content/approved.txt from his export)"
                    .into(),
            );
        }
        if let Some(u) = &item.update
            && (u.text.trim().is_empty() || u.reference.trim().is_empty())
        {
            bad(at, "update needs both text and ref".into());
        }
        for message in body_problems(&item.body) {
            bad(at, message);
        }
    }
    out
}

fn body_problems(body: &Body) -> Vec<String> {
    let mut p = Vec::new();
    let empty = |s: &str| s.trim().is_empty();
    match body {
        Body::Card { front, back } => {
            if empty(front) || empty(back) {
                p.push("card needs front and back".into());
            }
        }
        Body::Cloze { text, .. } => match content::cloze_blanks(text) {
            None => p.push("cloze braces are malformed; use {{c1::answer}}".into()),
            Some(b) if b.is_empty() => p.push("cloze has no {{cN::…}} blank".into()),
            Some(b) => {
                let nums: BTreeSet<u32> = b.iter().map(|(n, _)| *n).collect();
                if nums.len() > 3 {
                    p.push("cloze has more than 3 blanks (c1–c3)".into());
                }
            }
        },
        Body::Mcq { stem, options, answer, why } => {
            if empty(stem) {
                p.push("mcq stem is empty".into());
            }
            if !(4..=5).contains(&options.len()) {
                p.push(format!("mcq needs 4 or 5 options, has {}", options.len()));
            }
            if *answer >= options.len() {
                p.push(format!("mcq answer {answer} is out of range"));
            }
            if why.len() != options.len() {
                p.push("mcq needs one `why` line per option".into());
            }
            let distinct: BTreeSet<&str> = options.iter().map(|o| o.trim()).collect();
            if distinct.len() != options.len() {
                p.push("mcq options repeat".into());
            }
        }
        Body::Enumerate { prompt, points, pass } => {
            if empty(prompt) || points.len() < 2 {
                p.push("enumerate needs a prompt and at least 2 points".into());
            }
            if *pass == 0 || *pass > points.len() {
                p.push(format!("enumerate pass {pass} must be 1..={}", points.len()));
            }
        }
        Body::Essay { prompt, outline } => {
            if empty(prompt) || outline.is_empty() {
                p.push("essay needs a prompt and an outline".into());
            }
        }
        Body::Case { stem, steps } => {
            if empty(stem) || steps.is_empty() {
                p.push("case needs a stem and at least one step".into());
            }
            for (i, s) in steps.iter().enumerate() {
                match (s.options.len(), s.answer) {
                    (0, None) => {}
                    (0, Some(_)) => p.push(format!("case step {} has an answer but no options", i + 1)),
                    (_, None) => p.push(format!("case step {} has options but no answer", i + 1)),
                    (n, Some(a)) if a >= n => p.push(format!("case step {} answer is out of range", i + 1)),
                    _ => {}
                }
                if empty(&s.why) {
                    p.push(format!("case step {} needs `why`", i + 1));
                }
            }
        }
        Body::Table { columns, rows, blanks, .. } => {
            if columns.len() < 2 || rows.is_empty() {
                p.push("table needs at least 2 columns and 1 row".into());
            }
            for (i, r) in rows.iter().enumerate() {
                if r.len() != columns.len() {
                    p.push(format!("table row {} has {} cells, expected {}", i + 1, r.len(), columns.len()));
                }
            }
            if blanks.is_empty() {
                p.push("table has no blanks".into());
            }
            for [r, c] in blanks {
                if *r >= rows.len() || *c >= columns.len() || *c == 0 {
                    p.push(format!("table blank [{r}, {c}] is outside the grid (column 0 is the label)"));
                }
            }
        }
        Body::Order { steps, .. } => {
            if steps.len() < 3 {
                p.push("order needs at least 3 steps".into());
            }
        }
        Body::Calc { calculator, inputs, expect, .. } => {
            if inputs.is_empty() {
                p.push("calc needs inputs".into());
            }
            match crate::calc::spec(calculator) {
                None => p.push(format!("unknown calculator `{calculator}`")),
                Some(spec) => {
                    // Compute it: the inputs must be complete and `expect` must agree.
                    let inputs = crate::calc::Inputs::for_spec(spec, inputs);
                    for key in crate::calc::unknown_inputs(spec, &inputs) {
                        p.push(format!("calc input `{key}` is not used by `{calculator}`"));
                    }
                    match (spec.compute)(&inputs) {
                        Err(e) => p.push(format!("calc inputs: {e}")),
                        Ok(o) if !crate::calc::expect_matches(&o, expect) => p.push(format!(
                            "calc expect {expect:?} disagrees with the calculator: {} ({}, band `{}`)",
                            o.display, o.label, o.band
                        )),
                        Ok(_) => {}
                    }
                }
            }
        }
    }
    p
}

/// `hb.L01`: a lowercase chapter slug, `.L`, two digits.
pub fn is_lecture_id(id: &str) -> bool {
    let Some((chapter, num)) = id.split_once(".L") else {
        return false;
    };
    !chapter.is_empty()
        && chapter.bytes().all(|b| b.is_ascii_lowercase())
        && num.len() == 2
        && num.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Lecture;

    fn lec(yaml: &str) -> Lecture {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    const HEAD: &str = "schema: batna.content/v1\nlecture: hb.L01\ntitle: Liver Cirrhosis\nbook: { chapter: 1, pages: [1, 22] }\n";

    #[test]
    fn the_sample_fixture_is_clean() {
        let raw = include_str!("../../../fixtures/content/sample.yaml");
        let l: Lecture = serde_yaml_ng::from_str(raw).unwrap();
        let problems = lecture(&l, &Context::default());
        assert!(problems.is_empty(), "{problems:#?}");
        let kinds: BTreeSet<_> = l.items.iter().map(|i| i.body.kind()).collect();
        assert_eq!(kinds.len(), 9, "the fixture should show every kind");
    }

    #[test]
    fn catches_the_common_mistakes() {
        let l = lec(&format!(
            "{HEAD}items:
  - {{ id: hb.L01.001, kind: card, front: a, back: b, src: {{ page: 30 }}, status: unverified }}
  - {{ id: hb.L01.001, kind: mcq, stem: s, options: [a, b, c], answer: 3, why: [x], src: {{ page: 2 }}, status: approved }}
  - {{ id: hb.L1.2, kind: cloze, text: 'no blank', src: {{ page: 2 }}, status: unverified }}
"
        ));
        let msgs: Vec<String> = lecture(&l, &Context::default()).iter().map(|p| p.to_string()).collect();
        let has = |s: &str| msgs.iter().any(|m| m.contains(s));
        assert!(has("outside the lecture"), "{msgs:#?}");
        assert!(has("used twice"));
        assert!(has("4 or 5 options"));
        assert!(has("out of range"));
        assert!(has("one `why` line"));
        assert!(has("only Mohamed approves"));
        assert!(has("three digits"));
        assert!(has("no {{cN"));
    }

    #[test]
    fn approved_is_fine_when_mohamed_approved_it() {
        let l = lec(&format!(
            "{HEAD}items:\n  - {{ id: hb.L01.001, kind: card, front: a, back: b, src: {{ page: 3 }}, status: approved }}\n"
        ));
        let ok: BTreeSet<String> = ["hb.L01.001".to_string()].into();
        let cx = Context { approved: Some(&ok), ..Context::default() };
        assert!(lecture(&l, &cx).is_empty());
    }

    #[test]
    fn unknown_keys_and_long_quotes_are_flagged() {
        let l = lec(&format!(
            "{HEAD}items:\n  - {{ id: hb.L01.001, kind: card, front: a, back: b, src: {{ page: 3, quote: '{}' }}, status: unverified }}\n",
            "word ".repeat(26)
        ));
        let cx = Context {
            raw_keys: [("hb.L01.001".to_string(), vec!["front".into(), "back".into(), "answer".into()])]
                .into(),
            ..Context::default()
        };
        let msgs: Vec<String> = lecture(&l, &cx).iter().map(|p| p.to_string()).collect();
        assert!(msgs.iter().any(|m| m.contains("unknown key `answer`")), "{msgs:#?}");
        assert!(msgs.iter().any(|m| m.contains("longer than 25 words")));
    }

    #[test]
    fn calc_items_are_computed() {
        let item = |inputs: &str, expect: &str| {
            lec(&format!(
                "{HEAD}items:\n  - id: hb.L01.001\n    kind: calc\n    calculator: child_pugh\n    stem: s\n    \
                 inputs: {{ {inputs} }}\n    expect: {expect}\n    interpret: i\n    src: {{ page: 14 }}\n    status: unverified\n"
            ))
        };
        let full =
            "encephalopathy: none, ascites: mild, bilirubin_mg_dl: 2.5, albumin_g_dl: 3.0, pt_prolonged_s: 3";
        assert!(lecture(&item(full, "Child-Pugh B (8 points)"), &Context::default()).is_empty());
        let p = lecture(&item(full, "Child-Pugh C (10 points)"), &Context::default());
        assert!(p[0].message.contains("disagrees"), "{p:?}");
        let p = lecture(&item("encephalopathy: none, ascites: mild, bili: 2.5", "x"), &Context::default());
        assert!(p.iter().any(|p| p.message.contains("`bili` is not used")), "{p:?}");
        assert!(p.iter().any(|p| p.message.contains("missing input")), "{p:?}");
    }

    #[test]
    fn lecture_ids() {
        assert!(is_lecture_id("hb.L01"));
        assert!(is_lecture_id("gi.L11"));
        assert!(!is_lecture_id("hb.L1"));
        assert!(!is_lecture_id("HB.L01"));
        assert!(!is_lecture_id("hbL01"));
    }
}
