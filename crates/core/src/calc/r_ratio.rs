//! R ratio (R value) for the pattern of drug-induced liver injury.
//!
//! Formula: (ALT / ALT ULN) ÷ (ALP / ALP ULN). R ≥ 5 hepatocellular,
//! R ≤ 2 cholestatic, 2 < R < 5 mixed (CIOMS/RUCAM; EASL DILI guideline 2019;
//! Kullak-Ublick, Gut 2017, which the book reproduces in Lecture 8, p. 129).

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, num, show};

pub const SPEC: CalcSpec = CalcSpec {
    id: "r_ratio",
    name: "R ratio (DILI pattern)",
    area: Area::Liver,
    fields: &[
        num("alt_u_l", "ALT", "U/L", 1.0, 20000.0, 1.0),
        num("alt_uln_u_l", "ALT upper limit of normal", "U/L", 10.0, 100.0, 1.0),
        num("alp_u_l", "ALP", "U/L", 1.0, 5000.0, 1.0),
        num("alp_uln_u_l", "ALP upper limit of normal", "U/L", 30.0, 500.0, 1.0),
    ],
    source: "EASL DILI 2019; Kullak-Ublick 2017 (book Lecture 8, p. 129)",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let alt = i.get_pos("alt_u_l")? / i.get_pos("alt_uln_u_l")?;
    let alp = i.get_pos("alp_u_l")? / i.get_pos("alp_uln_u_l")?;
    let r = alt / alp;
    let (band, label, meaning) = if r >= 5.0 {
        ("r_ratio.hepatocellular", "Hepatocellular (R ≥ 5)", "Hepatocellular pattern of liver injury.")
    } else if r <= 2.0 {
        ("r_ratio.cholestatic", "Cholestatic (R ≤ 2)", "Cholestatic pattern of liver injury.")
    } else {
        ("r_ratio.mixed", "Mixed (R 2–5)", "Mixed hepatocellular-cholestatic pattern.")
    };
    Ok(Outcome {
        score: Some(r),
        display: show(r, 1),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(129),
        parts: vec![Part::new("ALT × ULN", alt), Part::new("ALP × ULN", alp)],
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(alt: f64, alp: f64) -> Outcome {
        compute(
            &Inputs::new()
                .num("alt_u_l", alt)
                .num("alt_uln_u_l", 40.0)
                .num("alp_u_l", alp)
                .num("alp_uln_u_l", 120.0),
        )
        .unwrap()
    }

    #[test]
    fn patterns() {
        // ALT 10×ULN, ALP 1×ULN → R 10.
        assert_eq!(run(400.0, 120.0).band, "r_ratio.hepatocellular");
        // ALT 2×ULN, ALP 3×ULN → R 0.67.
        assert_eq!(run(80.0, 360.0).band, "r_ratio.cholestatic");
        // ALT 6×ULN, ALP 2×ULN → R 3.
        let o = run(240.0, 240.0);
        assert!((o.score.unwrap() - 3.0).abs() < 1e-9);
        assert_eq!(o.band, "r_ratio.mixed");
        // Boundaries: exactly 5 is hepatocellular, exactly 2 cholestatic.
        assert_eq!(run(200.0, 120.0).band, "r_ratio.hepatocellular");
        assert_eq!(run(80.0, 120.0).band, "r_ratio.cholestatic");
    }
}
