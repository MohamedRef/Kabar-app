//! Rupiah Reader (spec §14): read prices aloud fast,
//! Rp26.000.000 → *dua puluh enam juta rupiah*.

use super::Rng;

const UNITS: [&str; 10] =
    ["nol", "satu", "dua", "tiga", "empat", "lima", "enam", "tujuh", "delapan", "sembilan"];

/// (value, word) for the big scales, largest first.
const SCALES: [(u64, &str); 4] =
    [(1_000_000_000_000, "triliun"), (1_000_000_000, "miliar"), (1_000_000, "juta"), (1_000, "ribu")];

/// Words for 1..=999 ("" for 0).
fn below_1000(n: u64) -> String {
    debug_assert!(n < 1000);
    let mut parts: Vec<String> = Vec::new();
    let (h, rest) = (n / 100, n % 100);
    match h {
        0 => {}
        1 => parts.push("seratus".into()),
        h => parts.push(format!("{} ratus", UNITS[h as usize])),
    }
    match rest {
        0 => {}
        10 => parts.push("sepuluh".into()),
        11 => parts.push("sebelas".into()),
        12..=19 => parts.push(format!("{} belas", UNITS[(rest - 10) as usize])),
        1..=9 => parts.push(UNITS[rest as usize].into()),
        _ => {
            parts.push(format!("{} puluh", UNITS[(rest / 10) as usize]));
            if rest % 10 > 0 {
                parts.push(UNITS[(rest % 10) as usize].into());
            }
        }
    }
    parts.join(" ")
}

/// Indonesian words for a number: 26_000_000 → "dua puluh enam juta".
/// Uses the se- forms (sepuluh, sebelas, seratus, seribu, sejuta is "satu juta").
pub fn number_words(n: u64) -> String {
    if n == 0 {
        return UNITS[0].into();
    }
    let mut parts: Vec<String> = Vec::new();
    let mut rest = n;
    for (i, (scale, word)) in SCALES.iter().enumerate() {
        let k = rest / scale;
        rest %= scale;
        if k == 0 {
            continue;
        }
        if i == 0 && k >= 1000 {
            // Beyond 999 triliun: "seribu triliun".
            parts.push(format!("{} {word}", number_words(k)));
        } else if *word == "ribu" && k == 1 {
            parts.push("seribu".into());
        } else {
            parts.push(format!("{} {word}", below_1000(k)));
        }
    }
    if rest > 0 {
        parts.push(below_1000(rest));
    }
    parts.join(" ")
}

/// `number_words` + " rupiah".
pub fn rupiah_words(n: u64) -> String {
    format!("{} rupiah", number_words(n))
}

