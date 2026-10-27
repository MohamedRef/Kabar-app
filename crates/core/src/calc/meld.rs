//! MELD and MELD-Na.
//!
//! The book (Lecture 1, p. 14–15) names the inputs and the 6–40 range but
//! not the formula. Formula: UNOS/OPTN MELD policy as used 2016–2023
//! (Kamath et al., Hepatology 2001; Kim et al., NEJM 2008 for sodium):
//! values below 1 are set to 1, creatinine is capped at 4 mg/dL (and set to 4
//! with dialysis twice in the past week), sodium is bounded to 125–137, and
//! the result is capped at 40. MELD 3.0 (2023) is not used, because the book
//! and the exam use MELD and MELD-Na.

use super::{Area, CalcError, CalcSpec, Inputs, Outcome, Part, flag, num};

const FIELDS_MELD: &[super::Field] = &[
    num("bilirubin_mg_dl", "Bilirubin", "mg/dL", 0.1, 50.0, 0.1),
    num("inr", "INR", "", 0.5, 15.0, 0.1),
    num("creatinine_mg_dl", "Creatinine", "mg/dL", 0.1, 20.0, 0.1),
    flag("dialysis", "Dialysis twice in the past week"),
];

pub const MELD: CalcSpec = CalcSpec {
    id: "meld",
    name: "MELD",
    area: Area::Liver,
    fields: FIELDS_MELD,
    source: "UNOS/OPTN MELD (Kamath 2001); book Lecture 1, p. 14–15",
    compute: compute_meld,
};

pub const MELD_NA: CalcSpec = CalcSpec {
    id: "meld_na",
    name: "MELD-Na",
    area: Area::Liver,
    fields: &[
        num("bilirubin_mg_dl", "Bilirubin", "mg/dL", 0.1, 50.0, 0.1),
        num("inr", "INR", "", 0.5, 15.0, 0.1),
        num("creatinine_mg_dl", "Creatinine", "mg/dL", 0.1, 20.0, 0.1),
        flag("dialysis", "Dialysis twice in the past week"),
        num("sodium_mmol_l", "Sodium", "mmol/L", 100.0, 170.0, 1.0),
    ],
    source: "UNOS/OPTN MELD-Na 2016 (Kim 2008); book Lecture 1, p. 14–15",
    compute: compute_meld_na,
};

/// MELD(i) before rounding: 10 × (0.957 ln Cr + 0.378 ln bili + 1.120 ln INR + 0.643).
fn meld_i(i: &Inputs) -> Result<(f64, Vec<Part>, Vec<String>), CalcError> {
    let mut notes = Vec::new();
    let bili = i.get_pos("bilirubin_mg_dl")?.max(1.0);
    let inr = i.get_pos("inr")?.max(1.0);
    let mut cr = i.get_pos("creatinine_mg_dl")?.max(1.0);
    if i.get_flag("dialysis")? {
        cr = 4.0;
        notes.push("Dialysis: creatinine set to 4 mg/dL.".to_owned());
    } else if cr > 4.0 {
        cr = 4.0;
        notes.push("Creatinine capped at 4 mg/dL.".to_owned());
    }
    let parts = vec![
        Part::new("Creatinine term", 9.57 * cr.ln()),
        Part::new("Bilirubin term", 3.78 * bili.ln()),
        Part::new("INR term", 11.2 * inr.ln()),
        Part::new("Constant", 6.43),
    ];
    let v = 10.0 * (0.957 * cr.ln() + 0.378 * bili.ln() + 1.120 * inr.ln() + 0.643);
    Ok((v, parts, notes))
}

fn band(score: u32) -> (&'static str, &'static str, &'static str) {
    // Book Lecture 11 (p. 189): the Alexandria programme transplants at MELD ≥15,
    // discusses MELD >30 case by case; AASLD evaluation from MELD ≥10 (p. 188).
    match score {
        0..=9 => {
            ("meld.lt10", "MELD below 10", "Below the AASLD threshold (MELD ≥10) for transplant evaluation.")
        }
        10..=14 => (
            "meld.10_14",
            "MELD 10–14",
            "Refer for transplant evaluation (AASLD: MELD ≥10). Below the book's transplant threshold of 15.",
        ),
        15..=30 => (
            "meld.15_30",
            "MELD 15–30",
            "Transplant candidate by the book's programme rule (MELD 15 or higher).",
        ),
        _ => (
            "meld.gt30",
            "MELD above 30",
            "Very high short-term mortality; the book's programme discusses MELD >30 case by case.",
        ),
    }
}

