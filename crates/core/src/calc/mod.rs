//! Clinical calculators for hepatology and the pancreas.
//!
//! Every calculator is a pure function from named [`Inputs`] to an
//! [`Outcome`]. The [`CalcSpec`] registry describes each calculator's fields so
//! the app can render one generic form, and so a content item of kind `calc`
//! (`calculator: child_pugh`, `inputs: {…}`) can be computed and checked by
//! the validator. Where the book gives the cut-offs, the book wins and the
//! outcome carries its printed page; other sources are cited in each module.
//!
//! Medical text here is English (like the book and the exam). Units follow
//! the book and Egyptian lab reports: bilirubin mg/dL, albumin g/dL,
//! creatinine mg/dL. Calculators whose published formula uses SI units
//! convert internally.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

pub use crate::content::CalcValue;

mod albi;
mod apri;
mod ascitic;
mod bclc;
mod bisap;
mod child_pugh;
mod clichy;
mod fib4;
mod glasgow;
mod kings;
mod maddrey;
mod meld;
mod r_ratio;
mod ranson;
mod saag;

/// Named inputs for one calculation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Inputs(pub BTreeMap<String, CalcValue>);

impl Inputs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Content inputs for one calculator. A key may drop its unit suffix
    /// (`alt` for `alt_u_l`, `bilirubin` for `bilirubin_mg_dl`); it is renamed
    /// to the field's key. Keys that match no field stay as written, so
    /// [`unknown_inputs`] can report them.
    pub fn for_spec(spec: &CalcSpec, raw: &BTreeMap<String, CalcValue>) -> Self {
        let rename = |k: &String| {
            spec.fields
                .iter()
                .find(|f| f.key == k || base_key(f.key) == k)
                .map_or_else(|| k.clone(), |f| f.key.to_owned())
        };
        Self(raw.iter().map(|(k, v)| (rename(k), v.clone())).collect())
    }

    pub fn num(mut self, key: &str, v: f64) -> Self {
        self.0.insert(key.to_owned(), CalcValue::Number(v));
        self
    }

    pub fn flag(mut self, key: &str, v: bool) -> Self {
        self.0.insert(key.to_owned(), CalcValue::Bool(v));
        self
    }

    pub fn choice(mut self, key: &str, v: &str) -> Self {
        self.0.insert(key.to_owned(), CalcValue::Text(v.to_owned()));
        self
    }

    /// A required finite number.
    pub fn get_num(&self, key: &'static str) -> Result<f64, CalcError> {
        self.opt_num(key)?.ok_or(CalcError::Missing(key))
    }

    /// An optional finite number. Integers written as choices ("2") are
    /// accepted so form text fields can pass strings straight through.
    pub fn opt_num(&self, key: &'static str) -> Result<Option<f64>, CalcError> {
        let v = match self.0.get(key) {
            None => return Ok(None),
            Some(CalcValue::Number(n)) => *n,
            Some(CalcValue::Text(s)) if s.trim().is_empty() => return Ok(None),
            Some(CalcValue::Text(s)) => s
                .trim()
                .parse::<f64>()
                .map_err(|_| CalcError::Invalid { field: key, reason: "not a number" })?,
            Some(CalcValue::Bool(_)) => {
                return Err(CalcError::Invalid { field: key, reason: "expected a number" });
            }
        };
        if v.is_finite() {
            Ok(Some(v))
        } else {
            Err(CalcError::Invalid { field: key, reason: "not a finite number" })
        }
    }

    /// A positive number, as needed under a logarithm or a division.
    pub fn get_pos(&self, key: &'static str) -> Result<f64, CalcError> {
        let v = self.get_num(key)?;
        if v > 0.0 { Ok(v) } else { Err(CalcError::Invalid { field: key, reason: "must be above zero" }) }
    }

    /// A yes/no input; missing means no.
    pub fn get_flag(&self, key: &'static str) -> Result<bool, CalcError> {
        match self.0.get(key) {
            None => Ok(false),
            Some(CalcValue::Bool(b)) => Ok(*b),
            Some(CalcValue::Text(s)) => match s.as_str() {
                "yes" | "true" => Ok(true),
                "no" | "false" | "" => Ok(false),
                _ => Err(CalcError::Invalid { field: key, reason: "expected yes or no" }),
            },
            Some(CalcValue::Number(_)) => {
                Err(CalcError::Invalid { field: key, reason: "expected yes or no" })
            }
        }
    }

    /// A required choice, matched against each option's id and its aliases
    /// (content writers say "mild" as often as "mild_moderate"). Returns the id.
    pub fn get_option(
        &self,
        key: &'static str,
        options: &[(&'static str, &[&str])],
    ) -> Result<&'static str, CalcError> {
        let text = match self.0.get(key) {
            None => return Err(CalcError::Missing(key)),
            Some(CalcValue::Text(s)) => s.trim().to_ascii_lowercase().replace([' ', '-'], "_"),
            Some(_) => return Err(CalcError::Invalid { field: key, reason: "not one of the options" }),
        };
        options
            .iter()
            .find(|(id, aliases)| *id == text || aliases.contains(&text.as_str()))
            .map(|(id, _)| *id)
            .ok_or(CalcError::Invalid { field: key, reason: "not one of the options" })
    }

    /// A required choice that must be one of `allowed`.
    pub fn get_choice(&self, key: &'static str, allowed: &[&str]) -> Result<&str, CalcError> {
        match self.0.get(key) {
            None => Err(CalcError::Missing(key)),
            Some(CalcValue::Text(s)) if allowed.contains(&s.as_str()) => Ok(s.as_str()),
            Some(_) => Err(CalcError::Invalid { field: key, reason: "not one of the options" }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CalcError {
    Unknown(String),
    Missing(&'static str),
    Invalid { field: &'static str, reason: &'static str },
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "unknown calculator `{id}`"),
            Self::Missing(k) => write!(f, "missing input `{k}`"),
            Self::Invalid { field, reason } => write!(f, "input `{field}`: {reason}"),
        }
    }
}

impl std::error::Error for CalcError {}

/// One line of the breakdown: a criterion and what it contributed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Part {
    pub label: String,
    /// Points for scores; 1/0 for criteria met or not.
    pub points: f64,
    /// `Some` for yes/no criteria (King's, Ranson, BISAP), `None` for points.
    pub met: Option<bool>,
}

impl Part {
    fn new(label: impl Into<String>, points: f64) -> Self {
        Self { label: label.into(), points, met: None }
    }

    fn met(label: impl Into<String>, met: bool) -> Self {
        Self { label: label.into(), points: if met { 1.0 } else { 0.0 }, met: Some(met) }
    }
}

/// The result of one calculation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Outcome {
    /// Numeric result; `None` for pure decision tables (BCLC, King's).
    pub score: Option<f64>,
    /// The result as shown, e.g. "9", "1.4 g/dL", "Stage B".
    pub display: String,
    /// Stable band id, e.g. `child_pugh.b`. Content `expect:` and
    /// interpretation MCQs key on this.
    pub band: &'static str,
    /// Short English label of the band, e.g. "Class B (7–9 points)".
    pub label: &'static str,
    /// What the band means, in the book's words where it has them.
    pub meaning: String,
    /// Printed book page of the cut-offs, when the book has them.
    pub page: Option<u32>,
    pub parts: Vec<Part>,
    /// Caveats, e.g. "48-hour criteria not entered".
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldKind {
    Number {
        unit: &'static str,
        min: f64,
        max: f64,
        step: f64,
    },
    Flag,
    /// (id, English label) pairs.
    Choice(&'static [(&'static str, &'static str)]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    pub optional: bool,
    pub hint: &'static str,
}

const fn num(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    min: f64,
    max: f64,
    step: f64,
) -> Field {
    Field { key, label, kind: FieldKind::Number { unit, min, max, step }, optional: false, hint: "" }
}

const fn flag(key: &'static str, label: &'static str) -> Field {
    Field { key, label, kind: FieldKind::Flag, optional: true, hint: "" }
}

const fn choice(
    key: &'static str,
    label: &'static str,
    options: &'static [(&'static str, &'static str)],
) -> Field {
    Field { key, label, kind: FieldKind::Choice(options), optional: false, hint: "" }
}

impl Field {
    const fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    const fn hint(mut self, hint: &'static str) -> Self {
        self.hint = hint;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Liver,
    Ascites,
    Tumour,
    AcuteFailure,
    Pancreas,
}

pub struct CalcSpec {
    /// The id used in content (`calculator: child_pugh`).
    pub id: &'static str,
    pub name: &'static str,
    pub area: Area,
    pub fields: &'static [Field],
    /// Formula source, shown under the result.
    pub source: &'static str,
    pub compute: fn(&Inputs) -> Result<Outcome, CalcError>,
}

impl fmt::Debug for CalcSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalcSpec").field("id", &self.id).finish_non_exhaustive()
    }
}

/// Every calculator, in the order the Calculators screen lists them.
pub fn all() -> &'static [CalcSpec] {
    &REGISTRY
}