/// Lowercases, turns punctuation into spaces and collapses whitespace.
fn normalize(s: &str) -> String {
    let cleaned: String =
        s.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' }).collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reads number words back, tolerant of spacing and case ("dua puluh enam juta rupiah").
///
/// Also accepts "sejuta"/"semiliar", "satu ribu", "se ratus", "setengah juta"
/// (500.000), "satu setengah juta" (1.500.000), "milyar", a "rupiah"/"rp"
/// anywhere, and digit groups below 1000 between words ("26 juta"), which is
/// how speech recognizers often write what was said.
pub fn parse_number_words(s: &str) -> Option<u64> {
    let norm = normalize(s);
    let mut tokens: Vec<&str> = Vec::new();
    for t in norm.split(' ') {
        match t {
            "" | "rupiah" | "rp" | "idr" => {}
            "sepuluh" => tokens.extend(["satu", "puluh"]),
            "sebelas" => tokens.extend(["satu", "belas"]),
            "seratus" => tokens.extend(["satu", "ratus"]),
            "seribu" => tokens.extend(["satu", "ribu"]),
            "sejuta" => tokens.extend(["satu", "juta"]),
            "semiliar" | "semilyar" => tokens.extend(["satu", "miliar"]),
            "setriliun" => tokens.extend(["satu", "triliun"]),
            "se" => tokens.push("satu"),
            t => tokens.push(t),
        }
    }
    if tokens.is_empty() {
        return None;
    }
    if tokens == ["nol"] {
        return Some(0);
    }

    // `last` is the place value filled most recently inside the current
    // group below 1000: 4 = nothing yet, 3 = hundreds, 2 = tens, 1 = units.
    let mut total: u64 = 0;
    let mut group: u64 = 0;
    let mut last = 4u8;
    let mut unit: Option<u64> = None;
    let mut half = false;
    let mut last_scale: u64 = 0;

    fn flush_unit(group: &mut u64, last: &mut u8, unit: &mut Option<u64>) -> Option<()> {
        if let Some(u) = unit.take() {
            if *last < 2 {
                return None;
            }
            *group += u;
            *last = 1;
        }
        Some(())
    }

    for t in tokens {
        if let Some(d) = UNITS.iter().position(|u| *u == t) {
            if d == 0 || unit.is_some() || half {
                return None;
            }
            unit = Some(d as u64);
        } else if let Ok(v) = t.parse::<u64>() {
            // A digit group from a recognizer: "26 juta", "2 ribu 500".
            if v >= 1000 || unit.is_some() || last != 4 || half {
                return None;
            }
            group = v;
            last = 0;
        } else {
            match t {
                "belas" => {
                    let u = unit.take()?;
                    if last < 3 {
                        return None;
                    }
                    group += 10 + u;
                    last = 0;
                }
                "puluh" => {
                    let u = unit.take()?;
                    if last < 3 {
                        return None;
                    }
                    group += 10 * u;
                    last = 2;
                }
                "ratus" => {
                    let u = unit.take()?;
                    if last < 4 {
                        return None;
                    }
                    group += 100 * u;
                    last = 3;
                }
                "setengah" => {
                    if half {
                        return None;
                    }
                    flush_unit(&mut group, &mut last, &mut unit)?;
                    half = true;
                }
                _ => {
                    let scale = match t {
                        "ribu" => 1_000,
                        "juta" => 1_000_000,
                        "miliar" | "milyar" => 1_000_000_000,
                        "triliun" => 1_000_000_000_000,
                        _ => return None,
                    };
                    flush_unit(&mut group, &mut last, &mut unit)?;
                    if last_scale != 0 && scale > last_scale {
                        // "seribu triliun": a bigger scale multiplies everything before it.
                        if half {
                            return None;
                        }
                        total = total.checked_add(group)?.checked_mul(scale)?;
                    } else {
                        if scale == last_scale || (group == 0 && !half) {
                            return None;
                        }
                        let value =
                            group.checked_mul(scale)?.checked_add(if half { scale / 2 } else { 0 })?;
                        total = total.checked_add(value)?;
                    }
                    last_scale = scale;
                    group = 0;
                    last = 4;
                    half = false;
                }
            }
        }
    }
    if half {
        return None;
    }
    flush_unit(&mut group, &mut last, &mut unit)?;
    total.checked_add(group)
}

/// Splits a numeral with separators into an exact value times `scale`.
/// `.` groups thousands when followed by exactly three digits (and always
/// when there is no suffix); otherwise `.` or `,` is a decimal point.
fn numeral(s: &str, scale: u64) -> Option<u64> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
        return None;
    }
    if !s.starts_with(|c: char| c.is_ascii_digit()) || !s.ends_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let digits = |p: &str| -> Option<u64> {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) { None } else { p.parse().ok() }
    };
    let groups_of_three = |sep: char| -> Option<u64> {
        let mut parts = s.split(sep);
        let first = parts.next()?;
        if first.len() > 3 {
            return None;
        }
        let mut v = digits(first)?;
        for p in parts {
            if p.len() != 3 {
                return None;
            }
            v = v.checked_mul(1000)?.checked_add(digits(p)?)?;
        }
        v.checked_mul(scale)
    };
    let dots = s.matches('.').count();
    let commas = s.matches(',').count();
    match (dots, commas) {
        (0, 0) => digits(s)?.checked_mul(scale),
        (_, 0) if dots > 1 || scale == 1 || s.rsplit('.').next()?.len() == 3 => groups_of_three('.'),
        (0, _) if commas > 1 || (scale == 1 && s.rsplit(',').next()?.len() == 3) => groups_of_three(','),
        (0, 1) | (1, 0) => {
            let (int, frac) = s.split_once([',', '.'])?;
            let int = digits(int)?;
            let f = digits(frac)?;
            let pow = 10u64.checked_pow(frac.len() as u32)?;
            let frac_value = f.checked_mul(scale)?;
            if frac_value % pow != 0 {
                return None;
            }
            int.checked_mul(scale)?.checked_add(frac_value / pow)
        }
        _ => {
            // "1.500,5" style: dots group thousands, the comma is decimal.
            let (int, frac) = s.split_once(',')?;
            let int = numeral(int, 1)?;
            let pow = 10u64.checked_pow(frac.len() as u32)?;
            let frac_value = digits(frac)?.checked_mul(scale)?;
            if frac_value % pow != 0 {
                return None;
            }
            int.checked_mul(scale)?.checked_add(frac_value / pow)
        }
    }
}

