//! Beaujon-Paul Brousse (Clichy) criteria for emergent liver transplantation
//! in acute liver failure.
//!
//! Source: the book's table, Lecture 10, p. 187 (Bernuau et al., 1986):
//! encephalopathy stage 3–4 (confusion or coma) and factor V < 20% of normal
//! below age 30, or < 30% from age 30.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, num};

pub const SPEC: CalcSpec = CalcSpec {
    id: "clichy",
    name: "Clichy criteria",
    area: Area::AcuteFailure,
    fields: &[
        num("he_stage", "Encephalopathy stage", "", 0.0, 4.0, 1.0),
        num("factor_v_pct", "Factor V", "% of normal", 0.0, 200.0, 1.0),
        num("age_years", "Age", "years", 0.0, 110.0, 1.0),
    ],
    source: "Book Lecture 10, p. 187 (Bernuau 1986)",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let he = i.get_num("he_stage")? >= 3.0;
    let age = i.get_num("age_years")?;
    let limit = if age < 30.0 { 20.0 } else { 30.0 };
    let fv = i.get_num("factor_v_pct")? < limit;
    let (band, label, meaning) = if he && fv {
        ("clichy.met", "Criteria met", "List for emergent liver transplantation.")
    } else {
        ("clichy.not_met", "Criteria not met", "Does not meet the Clichy criteria now; reassess.")
    };
    Ok(Outcome {
        score: None,
        display: label.to_owned(),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(187),
        parts: vec![
            Part::met("Encephalopathy stage 3–4", he),
            Part::met(if age < 30.0 { "Factor V < 20% (age < 30)" } else { "Factor V < 30% (age ≥ 30)" }, fv),
        ],
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(he: f64, fv: f64, age: f64) -> &'static str {
        compute(&Inputs::new().num("he_stage", he).num("factor_v_pct", fv).num("age_years", age))
            .unwrap()
            .band
    }

    #[test]
    fn age_dependent_factor_v() {
        assert_eq!(run(3.0, 25.0, 35.0), "clichy.met");
        assert_eq!(run(3.0, 25.0, 25.0), "clichy.not_met");
        assert_eq!(run(4.0, 15.0, 25.0), "clichy.met");
        assert_eq!(run(2.0, 10.0, 50.0), "clichy.not_met");
    }
}
