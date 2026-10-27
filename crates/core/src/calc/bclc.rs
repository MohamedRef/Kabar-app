//! BCLC staging of hepatocellular carcinoma, as a decision table.
//!
//! Source: the book, Lecture 18, p. 362 (stages) and p. 364–373 (first-line
//! treatment). Preserved liver function is Child-Pugh A, or B up to 7 points
//! without ascites. Stages A and B need performance status 0; vascular
//! invasion or extrahepatic spread with PS ≤ 2 is C; PS > 2 or impaired liver
//! function is D. The 2022 BCLC update (Reig et al., J Hepatol 2022) differs:
//! a single nodule of any size is stage A and stage B has three subgroups.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, flag, num};

pub const SPEC: CalcSpec = CalcSpec {
    id: "bclc",
    name: "BCLC staging",
    area: Area::Tumour,
    fields: &[
        num("performance_status", "ECOG performance status", "", 0.0, 4.0, 1.0),
        num("child_pugh_points", "Child-Pugh points", "", 5.0, 15.0, 1.0),
        flag("ascites", "Ascites present"),
        num("nodules", "Number of nodules", "", 1.0, 20.0, 1.0),
        num("largest_cm", "Largest nodule", "cm", 0.1, 30.0, 0.1),
        flag("vascular_invasion", "Macrovascular invasion"),
        flag("extrahepatic", "Extrahepatic spread"),
    ],
    source: "Book Lecture 18, p. 362 and 364–373 (BCLC 2018)",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let ps = i.get_num("performance_status")?;
    let cp = i.get_num("child_pugh_points")?;
    let ascites = i.get_flag("ascites")?;
    let nodules = i.get_num("nodules")?;
    let largest = i.get_num("largest_cm")?;
    let invasion = i.get_flag("vascular_invasion")?;
    let spread = i.get_flag("extrahepatic")?;

    let preserved = cp <= 6.0 || (cp <= 7.0 && !ascites);
    let very_early = nodules <= 1.0 && largest <= 2.0;
    let early = (nodules <= 1.0 && largest <= 5.0) || (nodules <= 3.0 && largest <= 3.0);

    let (band, label, meaning, page) = if !preserved || ps > 2.0 {
        (
            "bclc.d",
            "Stage D: end stage",
            "Hospice care; liver transplantation only in selected Child C patients with PS ≤ 1 within Milan criteria.",
            373,
        )
    } else if invasion || spread || ps > 0.0 {
        (
            "bclc.c",
            "Stage C: advanced",
            "First line: systemic therapy (atezolizumab plus bevacizumab preferred). Second line: TARE.",
            370,
        )
    } else if very_early {
        (
            "bclc.0",
            "Stage 0: very early",
            "First line: resection or thermal ablation; transplant if not a resection candidate.",
            364,
        )
    } else if early {
        (
            "bclc.a",
            "Stage A: early",
            "First line: resection or thermal ablation; transplant within Milan criteria.",
            364,
        )
    } else {
        ("bclc.b", "Stage B: intermediate", "First line: TACE or TARE; consider percutaneous ablation.", 369)
    };

    Ok(Outcome {
        score: None,
        display: label.split(':').next().unwrap_or(label).to_owned(),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(page),
        parts: vec![
            Part::met("Preserved liver function", preserved),
            Part::new("Performance status", ps),
            Part::met("Vascular invasion or extrahepatic spread", invasion || spread),
            Part::met("Within early-stage tumour limits", early),
        ],
        notes: vec!["BCLC 2022 differs: a single nodule of any size is stage A.".to_owned()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Inputs {
        Inputs::new()
            .num("performance_status", 0.0)
            .num("child_pugh_points", 5.0)
            .num("nodules", 1.0)
            .num("largest_cm", 1.8)
    }

    fn stage(i: Inputs) -> &'static str {
        compute(&i).unwrap().band
    }

    #[test]
    fn stages_from_the_book() {
        assert_eq!(stage(base()), "bclc.0");
        assert_eq!(stage(base().num("largest_cm", 4.5)), "bclc.a");
        assert_eq!(stage(base().num("nodules", 3.0).num("largest_cm", 3.0)), "bclc.a");
        assert_eq!(stage(base().num("nodules", 4.0).num("largest_cm", 2.0)), "bclc.b");
        assert_eq!(stage(base().num("nodules", 2.0).num("largest_cm", 4.0)), "bclc.b");
        assert_eq!(stage(base().num("largest_cm", 6.0)), "bclc.b");
        assert_eq!(stage(base().flag("vascular_invasion", true)), "bclc.c");
        assert_eq!(stage(base().num("performance_status", 1.0)), "bclc.c");
        assert_eq!(stage(base().num("performance_status", 3.0)), "bclc.d");
    }

    #[test]
    fn liver_function_rule() {
        // Child-Pugh B7 without ascites is preserved; with ascites it is not.
        assert_eq!(stage(base().num("child_pugh_points", 7.0)), "bclc.0");
        assert_eq!(stage(base().num("child_pugh_points", 7.0).flag("ascites", true)), "bclc.d");
        assert_eq!(stage(base().num("child_pugh_points", 8.0)), "bclc.d");
    }
}