/// The multiplier for a written suffix: "jt" → 1.000.000. Indonesian
/// listings use "M" for miliar (not million), so "2M" is 2.000.000.000.
fn suffix_scale(s: &str) -> Option<u64> {
    Some(match s {
        "" => 1,
        "rb" | "ribu" | "k" | "rbu" => 1_000,
        "jt" | "juta" | "jt." => 1_000_000,
        "m" | "miliar" | "milyar" | "mlr" | "b" => 1_000_000_000,
        "t" | "triliun" => 1_000_000_000_000,
        _ => return None,
    })
}

/// Reads a price as written in shops and listings: "Rp26.000.000", "Rp 26 jt",
/// "26rb", "1,5 juta", "15k", "Rp. 7.500".
///
/// Also: "Rp 26.000.000,-", "Rp7.500,00", "IDR 50.000", "1.5jt", "2M" (miliar),
/// compound "1jt 500rb" and "1jt500" (the trailing number is in ribu), and
/// written-out words via [`parse_number_words`].
pub fn parse_price(s: &str) -> Option<u64> {
    let mut t = s.trim().to_lowercase();
    for prefix in ["idr", "rp.", "rp"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest.trim_start().to_string();
            break;
        }
    }
    for suffix in [",-", ".-", ",00", "-", "rupiah"] {
        if let Some(rest) = t.strip_suffix(suffix) {
            t = rest.trim_end().to_string();
        }
    }
    if t.is_empty() {
        return None;
    }
    if t.chars().any(|c| c.is_ascii_alphabetic()) && t.contains(' ') && parse_number_words(&t).is_some() {
        return parse_number_words(&t);
    }

    // Split into (numeral, suffix) segments: "1jt 500rb" → [("1","jt"), ("500","rb")].
    let mut segments: Vec<(String, String)> = Vec::new();
    let mut chars = t.chars().peekable();
    while chars.peek().is_some() {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let mut num = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || c == '.' || c == ',' {
                num.push(c);
                chars.next();
            } else {
                break;
            }
        }
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let mut suf = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_alphabetic() || (c == '.' && !suf.is_empty()) {
                suf.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if num.is_empty() {
            return if segments.is_empty() { parse_number_words(&t) } else { None };
        }
        segments.push((num, suf.trim_end_matches('.').to_string()));
    }

    let mut total: u64 = 0;
    let mut prev_scale = u64::MAX;
    let count = segments.len();
    for (i, (num, suf)) in segments.iter().enumerate() {
        let mut scale = suffix_scale(suf)?;
        if suf.is_empty() && count > 1 {
            // "1jt500": a bare trailing number is in the next scale down.
            if i != count - 1 || prev_scale < 1_000 {
                return None;
            }
            scale = prev_scale / 1000;
            if num.len() > 3 || num.contains(['.', ',']) {
                return None;
            }
        }
        if scale >= prev_scale {
            return None;
        }
        total = total.checked_add(numeral(num, scale)?)?;
        prev_scale = scale;
    }
    Some(total)
}

