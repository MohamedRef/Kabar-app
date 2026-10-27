//! Serum-ascites albumin gradient (SAAG).
//!
//! Source: the book, Lecture 9, p. 150–151: SAAG = serum albumin − ascitic
//! albumin (g/dL); ≥ 1.1 portal hypertension, < 1.1 non-portal-hypertensive
//! ascites. Ascitic total protein (p. 150): low < 1.5 g/dL in cirrhosis,
//! high > 2.5 g/dL in heart failure and malignant ascites.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, num, show};

pub const SPEC: CalcSpec = CalcSpec {
    id: "saag",
    name: "SAAG",
    area: Area::Ascites,
    fields: &[
        num("serum_albumin_g_dl", "Serum albumin", "g/dL", 0.5, 6.0, 0.1),
        num("ascitic_albumin_g_dl", "Ascitic albumin", "g/dL", 0.0, 6.0, 0.1),
        num("ascitic_protein_g_dl", "Ascitic total protein", "g/dL", 0.0, 8.0, 0.1).optional(),
    ],
    source: "Book Lecture 9, p. 150–151",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let serum = i.get_num("serum_albumin_g_dl")?;
    let ascitic = i.get_num("ascitic_albumin_g_dl")?;
    // Rounded to one decimal, as labs report, so 1.1 lands on the cut-off.
    let saag = ((serum - ascitic) * 10.0).round() / 10.0;
    let high = saag >= 1.1;
    let (band, label, meaning) = if high {
        ("saag.high", "High SAAG (≥ 1.1 g/dL)", "Ascites most likely due to portal hypertension.")
    } else {
        ("saag.low", "Low SAAG (< 1.1 g/dL)", "Suggests non-portal-hypertensive ascites.")
    };
    let mut notes = Vec::new();
    let mut parts = vec![Part::new("Serum albumin", serum), Part::new("Ascitic albumin", ascitic)];
    if let Some(p) = i.opt_num("ascitic_protein_g_dl")? {
        parts.push(Part::new("Ascitic protein", p));
        if p < 1.5 {
            notes.push("Protein < 1.5 g/dL: low, as in cirrhosis.".to_owned());
        } else if p > 2.5 {
            notes.push("Protein > 2.5 g/dL: high, as in heart failure or malignant ascites.".to_owned());
        }
    }
    if high {
        notes.push("Portal hypertension plus a second cause also gives SAAG ≥ 1.1.".to_owned());
    }
    Ok(Outcome {
        score: Some(saag),
        display: format!("{} g/dL", show(saag, 1)),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(150),
        parts,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: f64, a: f64) -> Outcome {
        compute(&Inputs::new().num("serum_albumin_g_dl", s).num("ascitic_albumin_g_dl", a)).unwrap()
    }

    #[test]
    fn cut_off() {
        assert_eq!(run(3.0, 1.0).band, "saag.high");
        assert_eq!(run(3.2, 2.1).band, "saag.high"); // exactly 1.1
        assert_eq!(run(3.5, 2.5).band, "saag.low");
    }

    #[test]
    fn protein_note() {
        let o = compute(
            &Inputs::new()
                .num("serum_albumin_g_dl", 3.0)
                .num("ascitic_albumin_g_dl", 1.2)
                .num("ascitic_protein_g_dl", 3.0),
        )
        .unwrap();
        assert!(o.notes.iter().any(|n| n.contains("heart failure")));
    }
}
