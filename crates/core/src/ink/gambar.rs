//! Gambar! (draw it): Kabar says a word, the learner sketches it, AI vision
//! guesses what it is, in Indonesian. Picking words and scoring guesses is
//! deterministic; the guess itself is the `guess_sketch` task.

use serde::{Deserialize, Serialize};

/// Seconds to draw.
pub const ROUND_SECONDS: u32 = 60;

/// Concrete, drawable Indonesian words a beginner meets early (BIPA 1–2
/// topics: home, family, food, market, weather, transport, animals, body),
/// with their English meaning for the prompt card.
pub const DRAWABLE: &[(&str, &str)] = &[
    ("rumah", "house"),
    ("kucing", "cat"),
    ("anjing", "dog"),
    ("ikan", "fish"),
    ("burung", "bird"),
    ("ayam", "chicken"),
    ("sapi", "cow"),
    ("kuda", "horse"),
    ("gajah", "elephant"),
    ("pohon", "tree"),
    ("bunga", "flower"),
    ("matahari", "sun"),
    ("bulan", "moon"),
    ("bintang", "star"),
    ("hujan", "rain"),
    ("awan", "cloud"),
    ("gunung", "mountain"),
    ("laut", "sea"),
    ("sungai", "river"),
    ("pantai", "beach"),
    ("pasar", "market"),
    ("mobil", "car"),
    ("motor", "motorbike"),
    ("sepeda", "bicycle"),
    ("bus", "bus"),
    ("kapal", "ship"),
    ("pesawat", "airplane"),
    ("kereta", "train"),
    ("buku", "book"),
    ("pensil", "pencil"),
    ("meja", "table"),
    ("kursi", "chair"),
    ("pintu", "door"),
    ("jendela", "window"),
    ("tempat_tidur", "bed"),
    ("lampu", "lamp"),
    ("jam", "clock"),
    ("telepon", "phone"),
    ("tas", "bag"),
    ("sepatu", "shoe"),
    ("baju", "shirt"),
    ("topi", "hat"),
    ("kacamata", "glasses"),
    ("payung", "umbrella"),
    ("kunci", "key"),
    ("gelas", "glass"),
    ("piring", "plate"),
    ("sendok", "spoon"),
    ("nasi", "rice"),
    ("roti", "bread"),
    ("telur", "egg"),
    ("pisang", "banana"),
    ("apel", "apple"),
    ("kelapa", "coconut"),
    ("kopi", "coffee"),
    ("teh", "tea"),
    ("es_krim", "ice cream"),
    ("mata", "eye"),
    ("tangan", "hand"),
    ("kaki", "foot, leg"),
    ("telinga", "ear"),
    ("hidung", "nose"),
    ("mulut", "mouth"),
    ("kepala", "head"),
    ("masjid", "mosque"),
    ("sekolah", "school"),
    ("uang", "money"),
    ("bola", "ball"),
];

/// The model's guess (`prompts/guess_sketch@v1.md`); `tidak_jelas` when unclear.
pub use crate::ai::tasks::{GuessSketch as SketchGuess, UNCLEAR_SKETCH as UNCLEAR};

/// How the round went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Kabar guessed the word.
    Got,
    /// It was among Kabar's other ideas.
    Close,
    /// Kabar saw something else (often the funniest round).
    Missed,
    /// Kabar couldn't tell.
    Unclear,
}

pub fn judge(target: &str, guess: &SketchGuess) -> Verdict {
    let norm = |s: &str| s.trim().to_lowercase().replace([' ', '-'], "_");
    let t = norm(target);
    if guess.guess_id == UNCLEAR {
        Verdict::Unclear
    } else if norm(&guess.guess_id) == t {
        Verdict::Got
    } else if guess.alternatives.iter().any(|a| norm(a) == t) {
        Verdict::Close
    } else {
        Verdict::Missed
    }
}

/// The word as shown: `tempat_tidur` → `tempat tidur`.
pub fn display(lemma: &str) -> String {
    lemma.replace('_', " ")
}

/// Picks the next word: drawable words the learner has met come first (they
/// practise what they know), never one of the `recent` rounds; `seed` makes
/// the choice repeatable in tests and varied in the app.
pub fn next_word(known: &[&str], recent: &[&str], seed: u64) -> &'static (&'static str, &'static str) {
    let fresh = |w: &&(&str, &str)| !recent.contains(&w.0);
    let met: Vec<&(&str, &str)> = DRAWABLE.iter().filter(|w| known.contains(&w.0)).filter(fresh).collect();
    let pool: Vec<&(&str, &str)> = if met.is_empty() { DRAWABLE.iter().filter(fresh).collect() } else { met };
    let pool = if pool.is_empty() { DRAWABLE.iter().collect() } else { pool };
    // splitmix64 step for an even spread from sequential seeds.
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    pool[(z % pool.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiOutput;

    fn guess(id: &str, alts: &[&str]) -> SketchGuess {
        SketchGuess { guess_id: id.into(), alternatives: alts.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn judging_rounds() {
        assert_eq!(judge("kucing", &guess("kucing", &["anjing"])), Verdict::Got);
        assert_eq!(judge("kucing", &guess("harimau", &["kucing"])), Verdict::Close);
        assert_eq!(judge("tempat tidur", &guess("tempat_tidur", &[])), Verdict::Got);
        assert_eq!(judge("hujan", &guess("awan", &["matahari"])), Verdict::Missed);
        assert_eq!(judge("hujan", &guess(UNCLEAR, &[])), Verdict::Unclear);
        assert_eq!(display("es_krim"), "es krim");
    }

    #[test]
    fn validation_follows_the_golden_rubric() {
        assert!(guess("kucing", &["anjing", "harimau"]).validate().is_ok());
        assert!(guess("Kucing", &[]).validate().is_err());
        assert!(guess("seekor kucing", &[]).validate().is_err());
        assert!(guess("kucing", &["kucing"]).validate().is_err());
        assert!(guess("kucing", &["a", "b", "c", "d"]).validate().is_err());
        assert!(guess(UNCLEAR, &["kucing"]).validate().is_err());
        assert!(guess(UNCLEAR, &[]).validate().is_ok());
    }

    #[test]
    fn words_come_from_what_the_learner_knows_without_repeats() {
        let known = ["kucing", "pasar", "hujan", "makan"];
        for seed in 0..50 {
            let w = next_word(&known, &["pasar"], seed).0;
            assert!(w == "kucing" || w == "hujan", "{w}");
        }
        assert_eq!(next_word(&known, &["pasar"], 7), next_word(&known, &["pasar"], 7));
        // Nothing known yet: anything drawable.
        assert!(DRAWABLE.iter().any(|d| d.0 == next_word(&[], &[], 3).0));
        // Everything recent: still answers.
        let all: Vec<&str> = DRAWABLE.iter().map(|d| d.0).collect();
        let _ = next_word(&[], &all, 1);
        let lemmas: std::collections::BTreeSet<_> = DRAWABLE.iter().map(|d| d.0).collect();
        assert_eq!(lemmas.len(), DRAWABLE.len());
        assert!(DRAWABLE.iter().all(|d| guess(d.0, &[]).validate().is_ok()));
    }
}