fn finish(v: f64, parts: Vec<Part>, mut notes: Vec<String>) -> Outcome {
    let mut score = v.round().max(6.0);
    if score > 40.0 {
        score = 40.0;
        notes.push("Capped at 40.".to_owned());
    }
    let s = score as u32;
    let (band, label, meaning) = band(s);
    Outcome {
        score: Some(score),
        display: s.to_string(),
        band,
        label,
        meaning: meaning.to_owned(),
        page: Some(189),
        parts,
        notes,
    }
}

fn compute_meld(i: &Inputs) -> Result<Outcome, CalcError> {
    let (v, parts, notes) = meld_i(i)?;
    Ok(finish(v, parts, notes))
}

fn compute_meld_na(i: &Inputs) -> Result<Outcome, CalcError> {
    let (v, mut parts, mut notes) = meld_i(i)?;
    // UNOS: round MELD(i) to the tenth before the sodium step.
    let m = (v * 10.0).round() / 10.0;
    let na_in = i.get_num("sodium_mmol_l")?;
    let na = na_in.clamp(125.0, 137.0);
    if na != na_in {
        notes.push(format!("Sodium bounded to {na} mmol/L."));
    }
    let out = if m > 11.0 {
        let adj = 1.32 * (137.0 - na) - 0.033 * m * (137.0 - na);
        parts.push(Part::new("Sodium adjustment", adj));
        m + adj
    } else {
        notes.push("MELD 11 or below: no sodium adjustment.".to_owned());
        m
    };
    let mut o = finish(out, parts, notes);
    o.band = match o.band {
        "meld.lt10" => "meld_na.lt10",
        "meld.10_14" => "meld_na.10_14",
        "meld.15_30" => "meld_na.15_30",
        _ => "meld_na.gt30",
    };
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labs(b: f64, inr: f64, cr: f64) -> Inputs {
        Inputs::new().num("bilirubin_mg_dl", b).num("inr", inr).num("creatinine_mg_dl", cr)
    }

    #[test]
    fn normal_labs_give_the_floor_of_six() {
        assert_eq!(compute_meld(&labs(1.0, 1.0, 1.0)).unwrap().score, Some(6.0));
        // Values below 1 are raised to 1.
        assert_eq!(compute_meld(&labs(0.4, 0.9, 0.6)).unwrap().score, Some(6.0));
    }

    #[test]
    fn worked_example() {
        // Bili 4, INR 2, Cr 2: 10 × (0.957·0.6931 + 0.378·1.3863 + 1.12·0.6931 + 0.643) = 26.07 → 26.
        let o = compute_meld(&labs(4.0, 2.0, 2.0)).unwrap();
        assert_eq!(o.score, Some(26.0));
        assert_eq!(o.band, "meld.15_30");
    }

    #[test]
    fn creatinine_cap_and_dialysis() {
        let capped = compute_meld(&labs(4.0, 2.0, 7.0)).unwrap();
        let dial = compute_meld(&labs(4.0, 2.0, 1.0).flag("dialysis", true)).unwrap();
        assert_eq!(capped.score, dial.score);
        // 26.07 + 9.57 · ln 2 = 32.7 → 33.
        assert_eq!(dial.score, Some(33.0));
    }

    #[test]
    fn capped_at_forty() {
        let o = compute_meld(&labs(40.0, 8.0, 5.0)).unwrap();
        assert_eq!(o.score, Some(40.0));
    }

    #[test]
    fn meld_na_worked_example() {
        // MELD(i) 26.1 (rounded to the tenth), Na 130:
        // 26.1 + 1.32·7 − 0.033·26.1·7 = 26.1 + 9.24 − 6.03 = 29.31 → 29.
        let o = compute_meld_na(&labs(4.0, 2.0, 2.0).num("sodium_mmol_l", 130.0)).unwrap();
        assert_eq!(o.score, Some(29.0));
        assert_eq!(o.band, "meld_na.15_30");
    }

    #[test]
    fn meld_na_bounds_sodium_and_skips_low_meld() {
        let low = compute_meld_na(&labs(4.0, 2.0, 2.0).num("sodium_mmol_l", 110.0)).unwrap();
        let at125 = compute_meld_na(&labs(4.0, 2.0, 2.0).num("sodium_mmol_l", 125.0)).unwrap();
        assert_eq!(low.score, at125.score);
        let normal = compute_meld_na(&labs(1.0, 1.0, 1.0).num("sodium_mmol_l", 125.0)).unwrap();
        assert_eq!(normal.score, Some(6.0));
    }
}
