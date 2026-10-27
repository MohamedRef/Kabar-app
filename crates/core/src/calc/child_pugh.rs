//! Child-Pugh (Child-Turcotte-Pugh) classification.
//!
//! Source: the book, Lecture 1, Table 2, p. 14 (prothrombin time in seconds
//! prolonged). The INR alternative (<1.7 / 1.7–2.3 / >2.3) is the common
//! modern form (Pugh et al., Br J Surg 1973; AASLD practice guidance) and is
//! used only when PT is not entered.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, choice, num};

pub const HE: &[(&str, &str)] = &[("none", "None"), ("grade_1_2", "Stage 1–2"), ("grade_3_4", "Stage 3–4")];
pub const ASCITES: &[(&str, &str)] =
    &[("absent", "Absent"), ("mild_moderate", "Mild to moderate"), ("severe", "Severe")];

const HE_ALIASES: &[(&str, &[&str])] = &[
    ("none", &["absent", "no"]),
    ("grade_1_2", &["stage_1_2", "grade_1", "grade_2", "mild", "minimal"]),
    ("grade_3_4", &["stage_3_4", "grade_3", "grade_4", "severe", "coma"]),
];
const ASCITES_ALIASES: &[(&str, &[&str])] = &[
    ("absent", &["none", "no"]),
    ("mild_moderate", &["mild", "moderate", "slight", "controlled"]),
    ("severe", &["tense", "refractory", "gross"]),
];

pub const SPEC: CalcSpec = CalcSpec {
    id: "child_pugh",
    name: "Child-Pugh",
    area: Area::Liver,
    fields: &[
        choice("encephalopathy", "Encephalopathy", HE),
        choice("ascites", "Ascites", ASCITES),
        num("bilirubin_mg_dl", "Bilirubin", "mg/dL", 0.1, 50.0, 0.1),
        num("albumin_g_dl", "Albumin", "g/dL", 0.5, 6.0, 0.1),
        num("pt_prolonged_s", "Prothrombin time, seconds prolonged", "s", 0.0, 60.0, 0.5)
            .optional()
            .hint("The book's form. Leave empty to use INR."),
        num("inr", "INR", "", 0.5, 15.0, 0.1).optional().hint("Used only when PT is empty."),
    ],
    source: "Book Lecture 1, Table 2 (p. 14); INR bands: Pugh 1973 as used by AASLD",
    compute,
};

/// The 5–15 point total and its parts.
pub fn points(i: &Inputs) -> Result<(u32, Vec<Part>), CalcError> {
    let he = match i.get_option("encephalopathy", HE_ALIASES)? {
        "none" => 1,
        "grade_1_2" => 2,
        _ => 3,
    };
    let asc = match i.get_option("ascites", ASCITES_ALIASES)? {
        "absent" => 1,
        "mild_moderate" => 2,
        _ => 3,
    };
    let bili = i.get_num("bilirubin_mg_dl")?;
    let bili_p = if bili < 2.0 {
        1
    } else if bili <= 3.0 {
        2
    } else {
        3
    };
    let alb = i.get_num("albumin_g_dl")?;
    let alb_p = if alb > 3.5 {
        1
    } else if alb >= 2.8 {
        2
    } else {
        3
    };
    let (coag_label, coag_p) = match (i.opt_num("pt_prolonged_s")?, i.opt_num("inr")?) {
        (Some(pt), _) => (
            "Prothrombin time prolonged",
            if pt < 4.0 {
                1
            } else if pt <= 6.0 {
                2
            } else {
                3
            },
        ),
        (None, Some(inr)) => (
            "INR",
            if inr < 1.7 {
                1
            } else if inr <= 2.3 {
                2
            } else {
                3
            },
        ),
        (None, None) => return Err(CalcError::Missing("pt_prolonged_s")),
    };
    let parts = vec![
        Part::new("Encephalopathy", he as f64),
        Part::new("Ascites", asc as f64),
        Part::new("Bilirubin", bili_p as f64),
        Part::new("Albumin", alb_p as f64),
        Part::new(coag_label, coag_p as f64),
    ];
    Ok((he + asc + bili_p + alb_p + coag_p, parts))
}

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let (total, parts) = points(i)?;
    let (band, label, meaning) = match total {
        5..=6 => ("child_pugh.a", "Class A (5–6 points)", "1- and 2-year survival about 100% and 85%."),
        7..=9 => ("child_pugh.b", "Class B (7–9 points)", "1- and 2-year survival about 80% and 60%."),
        _ => ("child_pugh.c", "Class C (10–15 points)", "1- and 2-year survival about 45% and 35%."),
    };
    Ok(Outcome {
        score: Some(total as f64),
        display: total.to_string(),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(14),
        parts,
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Inputs {
        Inputs::new().choice("encephalopathy", "none").choice("ascites", "absent")
    }

    #[test]
    fn best_case_is_five_class_a() {
        let o =
            compute(&base().num("bilirubin_mg_dl", 1.0).num("albumin_g_dl", 4.0).num("pt_prolonged_s", 2.0))
                .unwrap();
        assert_eq!(o.score, Some(5.0));
        assert_eq!(o.band, "child_pugh.a");
    }

    #[test]
    fn book_boundaries() {
        // Bilirubin 2 and 3 are both 2 points; albumin 3.5 and 2.8 both 2; PT 4 and 6 both 2.
        let o =
            compute(&base().num("bilirubin_mg_dl", 3.0).num("albumin_g_dl", 2.8).num("pt_prolonged_s", 6.0))
                .unwrap();
        assert_eq!(o.score, Some(8.0));
        assert_eq!(o.band, "child_pugh.b");
        let o =
            compute(&base().num("bilirubin_mg_dl", 3.1).num("albumin_g_dl", 2.7).num("pt_prolonged_s", 6.5))
                .unwrap();
        assert_eq!(o.score, Some(11.0));
        assert_eq!(o.band, "child_pugh.c");
    }

    #[test]
    fn decompensated_example() {
        // Moderate ascites, grade 2 HE, bilirubin 2.5, albumin 3.0, INR 1.9 → 2+2+2+2+2 = 10, class C.
        let i = Inputs::new()
            .choice("encephalopathy", "grade_1_2")
            .choice("ascites", "mild_moderate")
            .num("bilirubin_mg_dl", 2.5)
            .num("albumin_g_dl", 3.0)
            .num("inr", 1.9);
        let o = compute(&i).unwrap();
        assert_eq!(o.score, Some(10.0));
        assert_eq!(o.band, "child_pugh.c");
        assert_eq!(o.parts[4].label, "INR");
    }

    #[test]
    fn pt_wins_over_inr_and_one_is_required() {
        let i = base().num("bilirubin_mg_dl", 1.0).num("albumin_g_dl", 4.0);
        assert_eq!(compute(&i), Err(CalcError::Missing("pt_prolonged_s")));
        let o = compute(&i.num("pt_prolonged_s", 1.0).num("inr", 3.0)).unwrap();
        assert_eq!(o.score, Some(5.0));
    }
}
