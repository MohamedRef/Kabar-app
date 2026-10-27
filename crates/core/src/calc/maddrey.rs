//! Maddrey discriminant function (DF) for alcoholic hepatitis.
//!
//! Formula: 4.6 × (patient PT − control PT, seconds) + bilirubin (mg/dL);
//! DF ≥ 32 is severe alcoholic hepatitis with high short-term mortality, where
//! corticosteroids are considered (Maddrey 1978; Carithers 1989; AASLD 2019
//! alcohol-associated liver disease guidance). The book (Lecture 12, p. 196)
//! discusses steroids for moderate to severe disease but gives no DF.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, num, show};

pub const SPEC: CalcSpec = CalcSpec {
    id: "maddrey",
    name: "Maddrey discriminant function",
    area: Area::Liver,
    fields: &[
        num("pt_s", "Prothrombin time (patient)", "s", 5.0, 120.0, 0.1),
        num("pt_control_s", "Prothrombin time (control)", "s", 5.0, 30.0, 0.1).hint("Often 12–13 s."),
        num("bilirubin_mg_dl", "Bilirubin", "mg/dL", 0.1, 50.0, 0.1),
    ],
    source: "Maddrey 1978, Carithers 1989; AASLD 2019",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let pt = i.get_pos("pt_s")?;
    let ctl = i.get_pos("pt_control_s")?;
    let bili = i.get_num("bilirubin_mg_dl")?;
    let pt_part = 4.6 * (pt - ctl);
    // Rounded to one decimal, as reported, so DF 32.0 lands on the cut-off.
    let df = ((pt_part + bili) * 10.0).round() / 10.0;
    let (band, label, meaning) = if df >= 32.0 {
        (
            "maddrey.severe",
            "Severe (DF ≥ 32)",
            "Severe alcoholic hepatitis: high 1-month mortality; consider corticosteroids if no contraindication.",
        )
    } else {
        (
            "maddrey.not_severe",
            "Not severe (DF < 32)",
            "Lower short-term mortality; corticosteroids not indicated by DF alone.",
        )
    };
    Ok(Outcome {
        score: Some(df),
        display: show(df, 1),
        band,
        label,
        meaning: meaning.to_owned(),
        page: None,
        parts: vec![Part::new("4.6 × PT prolongation", pt_part), Part::new("Bilirubin", bili)],
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(pt: f64, ctl: f64, b: f64) -> Outcome {
        compute(&Inputs::new().num("pt_s", pt).num("pt_control_s", ctl).num("bilirubin_mg_dl", b)).unwrap()
    }

    #[test]
    fn worked_examples() {
        // 4.6 × (19 − 12) + 10 = 42.2, severe.
        let o = run(19.0, 12.0, 10.0);
        assert!((o.score.unwrap() - 42.2).abs() < 1e-9);
        assert_eq!(o.band, "maddrey.severe");
        // 4.6 × (15 − 12) + 5 = 18.8.
        assert_eq!(run(15.0, 12.0, 5.0).band, "maddrey.not_severe");
        // Exactly 32: 4.6 × 5 + 9.
        assert_eq!(run(17.0, 12.0, 9.0).band, "maddrey.severe");
    }
}