/// Thousands with dots: 26_000_000 → "26.000.000".
fn dotted(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// Formats as shops write it: 26_000_000 → "Rp26.000.000".
pub fn format_rupiah(n: u64) -> String {
    format!("Rp{}", dotted(n))
}

/// `n / unit` with up to two decimals and a decimal comma ("2,75"), if exact.
fn short_decimal(n: u64, unit: u64) -> Option<String> {
    let hundredths = unit / 100;
    if hundredths == 0 || !n.is_multiple_of(hundredths) {
        return None;
    }
    let (int, frac) = (n / unit, (n % unit) / hundredths);
    Some(match frac {
        0 => int.to_string(),
        f if f % 10 == 0 => format!("{int},{}", f / 10),
        f => format!("{int},{f:02}"),
    })
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

impl Level {
    pub const ALL: [Self; 4] = [Self::Hundreds, Self::Thousands, Self::Millions, Self::Mixed];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drill {
    pub amount: u64,
    /// As shown on screen, in one of the real-world styles ("Rp26.000.000", "26jt", "Rp 15rb").
    pub shown: String,
    /// The expected reading, e.g. "dua puluh enam juta rupiah".
    pub answer: String,
}

/// A realistic amount for a level.
fn amount(level: Level, rng: &mut Rng) -> u64 {
    let r = rng.below(10) as u64;
    let k = |rng: &mut Rng, lo: u64, hi: u64| lo + rng.below((hi - lo + 1) as usize) as u64;
    match level {
        // Coins and parking: mostly round fifties, sometimes any number to read.
        Level::Hundreds if r < 7 => 50 * k(rng, 2, 19),
        Level::Hundreds => k(rng, 101, 999),
        // Gorengan, angkot, nasi goreng: Rp1.000–Rp20.000 in 500s.
        Level::Thousands if r < 4 => 500 * k(rng, 2, 40),
        // Meals, shirts: Rp20.000–Rp100.000 in 1.000s.
        Level::Thousands if r < 7 => 1_000 * k(rng, 20, 100),
        // Shoes, groceries: Rp100.000–Rp995.000 in 5.000s.
        Level::Thousands if r < 9 => 5_000 * k(rng, 20, 199),
        // Shop prices like Rp49.900.
        Level::Thousands => 5_000 * k(rng, 2, 20) - 100,
        // Phones, rent, school fees: Rp1.000.000–Rp10.000.000 in 50.000s.
        Level::Millions if r < 4 => 50_000 * k(rng, 20, 200),
        // Motorbikes, a good laptop: Rp10.000.000–Rp35.000.000 in 500.000s.
        Level::Millions if r < 7 => 500_000 * k(rng, 20, 70),
        // A used car: Rp35.000.000–Rp300.000.000 in millions.
        Level::Millions if r < 9 => 1_000_000 * k(rng, 35, 300),
        // A house: Rp500.000.000–Rp3.000.000.000 in 50.000.000s.
        Level::Millions => 50_000_000 * k(rng, 10, 60),
        Level::Mixed => {
            let level =
                [Level::Hundreds, Level::Thousands, Level::Thousands, Level::Millions, Level::Millions]
                    [rng.below(5)];
            amount(level, rng)
        }
    }
}

/// One of the ways the amount is written in shops, chats and listings.
fn shown(n: u64, rng: &mut Rng) -> String {
    let d = dotted(n);
    let mut styles = vec![format!("Rp{d}"), format!("Rp {d}")];
    if n >= 1_000 {
        styles.push(format!("Rp. {d}"));
        styles.push(format!("Rp{d},-"));
    }
    if (1_000..1_000_000).contains(&n) && n.is_multiple_of(1_000) {
        let k = n / 1_000;
        styles.extend([format!("{k}rb"), format!("{k}k"), format!("Rp {k}rb"), format!("{k} ribu")]);
    }
    if (1_000_000..1_000_000_000).contains(&n)
        && let Some(x) = short_decimal(n, 1_000_000)
    {
        styles.extend([format!("{x}jt"), format!("{x} jt"), format!("Rp {x}jt"), format!("{x} juta")]);
    }
    if n >= 1_000_000_000
        && let Some(x) = short_decimal(n, 1_000_000_000)
    {
        styles.extend([format!("{x}M"), format!("Rp {x}M"), format!("{x} miliar")]);
    }
    styles.swap_remove(rng.below(styles.len()))
}

/// `n` drills for a level, all different, deterministic for `seed`.
pub fn drills(level: Level, n: usize, seed: u64) -> Vec<Drill> {
    let mut rng = Rng::new(seed);
    let mut out: Vec<Drill> = Vec::with_capacity(n);
    let mut tries = 0;
    while out.len() < n {
        let a = amount(level, &mut rng);
        tries += 1;
        if out.iter().any(|d| d.amount == a) && tries < n * 20 {
            continue;
        }
        let shown = shown(a, &mut rng);
        out.push(Drill { amount: a, shown, answer: rupiah_words(a) });
    }
    out
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

impl Check {
    /// Correct or Close.
    pub fn is_right(&self) -> bool {
        !matches!(self, Self::Wrong { .. })
    }
}

/// Checks a typed (or recognized) reading.
pub fn check(drill: &Drill, answer: &str) -> Check {
    if normalize(answer) == normalize(&drill.answer) {
        return Check::Correct;
    }
    let heard = parse_number_words(answer);
    if heard == Some(drill.amount) {
        Check::Close { expected: drill.answer.clone() }
    } else {
        Check::Wrong { expected: drill.answer.clone(), heard }
    }
}

/// Points per right answer.
pub const POINTS_PER_CORRECT: u32 = 10;
/// Most bonus points per right answer, for answering in [`FAST_MILLIS`] or less.
pub const MAX_SPEED_BONUS: u32 = 5;
/// Average time per drill that earns the whole speed bonus.
pub const FAST_MILLIS: u64 = 3_000;
/// Average time per drill at which the speed bonus reaches zero.
pub const SLOW_MILLIS: u64 = 10_000;

/// Points for a round: accuracy first, speed as a bonus, never negative.
///
/// Each right answer (Correct or Close) is worth [`POINTS_PER_CORRECT`]. On
/// top, each right answer earns up to [`MAX_SPEED_BONUS`]: all of it when the
/// average time per drill (`millis / total`) is at most [`FAST_MILLIS`],
/// falling linearly to nothing at [`SLOW_MILLIS`]. The bonus scales with right
/// answers, so rushing through wrong answers earns nothing.
pub fn score(correct: u32, total: u32, millis: u64) -> u32 {
    if total == 0 {
        return 0;
    }
    let correct = correct.min(total);
    let avg = millis / u64::from(total);
    let bonus_per = if avg <= FAST_MILLIS {
        u64::from(MAX_SPEED_BONUS) * 100
    } else if avg >= SLOW_MILLIS {
        0
    } else {
        u64::from(MAX_SPEED_BONUS) * 100 * (SLOW_MILLIS - avg) / (SLOW_MILLIS - FAST_MILLIS)
    };
    let bonus = (u64::from(correct) * bonus_per + 50) / 100;
    correct * POINTS_PER_CORRECT + bonus as u32
}

const JV_UNITS: [&str; 10] = ["", "siji", "loro", "telu", "papat", "lima", "enem", "pitu", "wolu", "sanga"];
/// The linking forms used before puluh, atus, ewu, yuta: rong puluh, telung atus.
const JV_LINKED: [&str; 10] =
    ["", "", "rong", "telung", "patang", "limang", "nem", "pitung", "wolung", "sangang"];
const JV_TEENS: [&str; 10] = [
    "sepuluh", "sewelas", "rolas", "telulas", "patbelas", "limalas", "nembelas", "pitulas", "wolulas",
    "sangalas",
];
const JV_LIKUR: [&str; 9] = [
    "selikur",
    "rolikur",
    "telulikur",
    "patlikur",
    "selawe",
    "nemlikur",
    "pitulikur",
    "wolulikur",
    "sangalikur",
];

fn jv_below_100(n: u64) -> String {
    let (t, u) = ((n / 10) as usize, (n % 10) as usize);
    match n {
        0..=9 => JV_UNITS[u].into(),
        10..=19 => JV_TEENS[u].into(),
        20 => "rong puluh".into(),
        21..=29 => JV_LIKUR[u - 1].into(),
        _ => {
            let tens = match t {
                5 => "seket".to_string(),
                6 => "sewidak".to_string(),
                t => format!("{} puluh", JV_LINKED[t]),
            };
            if u == 0 { tens } else { format!("{tens} {}", JV_UNITS[u]) }
        }
    }
}

fn jv_below_1000(n: u64) -> String {
    let (h, rest) = (n / 100, n % 100);
    let hundreds = match h {
        0 => String::new(),
        1 => "satus".into(),
        h => format!("{} atus", JV_LINKED[h as usize]),
    };
    match (hundreds.is_empty(), rest) {
        (_, 0) => hundreds,
        (true, r) => jv_below_100(r),
        (false, r) => format!("{hundreds} {}", jv_below_100(r)),
    }
}

/// `k` thousands or millions: sewu, rong ewu, rolas ewu, selawe ewu, satus seket ewu.
fn jv_scaled(k: u64, one: &str, word: &str) -> String {
    match k {
        1 => one.into(),
        2..=9 => format!("{} {word}", JV_LINKED[k as usize]),
        k => format!("{} {word}", jv_below_1000(k)),
    }
}

/// Javanese (ngoko) number words for Pasar!: 3 → "telu", 25_000 → "selawe ewu".
/// `None` above what the table covers. Unverified until reviewed.
///
/// Covers 0 to 999.999.999: the units (siji … sanga), sewelas … sangalas,
/// the likur forms (selikur … sangalikur, with selawe for 25), seket (50) and
/// sewidak (60), satus / rong atus, sewu / rong ewu and sayuta / rong yuta.
/// Javanese content stays flagged unverified until the learner approves it.
pub fn javanese_number_words(n: u64) -> Option<String> {
    if n >= 1_000_000_000 {
        return None;
    }
    if n == 0 {
        return Some("nol".into());
    }
    let (m, th, rest) = (n / 1_000_000, (n / 1_000) % 1_000, n % 1_000);
    let mut parts: Vec<String> = Vec::new();
    if m > 0 {
        parts.push(jv_scaled(m, "sayuta", "yuta"));
    }
    if th > 0 {
        parts.push(jv_scaled(th, "sewu", "ewu"));
    }
    if rest > 0 {
        parts.push(jv_below_1000(rest));
    }
    Some(parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_words_reads_like_a_local() {
        let cases: &[(u64, &str)] = &[
            (0, "nol"),
            (1, "satu"),
            (7, "tujuh"),
            (10, "sepuluh"),
            (11, "sebelas"),
            (12, "dua belas"),
            (19, "sembilan belas"),
            (20, "dua puluh"),
            (21, "dua puluh satu"),
            (99, "sembilan puluh sembilan"),
            (100, "seratus"),
            (101, "seratus satu"),
            (110, "seratus sepuluh"),
            (111, "seratus sebelas"),
            (250, "dua ratus lima puluh"),
            (999, "sembilan ratus sembilan puluh sembilan"),
            (1_000, "seribu"),
            (1_001, "seribu satu"),
            (1_500, "seribu lima ratus"),
            (2_000, "dua ribu"),
            (7_500, "tujuh ribu lima ratus"),
            (11_000, "sebelas ribu"),
            (12_000, "dua belas ribu"),
            (100_000, "seratus ribu"),
            (125_000, "seratus dua puluh lima ribu"),
            (1_000_000, "satu juta"),
            (1_500_000, "satu juta lima ratus ribu"),
            (2_750_000, "dua juta tujuh ratus lima puluh ribu"),
            (26_000_000, "dua puluh enam juta"),
            (101_000_000, "seratus satu juta"),
            (1_000_000_000, "satu miliar"),
            (1_250_000_000, "satu miliar dua ratus lima puluh juta"),
            (1_000_000_000_000, "satu triliun"),
            (5_000_000_001_011, "lima triliun seribu sebelas"),
            (1_000_000_000_000_000, "seribu triliun"),
        ];
        for (n, words) in cases {
            assert_eq!(number_words(*n), *words, "{n}");
        }
        assert_eq!(rupiah_words(26_000_000), "dua puluh enam juta rupiah");
        assert_eq!(rupiah_words(7_500), "tujuh ribu lima ratus rupiah");
    }

    #[test]
    fn parse_number_words_round_trips() {
        let mut rng = Rng::new(42);
        let mut values: Vec<u64> = (0..=2_100).collect();
        for _ in 0..3_000 {
            let digits = 1 + rng.below(18) as u32;
            values.push(rng.next_u64() % 10u64.pow(digits));
        }
        values.extend([u64::MAX, 1_000_000_000_000_000, 999_999_999_999_999]);
        for n in values {
            assert_eq!(parse_number_words(&number_words(n)), Some(n), "{n}: {}", number_words(n));
            assert_eq!(parse_number_words(&rupiah_words(n)), Some(n), "{n}");
        }
    }

    #[test]
    fn parse_number_words_is_tolerant() {
        let cases: &[(&str, u64)] = &[
            ("  Dua  Puluh Enam JUTA rupiah ", 26_000_000),
            ("dua puluh enam juta", 26_000_000),
            ("sejuta", 1_000_000),
            ("sejuta lima ratus ribu", 1_500_000),
            ("satu ribu", 1_000),
            ("se ratus ribu", 100_000),
            ("Rp seribu", 1_000),
            ("dua-puluh ribu", 20_000),
            ("satu belas", 11),
            ("setengah juta", 500_000),
            ("satu setengah juta", 1_500_000),
            ("dua setengah juta rupiah", 2_500_000),
            ("dua milyar", 2_000_000_000),
            ("26 juta", 26_000_000),
            ("2 ribu 500", 2_500),
            ("nol rupiah", 0),
        ];
        for (s, n) in cases {
            assert_eq!(parse_number_words(s), Some(*n), "{s}");
        }
        for bad in [
            "",
            "rupiah",
            "dua tiga",
            "dua puluh sebelas",
            "puluh",
            "ribu",
            "dua juta tiga juta",
            "lima ratus dua ratus",
            "satu nol",
            "setengah",
            "dua puluh lima ribuan",
            "banyak",
            "1500 juta",
        ] {
            assert_eq!(parse_number_words(bad), None, "{bad}");
        }
    }

    #[test]
    fn parse_price_reads_shop_and_listing_styles() {
        let cases: &[(&str, u64)] = &[
            ("Rp26.000.000", 26_000_000),
            ("Rp 26.000.000,-", 26_000_000),
            ("Rp26.000.000,00", 26_000_000),
            ("Rp. 7.500", 7_500),
            ("rp7.500", 7_500),
            ("IDR 50.000", 50_000),
            ("7.500", 7_500),
            ("7,500", 7_500),
            ("500", 500),
            ("Rp500", 500),
            ("26jt", 26_000_000),
            ("26 jt", 26_000_000),
            ("Rp 26 jt", 26_000_000),
            ("1,5 juta", 1_500_000),
            ("1.5jt", 1_500_000),
            ("2,75jt", 2_750_000),
            ("15rb", 15_000),
            ("15 ribu", 15_000),
            ("Rp 15rb", 15_000),
            ("15k", 15_000),
            ("15K", 15_000),
            ("7,5rb", 7_500),
            ("2M", 2_000_000_000),
            ("1,25M", 1_250_000_000),
            ("2 miliar", 2_000_000_000),
            ("1jt 500rb", 1_500_000),
            ("1jt500", 1_500_000),
            ("2jt750", 2_750_000),
            ("dua puluh enam juta rupiah", 26_000_000),
            ("seribu", 1_000),
        ];
        for (s, n) in cases {
            assert_eq!(parse_price(s), Some(*n), "{s}");
        }
        for bad in
            ["", "Rp", "abc", "1,2,3jt", "26.00.000", "1,3333333jt", "500rb 2jt", "1jt 2jt", "jt", "1.5"]
        {
            assert_eq!(parse_price(bad), None, "{bad}");
        }
    }

    #[test]
    fn format_rupiah_uses_dots() {
        assert_eq!(format_rupiah(0), "Rp0");
        assert_eq!(format_rupiah(500), "Rp500");
        assert_eq!(format_rupiah(7_500), "Rp7.500");
        assert_eq!(format_rupiah(100_000), "Rp100.000");
        assert_eq!(format_rupiah(26_000_000), "Rp26.000.000");
        assert_eq!(format_rupiah(1_250_000_000), "Rp1.250.000.000");
        for n in [1, 999, 1_000, 12_345_678, u64::MAX] {
            assert_eq!(parse_price(&format_rupiah(n)), Some(n));
        }
    }

    #[test]
    fn drills_are_deterministic_realistic_and_readable() {
        for level in Level::ALL {
            for seed in 0..30 {
                let a = drills(level, 12, seed);
                assert_eq!(a, drills(level, 12, seed));
                assert_eq!(a.len(), 12);
                for d in &a {
                    assert_eq!(parse_price(&d.shown), Some(d.amount), "{level:?} {d:?}");
                    assert_eq!(d.answer, rupiah_words(d.amount));
                    assert_eq!(check(d, &d.answer), Check::Correct);
                    let ok = match level {
                        Level::Hundreds => (100..=999).contains(&d.amount),
                        Level::Thousands => (1_000..=999_000).contains(&d.amount) && d.amount % 100 == 0,
                        Level::Millions => d.amount >= 1_000_000 && d.amount % 50_000 == 0,
                        Level::Mixed => d.amount >= 100,
                    };
                    assert!(ok, "{level:?}: {d:?}");
                }
                let mut amounts: Vec<u64> = a.iter().map(|d| d.amount).collect();
                amounts.sort();
                amounts.dedup();
                assert_eq!(amounts.len(), 12, "{level:?} seed {seed}: repeats");
            }
            assert_ne!(drills(level, 12, 1), drills(level, 12, 2));
        }
        // Over many seeds every display style shows up.
        let shown: Vec<String> = (0..40).flat_map(|s| drills(Level::Mixed, 10, s)).map(|d| d.shown).collect();
        for needle in ["Rp. ", ",-", "rb", "k", "jt", " juta", "M"] {
            assert!(shown.iter().any(|s| s.contains(needle)), "no {needle:?} style");
        }
        assert!(shown.iter().any(|s| s.starts_with("Rp") && !s.contains(' ')));
        assert!(drills(Level::Hundreds, 0, 1).is_empty());
    }

    #[test]
    fn check_grades_kindly() {
        let d = Drill { amount: 26_000_000, shown: "Rp26.000.000".into(), answer: rupiah_words(26_000_000) };
        assert_eq!(check(&d, "dua puluh enam juta rupiah"), Check::Correct);
        assert_eq!(check(&d, "  Dua puluh enam  juta Rupiah."), Check::Correct);
        let close = Check::Close { expected: "dua puluh enam juta rupiah".into() };
        assert_eq!(check(&d, "dua puluh enam juta"), close);
        assert_eq!(check(&d, "26 juta rupiah"), close);
        assert_eq!(
            check(&d, "dua puluh juta"),
            Check::Wrong { expected: "dua puluh enam juta rupiah".into(), heard: Some(20_000_000) }
        );
        assert_eq!(check(&d, "hmm"), Check::Wrong { expected: d.answer.clone(), heard: None });

        let m = Drill { amount: 1_000_000, shown: "1jt".into(), answer: rupiah_words(1_000_000) };
        assert_eq!(check(&m, "satu juta rupiah"), Check::Correct);
        assert!(matches!(check(&m, "sejuta rupiah"), Check::Close { .. }));
        assert!(check(&m, "sejuta").is_right());
        assert!(!check(&m, "seribu").is_right());
    }

    #[test]
    fn score_puts_accuracy_first() {
        assert_eq!(score(0, 0, 0), 0);
        assert_eq!(score(0, 10, 1_000), 0, "fast but wrong earns nothing");
        assert_eq!(score(10, 10, 20_000), 150, "all right, fast");
        assert_eq!(score(10, 10, 100_000), 100, "all right, slow: no bonus, still full points");
        assert_eq!(score(10, 10, 65_000), 125, "halfway between fast and slow");
        assert_eq!(score(5, 10, 20_000), 75);
        assert!(score(10, 10, 100_000) > score(6, 10, 1_000), "accuracy beats speed");
        assert_eq!(score(12, 10, 0), score(10, 10, 0), "correct is capped at total");
        let mut prev = u32::MAX;
        for ms in (0..=150_000).step_by(5_000) {
            let s = score(8, 10, ms);
            assert!(s <= prev, "slower never scores more");
            prev = s;
        }
    }

    #[test]
    fn javanese_numbers() {
        let cases: &[(u64, &str)] = &[
            (0, "nol"),
            (1, "siji"),
            (2, "loro"),
            (3, "telu"),
            (4, "papat"),
            (5, "lima"),
            (6, "enem"),
            (7, "pitu"),
            (8, "wolu"),
            (9, "sanga"),
            (10, "sepuluh"),
            (11, "sewelas"),
            (12, "rolas"),
            (13, "telulas"),
            (15, "limalas"),
            (19, "sangalas"),
            (20, "rong puluh"),
            (21, "selikur"),
            (25, "selawe"),
            (29, "sangalikur"),
            (30, "telung puluh"),
            (35, "telung puluh lima"),
            (50, "seket"),
            (55, "seket lima"),
            (60, "sewidak"),
            (75, "pitung puluh lima"),
            (100, "satus"),
            (150, "satus seket"),
            (200, "rong atus"),
            (500, "limang atus"),
            (600, "nem atus"),
            (1_000, "sewu"),
            (1_500, "sewu limang atus"),
            (2_000, "rong ewu"),
            (7_500, "pitung ewu limang atus"),
            (10_000, "sepuluh ewu"),
            (12_000, "rolas ewu"),
            (25_000, "selawe ewu"),
            (50_000, "seket ewu"),
            (150_000, "satus seket ewu"),
            (200_000, "rong atus ewu"),
            (1_000_000, "sayuta"),
            (2_000_000, "rong yuta"),
            (1_500_000, "sayuta limang atus ewu"),
            (25_000_000, "selawe yuta"),
            (
                999_999_999,
                "sangang atus sangang puluh sanga yuta sangang atus sangang puluh sanga ewu sangang atus sangang puluh sanga",
            ),
        ];
        for (n, words) in cases {
            assert_eq!(javanese_number_words(*n).as_deref(), Some(*words), "{n}");
        }
        assert_eq!(javanese_number_words(1_000_000_000), None);
        assert_eq!(javanese_number_words(u64::MAX), None);
        for n in 1..1_000_000u64 {
            let w = javanese_number_words(n).unwrap();
            assert!(
                !w.is_empty() && !w.contains("  ") && !w.starts_with(' ') && !w.ends_with(' '),
                "{n}: {w:?}"
            );
        }
    }
}
