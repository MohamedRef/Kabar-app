//! The language engine: tokenizer → slang normalizer → language ID →
//! morphology. Deterministic and local; AI only fills gaps later.

pub mod analyze;
pub mod langid;
pub mod morph_id;
pub mod morph_jv;
pub mod normalize;
pub mod token;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Indonesian,
    Javanese,
    English,
    /// Names of people, places, brands.
    Proper,
    Unknown,
}

/// Word lists the engine can consult. Implemented by the app's SQLite
/// dictionary; tests and wasm use in-memory sets.
pub trait Lexicon {
    /// `word` is a known Indonesian root (lowercase).
    fn is_id_root(&self, word: &str) -> bool;
    /// `word` is a known Javanese root (lowercase).
    fn is_jv_root(&self, word: &str) -> bool;
}

/// The roots bundled with sastrawi-rs (~26k Indonesian, ~1.3k Javanese) plus
/// our own additions. A starting point until the Wiktionary import lands.
pub struct BuiltinLexicon {
    id: sastrawi::Dictionary,
    jv: sastrawi::javanese::JavaneseDictionary,
    extra_jv: std::collections::HashSet<&'static str>,
}

/// Javanese roots missing from sastrawi-rs that the test texts and first
/// lessons need. Unverified until reviewed (see docs/PLAN.md D5).
const EXTRA_JV_ROOTS: &[&str] = &[
    "cul",
    "tuku",
    "arep",
    "pira",
    "piye",
    "sek",
    "wis",
    "durung",
    "karo",
    "kowe",
    "sing",
    "ora",
    "apa",
    "mangan",
    "nedha",
    "dhahar",
    "sampun",
    "sampeyan",
    "panjenengan",
    "koen",
    "rek",
    "omah",
    "laku",
    "tulis",
    "waca",
    "sapu",
    "ombe",
    "turu",
    "lunga",
    "teka",
    "jupuk",
    "gawa",
    "weneh",
    "dol",
    "rega",
    "klambi",
    "pasar",
    "larang",
    "murah",
    "nang",
    "neng",
    "ning",
    "dhe",
    "mas",
    "mbak",
    "pak",
    "bu",
];

impl Default for BuiltinLexicon {
    fn default() -> Self {
        Self {
            id: sastrawi::Dictionary::new(),
            jv: sastrawi::javanese::JavaneseDictionary::new(),
            extra_jv: EXTRA_JV_ROOTS.iter().copied().collect(),
        }
    }
}

impl Lexicon for BuiltinLexicon {
    fn is_id_root(&self, word: &str) -> bool {
        self.id.find(word)
    }

    fn is_jv_root(&self, word: &str) -> bool {
        self.extra_jv.contains(word) || self.jv.find(word)
    }
}
