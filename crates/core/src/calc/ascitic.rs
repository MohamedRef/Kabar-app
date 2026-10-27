//! Ascitic fluid: SBP and its variants, and secondary bacterial peritonitis.
//!
//! Source: the book, Lecture 9, p. 167–168. SBP: ascitic PMN ≥ 250/mm³ with
//! or without a positive culture. Variants: culture-positive neutrocytic
//! ascites, culture-negative neutrocytic ascites, and bacterascites (PMN
//! < 250/mm³ with a single organism). Secondary peritonitis is suspected with
//! total protein > 1.5 g/dL, PMN > 5,000/mm³, a polymicrobial culture, glucose
//! < 50 mg/dL, or ascitic LDH above serum LDH. (The book prints "50 mg/l";
//! Runyon's criterion is 50 mg/dL; see medstudy/book-issues.md.)

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, choice, flag, num};

const CULTURE: &[(&str, &str)] = &[
    ("pending", "Not sent or pending"),
    ("negative", "Negative"),
    ("monomicrobial", "Positive, one organism"),
    ("polymicrobial", "Positive, several organisms"),
];

pub const SPEC: CalcSpec = CalcSpec {
    id: "ascitic_pmn",
    name: "Ascitic PMN / SBP",
    area: Area::Ascites,
    fields: &[
        num("pmn_per_mm3", "Ascitic PMN (neutrophils)", "/mm³", 0.0, 100000.0, 1.0)
            .hint("Total WBC × neutrophil %."),
        choice("culture", "Ascitic culture", CULTURE),
        num("protein_g_dl", "Ascitic total protein", "g/dL", 0.0, 8.0, 0.1).optional(),
        num("glucose_mg_dl", "Ascitic glucose", "mg/dL", 0.0, 500.0, 1.0).optional(),
        flag("ldh_above_serum", "Ascitic LDH above serum LDH"),
    ],
    source: "Book Lecture 9, p. 167–168",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let pmn = i.get_num("pmn_per_mm3")?;
    let culture = i.get_choice("culture", &["pending", "negative", "monomicrobial", "polymicrobial"])?;
    let neutrocytic = pmn >= 250.0;

    let protein = i.opt_num("protein_g_dl")?;
    let glucose = i.opt_num("glucose_mg_dl")?;
    let secondary = [
        Part::met("Total protein > 1.5 g/dL", protein.is_some_and(|p| p > 1.5)),
        Part::met("PMN > 5,000/mm³", pmn > 5000.0),
        Part::met("Polymicrobial culture", culture == "polymicrobial"),
        Part::met("Glucose < 50 mg/dL", glucose.is_some_and(|g| g < 50.0)),
        Part::met("Ascitic LDH > serum LDH", i.get_flag("ldh_above_serum")?),
    ];
    let secondary_hits = secondary.iter().filter(|p| p.points > 0.0).count();

    let (band, label, meaning) = if neutrocytic && secondary_hits > 0 {
        (
            "ascitic.secondary",
            "Suspect secondary peritonitis",
            "Neutrocytic ascites with features of secondary bacterial peritonitis: look for a surgically treatable source (abscess, perforation).",
        )
    } else if neutrocytic && culture == "monomicrobial" {
        (
            "ascitic.sbp_culture_positive",
            "SBP: culture-positive neutrocytic ascites",
            "PMN ≥ 250/mm³ with a positive culture: treat as SBP.",
        )
    } else if neutrocytic && culture == "negative" {
        (
            "ascitic.sbp_culture_negative",
            "SBP: culture-negative neutrocytic ascites",
            "PMN ≥ 250/mm³, culture negative: treat as SBP; also consider TB peritonitis, carcinomatosis and fungal peritonitis.",
        )
    } else if neutrocytic {
        (
            "ascitic.sbp",
            "SBP (PMN ≥ 250/mm³)",
            "Diagnostic of SBP with or without a positive culture: start empirical antibiotics.",
        )
    } else if culture == "monomicrobial" {
        (
            "ascitic.bacterascites",
            "Bacterascites",
            "PMN < 250/mm³ with a single organism: may be resolving or evolving SBP.",
        )
    } else if culture == "polymicrobial" {
        (
            "ascitic.polymicrobial",
            "Polymicrobial, non-neutrocytic",
            "PMN < 250/mm³ with several organisms: suspect gut puncture during paracentesis or secondary peritonitis.",
        )
    } else {
        ("ascitic.no_sbp", "No SBP (PMN < 250/mm³)", "SBP is not diagnosed.")
    };

    let mut parts = vec![Part::met("PMN ≥ 250/mm³", neutrocytic)];
    parts.extend(secondary);
    Ok(Outcome {
        score: Some(pmn),
        display: format!("{pmn:.0}/mm³"),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(167),
        parts,
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(pmn: f64, culture: &str) -> Outcome {
        compute(&Inputs::new().num("pmn_per_mm3", pmn).choice("culture", culture)).unwrap()
    }

    #[test]
    fn variants() {
        assert_eq!(run(250.0, "pending").band, "ascitic.sbp");
        assert_eq!(run(600.0, "monomicrobial").band, "ascitic.sbp_culture_positive");
        assert_eq!(run(600.0, "negative").band, "ascitic.sbp_culture_negative");
        assert_eq!(run(120.0, "monomicrobial").band, "ascitic.bacterascites");
        assert_eq!(run(249.0, "negative").band, "ascitic.no_sbp");
    }

    #[test]
    fn secondary_peritonitis() {
        assert_eq!(run(800.0, "polymicrobial").band, "ascitic.secondary");
        assert_eq!(run(6000.0, "monomicrobial").band, "ascitic.secondary");
        let o = compute(
            &Inputs::new()
                .num("pmn_per_mm3", 900.0)
                .choice("culture", "pending")
                .num("protein_g_dl", 2.0)
                .num("glucose_mg_dl", 30.0),
        )
        .unwrap();
        assert_eq!(o.band, "ascitic.secondary");
        assert_eq!(o.parts.iter().filter(|p| p.points > 0.0).count(), 3);
    }
}
