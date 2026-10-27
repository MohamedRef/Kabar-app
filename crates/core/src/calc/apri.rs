//! APRI (AST to platelet ratio index).
//!
//! Formula: (AST / AST upper limit of normal) × 100 / platelets [10⁹/L]
//! (Wai et al., Hepatology 2003). The book (Lecture 1, p. 10) names APRI but
//! gives no cut-offs, so the bands follow Wai 2003 and the WHO hepatitis C
//! guidelines (2016): < 0.5 significant fibrosis unlikely, > 1.5 significant
//! fibrosis likely, > 2.0 cirrhosis likely.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, num, show};

pub const SPEC: CalcSpec = CalcSpec {
    id: "apri",
    name: "APRI",
    area: Area::Liver,
    fields: &[
        num("ast_u_l", "AST", "U/L", 1.0, 10000.0, 1.0),
        num("ast_uln_u_l", "AST upper limit of normal", "U/L", 10.0, 100.0, 1.0).hint("Often 40 U/L."),
        num("platelets_10e9_l", "Platelets", "×10⁹/L", 1.0, 1500.0, 1.0),
    ],
    source: "Wai 2003; WHO HCV guidelines 2016 (book Lecture 1, p. 10 names the test)",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let ast = i.get_pos("ast_u_l")?;
    let uln = i.get_pos("ast_uln_u_l")?;
    let plt = i.get_pos("platelets_10e9_l")?;
    let score = ast / uln * 100.0 / plt;
    let (band, label, meaning) = if score < 0.5 {
        ("apri.low", "Below 0.5", "Significant fibrosis unlikely.")
    } else if score <= 1.5 {
        ("apri.mid", "0.5–1.5", "Indeterminate: confirm with elastography.")
    } else if score <= 2.0 {
        ("apri.fibrosis", "Above 1.5", "Significant fibrosis likely.")
    } else {
        ("apri.cirrhosis", "Above 2.0", "Cirrhosis likely.")
    };
    Ok(Outcome {
        score: Some(score),
        display: show(score, 2),
        band,
        label,
        meaning: meaning.to_owned(),
        page: None,
        parts: vec![Part::new("AST ÷ ULN", ast / uln)],
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ast: f64, uln: f64, plt: f64) -> Outcome {
        compute(&Inputs::new().num("ast_u_l", ast).num("ast_uln_u_l", uln).num("platelets_10e9_l", plt))
            .unwrap()
    }

    #[test]
    fn worked_examples() {
        // (80/40) × 100 / 100 = 2.0, the top of "fibrosis likely".
        let o = run(80.0, 40.0, 100.0);
        assert!((o.score.unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(o.band, "apri.fibrosis");
        assert_eq!(run(30.0, 40.0, 250.0).band, "apri.low");
        assert_eq!(run(120.0, 40.0, 90.0).band, "apri.cirrhosis");
    }
}