pub fn spec(id: &str) -> Option<&'static CalcSpec> {
    REGISTRY.iter().find(|s| s.id == id)
}

pub fn compute(id: &str, inputs: &Inputs) -> Result<Outcome, CalcError> {
    let spec = spec(id).ok_or_else(|| CalcError::Unknown(id.to_owned()))?;
    (spec.compute)(inputs)
}

static REGISTRY: [CalcSpec; 16] = [
    child_pugh::SPEC,
    meld::MELD,
    meld::MELD_NA,
    albi::SPEC,
    fib4::SPEC,
    apri::SPEC,
    r_ratio::SPEC,
    maddrey::SPEC,
    glasgow::SPEC,
    saag::SPEC,
    ascitic::SPEC,
    bclc::SPEC,
    kings::SPEC,
    clichy::SPEC,
    ranson::SPEC,
    bisap::SPEC,
];

const UNIT_SUFFIXES: &[&str] = &[
    "_10e9_l", "_per_mm3", "_mm_hg", "_meq_l", "_mmol_l", "_mg_dl", "_g_dl", "_iu_l", "_u_l", "_years",
    "_points", "_days", "_pct", "_cm", "_s", "_l",
];

/// A field key without its unit suffix: `alt_uln_u_l` → `alt_uln`.
fn base_key(key: &str) -> &str {
    UNIT_SUFFIXES.iter().find_map(|s| key.strip_suffix(s)).unwrap_or(key)
}

