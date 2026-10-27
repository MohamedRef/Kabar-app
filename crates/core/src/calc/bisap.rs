//! BISAP score for acute pancreatitis.
//!
//! Criteria (Wu et al., Gut 2008), one point each: BUN > 25 mg/dL, impaired
//! mental status, SIRS (two or more of: temperature < 36 or > 38 °C, heart
//! rate > 90, respiratory rate > 20 or PaCO₂ < 32 mmHg, WBC < 4,000 or >
//! 12,000/mm³ or > 10% bands), age > 60, pleural effusion. The book
//! (Lecture 15, p. 288) reads a score of 2 or more as severe acute
//! pancreatitis; Wu 2008 reported the mortality rise from 3 points.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, flag, num};

pub const SPEC: CalcSpec = CalcSpec {
    id: "bisap",
    name: "BISAP",
    area: Area::Pancreas,
    fields: &[
        num("bun_mg_dl", "BUN", "mg/dL", 1.0, 300.0, 1.0).hint("Urea ÷ 2.14."),
        flag("impaired_mental_status", "Impaired mental status"),
        flag("sirs", "SIRS (2 or more criteria)"),
        num("age_years", "Age", "years", 0.0, 110.0, 1.0),
        flag("pleural_effusion", "Pleural effusion"),
    ],
    source: "Wu 2008; book Lecture 15, p. 288",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let parts = vec![
        Part::met("BUN > 25 mg/dL", i.get_num("bun_mg_dl")? > 25.0),
        Part::met("Impaired mental status", i.get_flag("impaired_mental_status")?),
        Part::met("SIRS", i.get_flag("sirs")?),
        Part::met("Age > 60", i.get_num("age_years")? > 60.0),
        Part::met("Pleural effusion", i.get_flag("pleural_effusion")?),
    ];
    let total = parts.iter().filter(|p| p.points > 0.0).count() as u32;
    let (band, label, meaning) = if total >= 2 {
        ("bisap.severe", "2 or more", "Predicts severe acute pancreatitis, organ failure and mortality.")
    } else {
        ("bisap.mild", "0–1", "Severe acute pancreatitis less likely.")
    };
    Ok(Outcome {
        score: Some(total as f64),
        display: total.to_string(),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(288),
        parts,
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_cut_off_is_two() {
        let i = Inputs::new().num("bun_mg_dl", 30.0).num("age_years", 65.0);
        let o = compute(&i).unwrap();
        assert_eq!(o.score, Some(2.0));
        assert_eq!(o.band, "bisap.severe");
        let o = compute(&Inputs::new().num("bun_mg_dl", 20.0).num("age_years", 65.0)).unwrap();
        assert_eq!(o.band, "bisap.mild");
    }

    #[test]
    fn all_five() {
        let i = Inputs::new()
            .num("bun_mg_dl", 40.0)
            .flag("impaired_mental_status", true)
            .flag("sirs", true)
            .num("age_years", 70.0)
            .flag("pleural_effusion", true);
        assert_eq!(compute(&i).unwrap().score, Some(5.0));
    }
}
