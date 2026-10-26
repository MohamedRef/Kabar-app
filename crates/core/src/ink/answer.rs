//! Ink feedback for handwritten answers (write-mode lessons): which letters
//! to keep, circle, add or strike, so the correct form can be written in
//! under the learner's own.

/// What happened to one letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Written as expected.
    Keep,
    /// Written, but a different letter was expected: circle it.
    Wrong,
    /// Expected but not written: show a caret.
    Missing,
    /// Written but not expected: strike it.
    Extra,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LetterMark {
    pub mark: Mark,
    /// The expected letter (none for `Extra`).
    pub expected: Option<char>,
    /// The written letter (none for `Missing`).
    pub written: Option<char>,
    /// Index of the written letter in the written text (in chars), or where
    /// a missing letter would go.
    pub at: usize,
}

fn same(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

/// Aligns what was written with what was expected, letter by letter
/// (Levenshtein, case-insensitive; ties prefer a substitution, so one wrong
/// letter reads as one circle, not a strike plus a caret).
pub fn letter_diff(expected: &str, written: &str) -> Vec<LetterMark> {
    let e: Vec<char> = expected.chars().collect();
    let w: Vec<char> = written.chars().collect();
    let (n, m) = (e.len(), w.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = d[i - 1][j - 1] + usize::from(!same(e[i - 1], w[j - 1]));
            d[i][j] = sub.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
        }
    }
    let mut out = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + usize::from(!same(e[i - 1], w[j - 1])) {
            let mark = if same(e[i - 1], w[j - 1]) { Mark::Keep } else { Mark::Wrong };
            out.push(LetterMark { mark, expected: Some(e[i - 1]), written: Some(w[j - 1]), at: j - 1 });
            i -= 1;
            j -= 1;
        } else if j > 0 && d[i][j] == d[i][j - 1] + 1 {
            out.push(LetterMark { mark: Mark::Extra, expected: None, written: Some(w[j - 1]), at: j - 1 });
            j -= 1;
        } else {
            out.push(LetterMark { mark: Mark::Missing, expected: Some(e[i - 1]), written: None, at: j });
            i -= 1;
        }
    }
    out.reverse();
    out
}

/// Number of letters to fix.
pub fn mistakes(marks: &[LetterMark]) -> usize {
    marks.iter().filter(|m| m.mark != Mark::Keep).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(m: &[LetterMark]) -> String {
        m.iter()
            .map(|m| match m.mark {
                Mark::Keep => '.',
                Mark::Wrong => 'w',
                Mark::Missing => 'm',
                Mark::Extra => 'x',
            })
            .collect()
    }

    #[test]
    fn learner_slips_from_the_golden_set() {
        // berkerja for bekerja: one extra r.
        let m = letter_diff("bekerja", "berkerja");
        assert_eq!(kinds(&m), "..x.....");
        assert_eq!(m[2].written, Some('r'));
        assert_eq!(mistakes(&m), 1);

        // makananya for makanannya: one missing n.
        let m = letter_diff("makanannya", "makananya");
        assert_eq!(mistakes(&m), 1);
        assert_eq!(m.iter().find(|x| x.mark == Mark::Missing).unwrap().expected, Some('n'));

        // A wrong letter is one circle.
        let m = letter_diff("Saya sedang membaca buku.", "Saya sedang membava buku.");
        assert_eq!(mistakes(&m), 1);
        let w = m.iter().find(|x| x.mark == Mark::Wrong).unwrap();
        assert_eq!((w.expected, w.written, w.at), (Some('c'), Some('v'), 17));
    }

    #[test]
    fn case_is_not_a_mistake_and_empty_inputs_work() {
        assert_eq!(mistakes(&letter_diff("Apa kabar?", "apa Kabar?")), 0);
        assert_eq!(kinds(&letter_diff("abc", "")), "mmm");
        assert_eq!(kinds(&letter_diff("", "ab")), "xx");
        assert!(letter_diff("", "").is_empty());
    }
}
