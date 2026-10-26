//! Rupiah Reader (spec §14): read prices aloud fast,
//! Rp26.000.000 → *dua puluh enam juta rupiah*.

/// Indonesian words for a number: 26_000_000 → "dua puluh enam juta".
/// Uses the se- forms (sepuluh, sebelas, seratus, seribu, sejuta is "satu juta").
pub fn number_words(n: u64) -> String {
    let _ = n;
    todo!("m10 agent A")
}

/// `number_words` + " rupiah".
pub fn rupiah_words(n: u64) -> String {
    let _ = n;
    todo!("m10 agent A")
}

/// Reads number words back, tolerant of spacing and case ("dua puluh enam juta rupiah").
pub fn parse_number_words(s: &str) -> Option<u64> {
    let _ = s;
    todo!("m10 agent A")
}

/// Reads a price as written in shops and listings: "Rp26.000.000", "Rp 26 jt",
/// "26rb", "1,5 juta", "15k", "Rp. 7.500".
pub fn parse_price(s: &str) -> Option<u64> {
    let _ = s;
    todo!("m10 agent A")
}

/// Formats as shops write it: 26_000_000 → "Rp26.000.000".
pub fn format_rupiah(n: u64) -> String {
    let _ = n;
    todo!("m10 agent A")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Level {
    /// 100 – 999.
    Hundreds,
    /// Rp1.000 – Rp999.000: street food, angkot.
    Thousands,
    /// Rp1.000.000 and up: phones, rent.
    Millions,
    Mixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drill {
    pub amount: u64,
    /// As shown on screen, in one of the real-world styles ("Rp26.000.000", "26jt", "Rp 15rb").
    pub shown: String,
    /// The expected reading, e.g. "dua puluh enam juta rupiah".
    pub answer: String,
}

pub fn drills(level: Level, n: usize, seed: u64) -> Vec<Drill> {
    let _ = (level, n, seed);
    todo!("m10 agent A")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    Correct,
    /// Right number, small slip (missing "rupiah", spacing, "sejuta"): counts as correct.
    Close {
        expected: String,
    },
    /// A different number; `heard` is what the answer said, if readable.
    Wrong {
        expected: String,
        heard: Option<u64>,
    },
}

/// Checks a typed (or recognized) reading.
pub fn check(drill: &Drill, answer: &str) -> Check {
    let _ = (drill, answer);
    todo!("m10 agent A")
}

/// Points for a round: accuracy first, speed as a bonus, never negative.
pub fn score(correct: u32, total: u32, millis: u64) -> u32 {
    let _ = (correct, total, millis);
    todo!("m10 agent A")
}

/// Javanese (ngoko) number words for Pasar!: 3 → "telu", 25_000 → "selawe ewu".
/// `None` above what the table covers. Unverified until reviewed.
pub fn javanese_number_words(n: u64) -> Option<String> {
    let _ = n;
    todo!("m10 agent A")
}