/// Input keys in `inputs` that the calculator does not read: almost always a
/// typo in content.
pub fn unknown_inputs<'a>(spec: &CalcSpec, inputs: &'a Inputs) -> Vec<&'a str> {
    inputs.0.keys().map(String::as_str).filter(|k| !spec.fields.iter().any(|f| f.key == *k)).collect()
}

/// Whether a content item's `expect` agrees with the computed outcome. It
/// may be the band id (`child_pugh.b`) or free text that shows the result,
/// such as "Child-Pugh B (8 points)" (contains "8") or "Stage C" for BCLC.
pub fn expect_matches(outcome: &Outcome, expect: &str) -> bool {
    let expect = expect.trim();
    if expect == outcome.band {
        return true;
    }
    if let Some(score) = outcome.score {
        // A number written in `expect` equal to the score at the precision it
        // is written with: "0.9" and "0.90" both match 0.898.
        let matches = |n: &String| {
            let dp = n.split_once('.').map_or(0, |(_, f)| f.len()) as i32;
            let scale = 10f64.powi(dp);
            n.parse::<f64>().is_ok_and(|v| ((score * scale).round() - v * scale).abs() < 0.5)
        };
        if numbers(expect).iter().any(matches) {
            return true;
        }
    }
    expect.eq_ignore_ascii_case(&outcome.display) || expect.contains(outcome.label)
}

/// The numbers written in a text, as written ("8", "−2.59", "1.4").
fn numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        let sign = (c == '-' || c == '−') && cur.is_empty();
        if c.is_ascii_digit() || (c == '.' && !cur.is_empty()) || sign {
            cur.push(if c == '−' { '-' } else { c });
        } else if !cur.is_empty() {
            let n = std::mem::take(&mut cur);
            if n.chars().any(|c| c.is_ascii_digit()) {
                out.push(n.trim_end_matches('.').to_owned());
            }
        }
    }
    out
}

/// `docs/CALCULATORS.md`: every calculator's id and inputs, for content
/// writers. A test keeps the file equal to this.
pub fn reference_markdown() -> String {
    let mut out = String::from(
        "# Calculators\n\nGenerated from `crates/core/src/calc` (run \
         `cargo test -p batna-core -- --ignored print_reference`). A `calc` item names one of these in \
         `calculator:` and gives its `inputs` by key (a key may drop its unit suffix: `alt` for \
         `alt_u_l`). `just validate` computes every item, so a typo, a \
         missing input or an `expect` that disagrees fails the check. `expect` may be the band id \
         (`child_pugh.b`) or text that contains the result (`Child-Pugh B (8 points)`, `Stage C`).\n",
    );
    for s in all() {
        out.push_str(&format!(
            "\n## `{}`: {}\n\nSource: {}\n\n| input | kind | |\n|---|---|---|\n",
            s.id, s.name, s.source
        ));
        for f in s.fields {
            let kind = match f.kind {
                FieldKind::Number { unit: "", .. } => "number".to_owned(),
                FieldKind::Number { unit, .. } => format!("number, {unit}"),
                FieldKind::Flag => "true / false (default false)".to_owned(),
                FieldKind::Choice(opts) => {
                    opts.iter().map(|(id, _)| format!("`{id}`")).collect::<Vec<_>>().join(", ")
                }
            };
            let note = if f.optional && !matches!(f.kind, FieldKind::Flag) { "optional" } else { "" };
            out.push_str(&format!(
                "| `{}` | {kind} | {}{}{} |\n",
                f.key,
                f.label,
                if note.is_empty() { "" } else { "; " },
                note
            ));
        }
    }
    out
}

