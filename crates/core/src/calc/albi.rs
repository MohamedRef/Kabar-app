//! ALBI (albumin-bilirubin) grade.
//!
//! Source: the book, Lecture 18, p. 363 (Johnson et al., J Clin Oncol 2015):
//! (log10 bilirubin µmol/L × 0.66) + (albumin g/L × −0.085); grade 1 ≤ −2.60,
//! grade 2 > −2.60 to ≤ −1.39, grade 3 > −1.39. Inputs are taken in mg/dL and
//! g/dL and converted (bilirubin × 17.1, albumin × 10).

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, num, show};

pub const SPEC: CalcSpec = CalcSpec {
    id: "albi",
    name: "ALBI grade",
    area: Area::Tumour,
    fields: &[
        num("bilirubin_mg_dl", "Bilirubin", "mg/dL", 0.1, 50.0, 0.1),
        num("albumin_g_dl", "Albumin", "g/dL", 0.5, 6.0, 0.1),
    ],
    source: "Johnson 2015; book Lecture 18, p. 363",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let bili_umol = i.get_pos("bilirubin_mg_dl")? * 17.1;
    let alb_g_l = i.get_pos("albumin_g_dl")? * 10.0;
    let b = bili_umol.log10() * 0.66;
    let a = alb_g_l * -0.085;
    let score = b + a;
    let (band, label, meaning) = if score <= -2.60 {
        ("albi.1", "Grade 1 (≤ −2.60)", "Best liver function and survival group.")
    } else if score <= -1.39 {
        ("albi.2", "Grade 2 (> −2.60 to ≤ −1.39)", "Intermediate liver function group.")
    } else {
        ("albi.3", "Grade 3 (> −1.39)", "Worst liver function and survival group.")
    };
    Ok(Outcome {
        score: Some(score),
        display: show(score, 2),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(363),
        parts: vec![Part::new("Bilirubin term", b), Part::new("Albumin term", a)],
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(b: f64, a: f64) -> Outcome {
        compute(&Inputs::new().num("bilirubin_mg_dl", b).num("albumin_g_dl", a)).unwrap()
    }

    #[test]
    fn grades() {
        // Bili 1 mg/dL = 17.1 µmol/L: 0.66 · log10 17.1 = 0.814.
        // Albumin 4.2 g/dL: −3.57 → −2.76, grade 1.
        let o = run(1.0, 4.2);
        assert!((o.score.unwrap() - -2.756).abs() < 0.005);
        assert_eq!(o.band, "albi.1");
        // Albumin 4.0 g/dL: −3.40 → −2.59, just inside grade 2.
        assert_eq!(run(1.0, 4.0).band, "albi.2");
        // Bili 3 mg/dL, albumin 2.5 g/dL: 0.66·1.710 − 2.125 = −0.996 → grade 3.
        assert_eq!(run(3.0, 2.5).band, "albi.3");
    }
}
