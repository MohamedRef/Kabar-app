//! Pure logic for `table` items: checking a typed cell and suggesting a grade.

use batna_core::review::Grade;

/// Lower-case, no spaces, one kind of dash, `>=`/`<=` as `≥`/`≤`, no final
/// full stop. "2.8 – 3.5" and "2.8-3.5" compare equal.
pub fn normalize(text: &str) -> String {
    let s = text.trim().trim_end_matches('.').to_lowercase().replace(">=", "≥").replace("<=", "≤");
    s.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '–' | '—' | '−' | '‐' => '-',
            other => other,
        })
        .collect()
}

/// Whether a typed cell matches the book's cell. Lenient on spacing, dashes
/// and case, and on a bracketed aside in the answer ("Absent (none)"); an
/// answer with " or " accepts either side. The learner can still overrule it.
pub fn matches(typed: &str, answer: &str) -> bool {
    let typed = normalize(typed);
    if typed.is_empty() {
        return false;
    }
    let without_aside = strip_brackets(answer);
    let mut accepted = vec![normalize(answer), normalize(&without_aside)];
    accepted.extend(answer.split(" or ").map(normalize));
    accepted.iter().any(|a| !a.is_empty() && *a == typed)
}

fn strip_brackets(text: &str) -> String {
    let mut depth = 0u32;
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// The grade to suggest from the cells recalled.
pub fn suggest(right: usize, total: usize) -> Grade {
    if right >= total {
        Grade::Good
    } else if right * 5 >= total * 3 {
        Grade::Hard
    } else {
        Grade::Again
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_cells_match_despite_spacing_dashes_and_case() {
        // Cells from the Child-Pugh table (fixtures/content/sample.yaml).
        assert!(matches("2.8 – 3.5", "2.8-3.5"));
        assert!(matches(" stage 1-2 ", "Stage 1-2"));
        assert!(matches(">= 3", "≥3"));
        assert!(matches("Mild to moderate.", "Mild to moderate"));
        assert!(matches("Absent", "Absent (none)"));
        assert!(matches("SBP", "SBP or HRS"));
        assert!(!matches("2-3", "2.8-3.5"));
        assert!(!matches("", "<2"));
        assert!(!matches("   ", "<2"));
    }

    #[test]
    fn suggestion_follows_the_share_of_cells_recalled() {
        assert_eq!(suggest(3, 3), Grade::Good);
        assert_eq!(suggest(2, 3), Grade::Hard);
        assert_eq!(suggest(3, 5), Grade::Hard);
        assert_eq!(suggest(1, 3), Grade::Again);
        assert_eq!(suggest(0, 0), Grade::Good);
    }
}
