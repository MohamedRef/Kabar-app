//! Ranson's criteria for acute pancreatitis.
//!
//! Criteria (Ranson 1974 non-gallstone; Ranson 1979 gallstone), one point
//! each. Non-gallstone at admission: age > 55, WBC > 16,000/mm³, glucose >
//! 200 mg/dL, LDH > 350 IU/L, AST > 250 IU/L; within 48 h: haematocrit fall >
//! 10 points, BUN rise > 5 mg/dL, calcium < 8 mg/dL, PaO₂ < 60 mmHg, base
//! deficit > 4 mEq/L, fluid sequestration > 6 L. Gallstone: age > 70,
//! WBC > 18,000, glucose > 220, LDH > 400, AST > 250; BUN rise > 2, base
//! deficit > 5, sequestration > 4 L, no PaO₂ criterion. The book prints the
//! criteria as an image; its mortality bands (Lecture 15, p. 288): ≤ 2 points
//! 0–3%, 3–4 15%, 5–6 40%, 7–11 nearly 100%.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, flag, num};

pub const SPEC: CalcSpec = CalcSpec {
    id: "ranson",
    name: "Ranson's criteria",
    area: Area::Pancreas,
    fields: &[
        flag("gallstone", "Gallstone pancreatitis"),
        num("age_years", "Age", "years", 0.0, 110.0, 1.0),
        num("wbc_per_mm3", "White cells", "/mm³", 0.0, 100000.0, 100.0),
        num("glucose_mg_dl", "Glucose", "mg/dL", 10.0, 1500.0, 1.0),
        num("ldh_iu_l", "LDH", "IU/L", 10.0, 10000.0, 1.0),
        num("ast_iu_l", "AST", "IU/L", 1.0, 10000.0, 1.0),
        num("hct_fall_points", "Haematocrit fall in 48 h", "points", 0.0, 40.0, 1.0).optional(),
        num("bun_rise_mg_dl", "BUN rise in 48 h", "mg/dL", -50.0, 100.0, 1.0)
            .optional()
            .hint("Urea rise ÷ 2.14."),
        num("calcium_mg_dl", "Calcium at 48 h", "mg/dL", 3.0, 15.0, 0.1).optional(),
        num("pao2_mm_hg", "PaO₂ at 48 h", "mmHg", 20.0, 600.0, 1.0).optional(),
        num("base_deficit_meq_l", "Base deficit at 48 h", "mEq/L", -20.0, 30.0, 1.0).optional(),
        num("fluid_sequestration_l", "Fluid sequestration in 48 h", "L", -10.0, 30.0, 0.5).optional(),
    ],
    source: "Ranson 1974/1979; book Lecture 15, p. 288 (mortality bands)",
    compute,
};

fn compute(i: &Inputs) -> Result<Outcome, CalcError> {
    let gs = i.get_flag("gallstone")?;
    let (age_c, wbc_c, glu_c, ldh_c, bun_c, bd_c, fluid_c) = if gs {
        (70.0, 18000.0, 220.0, 400.0, 2.0, 5.0, 4.0)
    } else {
        (55.0, 16000.0, 200.0, 350.0, 5.0, 4.0, 6.0)
    };
    let mut parts = vec![
        Part::met(format!("Age > {age_c}"), i.get_num("age_years")? > age_c),
        Part::met(format!("WBC > {wbc_c}/mm³"), i.get_num("wbc_per_mm3")? > wbc_c),
        Part::met(format!("Glucose > {glu_c} mg/dL"), i.get_num("glucose_mg_dl")? > glu_c),
        Part::met(format!("LDH > {ldh_c} IU/L"), i.get_num("ldh_iu_l")? > ldh_c),
        Part::met("AST > 250 IU/L", i.get_num("ast_iu_l")? > 250.0),
    ];
    let mut late: Vec<(String, Option<bool>)> = vec![
        ("Haematocrit fall > 10 points".into(), i.opt_num("hct_fall_points")?.map(|v| v > 10.0)),
        (format!("BUN rise > {bun_c} mg/dL"), i.opt_num("bun_rise_mg_dl")?.map(|v| v > bun_c)),
        ("Calcium < 8 mg/dL".into(), i.opt_num("calcium_mg_dl")?.map(|v| v < 8.0)),
        (format!("Base deficit > {bd_c} mEq/L"), i.opt_num("base_deficit_meq_l")?.map(|v| v > bd_c)),
        (
            format!("Fluid sequestration > {fluid_c} L"),
            i.opt_num("fluid_sequestration_l")?.map(|v| v > fluid_c),
        ),
    ];
    if !gs {
        late.insert(3, ("PaO₂ < 60 mmHg".into(), i.opt_num("pao2_mm_hg")?.map(|v| v < 60.0)));
    }
    let missing = late.iter().filter(|(_, v)| v.is_none()).count();
    parts.extend(late.into_iter().filter_map(|(l, v)| v.map(|m| Part::met(l, m))));
    let total = parts.iter().filter(|p| p.points > 0.0).count() as u32;

    let (band, label, meaning) = match total {
        0..=2 => ("ranson.0_2", "0–2 points", "Mortality 0–3%."),
        3..=4 => ("ranson.3_4", "3–4 points", "Mortality about 15%."),
        5..=6 => ("ranson.5_6", "5–6 points", "Mortality about 40%."),
        _ => ("ranson.7_11", "7–11 points", "Mortality nearly 100%."),
    };
    let mut notes = Vec::new();
    if missing > 0 {
        notes.push(format!("{missing} of the 48-hour criteria not entered: the score can only rise."));
    }
    Ok(Outcome {
        score: Some(total as f64),
        display: total.to_string(),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(288),
        parts,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admission() -> Inputs {
        Inputs::new()
            .num("age_years", 60.0)
            .num("wbc_per_mm3", 17000.0)
            .num("glucose_mg_dl", 150.0)
            .num("ldh_iu_l", 400.0)
            .num("ast_iu_l", 100.0)
    }

    #[test]
    fn admission_only_is_partial() {
        let o = compute(&admission()).unwrap();
        assert_eq!(o.score, Some(3.0));
        assert_eq!(o.band, "ranson.3_4");
        assert!(o.notes[0].starts_with("6 of"));
    }

    #[test]
    fn full_score() {
        let i = admission()
            .num("hct_fall_points", 12.0)
            .num("bun_rise_mg_dl", 6.0)
            .num("calcium_mg_dl", 7.5)
            .num("pao2_mm_hg", 70.0)
            .num("base_deficit_meq_l", 2.0)
            .num("fluid_sequestration_l", 3.0);
        let o = compute(&i).unwrap();
        assert_eq!(o.score, Some(6.0));
        assert_eq!(o.band, "ranson.5_6");
        assert!(o.notes.is_empty());
    }

    #[test]
    fn gallstone_cut_offs_are_higher() {
        // Age 60 and WBC 17,000 count for non-gallstone but not gallstone.
        let o = compute(&admission().flag("gallstone", true)).unwrap();
        assert_eq!(o.score, Some(0.0));
        assert!(o.notes[0].starts_with("5 of"));
    }
}
