//! FIB-4 index.
//!
//! Formula: age × AST / (platelets [10⁹/L] × √ALT) (Sterling et al.,
//! Hepatology 2006). Bands from the book, Lecture 6, p. 102: < 1.3 low,
//! 1.3–3.25 intermediate, > 3.25 high risk of advanced fibrosis. Lecture 2
//! (EASL 2021) uses < 1.3 to rule out and refers above 1.3 for elastography.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, num, show};

pub const SPEC: CalcSpec = CalcSpec {
    id: "fib4",
    name: "FIB-4",
    area: Area::Liver,
    fields: &[
        num("age_years", "Age", "years", 1.0, 110.0, 1.0),
        num("ast_u_l", "AST", "U/L", 1.0, 10000.0, 1.0),
        num("alt_u_l", "ALT", "U/L", 1.0, 10000.0, 1.0),
        num("platelets_10e9_l", "Platelets", "×10⁹/L", 1.0, 1500.0, 1.0),
    ],
    source: "Sterling 2006; book Lecture 6, p. 102",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let age = i.get_pos("age_years")?;
    let ast = i.get_pos("ast_u_l")?;
    let alt = i.get_pos("alt_u_l")?;
    let plt = i.get_pos("platelets_10e9_l")?;
    let score = age * ast / (plt * alt.sqrt());
    let (band, label, meaning) = if score < 1.3 {
        ("fib4.low", "Low risk (< 1.3)", "Advanced fibrosis unlikely.")
    } else if score <= 3.25 {
        (
            "fib4.mid",
            "Intermediate (1.3–3.25)",
            "Indeterminate: go on to elastography (Fibroscan) or a patented test.",
        )
    } else {
        ("fib4.high", "High risk (> 3.25)", "Advanced fibrosis likely.")
    };
    let mut notes = Vec::new();
    if age < 35.0 || age > 65.0 {
        notes.push("Less accurate below 35 or above 65 years.".to_owned());
    }
    Ok(Outcome {
        score: Some(score),
        display: show(score, 2),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(102),
        parts: vec![Part::new("Age × AST", age * ast), Part::new("Platelets × √ALT", plt * alt.sqrt())],
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(age: f64, ast: f64, alt: f64, plt: f64) -> Outcome {
        compute(
            &Inputs::new()
                .num("age_years", age)
                .num("ast_u_l", ast)
                .num("alt_u_l", alt)
                .num("platelets_10e9_l", plt),
        )
        .unwrap()
    }

    #[test]
    fn worked_examples() {
        // 50 × 40 / (200 × 5) = 2.0.
        let o = run(50.0, 40.0, 25.0, 200.0);
        assert!((o.score.unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(o.band, "fib4.mid");
        // 40 × 20 / (250 × √36) = 0.53.
        assert_eq!(run(40.0, 20.0, 36.0, 250.0).band, "fib4.low");
        // 60 × 80 / (90 × √49) = 7.6.
        assert_eq!(run(60.0, 80.0, 49.0, 90.0).band, "fib4.high");
    }

    #[test]
    fn zero_platelets_is_an_error() {
        assert!(
            compute(
                &Inputs::new()
                    .num("age_years", 50.0)
                    .num("ast_u_l", 40.0)
                    .num("alt_u_l", 25.0)
                    .num("platelets_10e9_l", 0.0)
            )
            .is_err()
        );
    }
}