/// Round to `dp` decimal places for display.
fn show(v: f64, dp: usize) -> String {
    format!("{v:.dp$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_are_unique_and_known() {
        let mut ids: Vec<_> = all().iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all().len());
        assert!(spec("child_pugh").is_some());
        assert!(matches!(compute("nope", &Inputs::new()), Err(CalcError::Unknown(_))));
    }

    #[test]
    fn every_field_key_is_unique_per_calculator() {
        for s in all() {
            let mut keys: Vec<_> = s.fields.iter().map(|f| f.key).collect();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), s.fields.len(), "{}", s.id);
        }
    }

    #[test]
    fn empty_inputs_fail_cleanly() {
        for s in all() {
            if s.fields.iter().any(|f| !f.optional) {
                assert!((s.compute)(&Inputs::new()).is_err(), "{}", s.id);
            }
        }
    }

    #[test]
    fn expect_accepts_band_ids_and_written_results() {
        let i = Inputs::new()
            .choice("encephalopathy", "none")
            .choice("ascites", "mild")
            .num("bilirubin_mg_dl", 2.5)
            .num("albumin_g_dl", 3.0)
            .num("pt_prolonged_s", 3.0);
        let o = compute("child_pugh", &i).unwrap();
        assert!(expect_matches(&o, "child_pugh.b"));
        assert!(expect_matches(&o, "Child-Pugh B (8 points)"));
        assert!(!expect_matches(&o, "Child-Pugh B (9 points)"));
        assert!(!expect_matches(&o, "child_pugh.c"));
        let albi =
            compute("albi", &Inputs::new().num("bilirubin_mg_dl", 1.0).num("albumin_g_dl", 4.0)).unwrap();
        assert!(expect_matches(&albi, "ALBI −2.59, grade 2"));
        let bclc = compute(
            "bclc",
            &Inputs::new()
                .num("performance_status", 1.0)
                .num("child_pugh_points", 5.0)
                .num("nodules", 1.0)
                .num("largest_cm", 3.0),
        )
        .unwrap();
        assert!(expect_matches(&bclc, "Stage C"));
        assert!(!expect_matches(&bclc, "Stage B"));
    }

    #[test]
    fn keys_may_drop_their_unit_suffix() {
        let spec = spec("r_ratio").unwrap();
        let raw: BTreeMap<String, CalcValue> =
            [("alt", 120.0), ("alt_uln", 40.0), ("alp", 600.0), ("alp_uln_u_l", 120.0)]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), CalcValue::Number(v)))
                .collect();
        let i = Inputs::for_spec(spec, &raw);
        assert!(unknown_inputs(spec, &i).is_empty());
        let o = (spec.compute)(&i).unwrap();
        assert!(expect_matches(&o, "R = 0.6, cholestatic"));
    }

    #[test]
    fn numbers_match_at_the_written_precision() {
        let i = Inputs::new()
            .num("age_years", 45.0)
            .num("ast_u_l", 30.0)
            .num("alt_u_l", 36.0)
            .num("platelets_10e9_l", 250.0);
        let o = compute("fib4", &i).unwrap();
        assert!(expect_matches(&o, "FIB-4 0.9"));
        assert!(expect_matches(&o, "FIB-4 0.90"));
        assert!(!expect_matches(&o, "FIB-4 1.9"));
    }

    #[test]
    fn unknown_inputs_are_reported() {
        let i = Inputs::new().num("bilirubin", 2.0).num("albumin_g_dl", 3.0);
        assert_eq!(unknown_inputs(spec("albi").unwrap(), &i), vec!["bilirubin"]);
    }

    #[test]
    fn registry_matches_the_content_schema_list() {
        let mut ids: Vec<_> = all().iter().map(|s| s.id).collect();
        let mut listed = crate::content::CALCULATORS.to_vec();
        ids.sort_unstable();
        listed.sort_unstable();
        assert_eq!(ids, listed);
    }

    #[test]
    fn the_reference_doc_is_current() {
        let doc = include_str!("../../../../docs/CALCULATORS.md");
        assert!(doc == reference_markdown(), "docs/CALCULATORS.md is stale: run the print_reference test");
    }

    #[test]
    #[ignore = "prints docs/CALCULATORS.md"]
    fn print_reference() {
        print!("{}", reference_markdown());
    }

    #[test]
    fn inputs_parse_from_yaml_shapes() {
        let i = Inputs::new().choice("x", "2.5").choice("f", "yes");
        assert_eq!(i.get_num("x"), Ok(2.5));
        assert_eq!(i.get_flag("f"), Ok(true));
        assert!(Inputs::new().num("x", f64::NAN).get_num("x").is_err());
        assert!(Inputs::new().num("x", 0.0).get_pos("x").is_err());
    }
}
