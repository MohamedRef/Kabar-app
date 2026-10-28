//! Build-time content shipped in content packs (`docs/SPEC.md` §18): word
//! glosses, the Arabic Bridge dataset and normalized dictionary entries.
//!
//! These types are the source of truth for `schemas/*.schema.json`, which
//! `kabar-tools schemas` regenerates (with the `schema` feature). Content
//! files are parsed with [`from_json_strict`] and checked against schemas
//! that forbid unknown fields, so a typo in generated content fails
//! validation instead of being silently dropped.
//!
//! The gloss types ([`GlossEntry`] and what it holds) don't use serde's
//! `deny_unknown_fields` themselves: they are embedded in synced Teach rows
//! (`learn::TaughtEntry`), which must read fields from a newer build.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Bumped whenever a content type changes incompatibly. Readers refuse packs
/// with a newer major version instead of misreading them.
pub const CONTENT_SCHEMA_VERSION: u32 = 1;

/// Parses one content record, failing on any field the type doesn't know
/// (at any depth), the way `deny_unknown_fields` would.
pub fn from_json_strict<T: DeserializeOwned>(json: &str) -> Result<T, serde_json::Error> {
    use serde::de::Error;
    let mut unknown = Vec::new();
    let mut de = serde_json::Deserializer::from_str(json);
    let value: T = serde_ignored::deserialize(&mut de, |path| unknown.push(path.to_string()))?;
    de.end()?;
    match unknown.first() {
        None => Ok(value),
        Some(path) => Err(serde_json::Error::custom(format!("unknown field `{path}`"))),
    }
}

/// Learning language of an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Indonesian (`id`).
    Id,
    /// Javanese (`jv`).
    Jv,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Id => "id",
            Lang::Jv => "jv",
        }
    }
}

/// Whether the learner has reviewed an item. Everything starts unverified;
/// Javanese stays unverified until the learner (or a native speaker) approves it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    #[default]
    Unverified,
    Approved,
    /// Reviewed and found wrong; kept so the next batch regenerates it.
    NeedsFix,
}

/// The same text in the three explanation voices (§11).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema), schemars(deny_unknown_fields))]
pub struct Trilingual {
    /// Egyptian colloquial Arabic (بالمصري).
    pub arz: String,
    /// Modern Standard Arabic.
    pub ar: String,
    /// English.
    pub en: String,
}

/// Part of speech, coarse enough for exercises and popovers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Pos {
    Noun,
    ProperNoun,
    Verb,
    Adjective,
    Adverb,
    Pronoun,
    Preposition,
    Conjunction,
    Particle,
    Numeral,
    Classifier,
    Determiner,
    Interjection,
    Phrase,
}

/// Register of a word or sense.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Register {
    #[default]
    Neutral,
    Formal,
    Informal,
    /// Jakarta / chat Indonesian (gue, lo, banget).
    Colloquial,
    Slang,
    Literary,
    Religious,
}

/// Javanese speech level (unggah-ungguh, §13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SpeechLevel {
    /// Plain speech: friends, younger people, yourself.
    Ngoko,
    /// The in-between level used in markets and casual polite talk.
    Madya,
    /// Polite speech to elders, strangers, people of higher status.
    Krama,
    /// Honorific words that elevate the other person (their body, actions, belongings).
    KramaInggil,
    /// Humble words for your own actions toward a respected person.
    KramaAndhap,
}

/// Who produced a piece of content, so a better pass can replace it later.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema), schemars(deny_unknown_fields))]
pub struct Provenance {
    /// `claude-code`, `wiktionary`, `user`, …
    pub generator: String,
    /// Versioned prompt id, e.g. `gloss@v1`. Empty for imports.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prompt: String,
}

// --- Glosses -----------------------------------------------------------------

/// One meaning of a word.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema), schemars(deny_unknown_fields))]
pub struct Sense {
    pub pos: Pos,
    /// Short gloss in each voice (a few words, not a definition essay).
    pub gloss: Trilingual,
    #[serde(default, skip_serializing_if = "is_default")]
    pub register: Register,
}

/// An example sentence with its translations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema), schemars(deny_unknown_fields))]
pub struct Example {
    /// The sentence in the learning language. Contains the word (or a form of it).
    pub text: String,
    pub translation: Trilingual,
    /// Index into `senses` of the meaning this example shows.
    #[serde(default)]
    pub sense: u8,
}

/// A prebuilt gloss for one frequent word form (§18), shown in the word
/// popover and used by lessons. Keyed by surface form, because that is what
/// the Reader looks up; `lemma`/`root`/`affixes` give the breakdown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema), schemars(deny_unknown_fields))]
pub struct GlossEntry {
    pub lang: Lang,
    /// Surface form, lowercase, as in the frequency list.
    pub word: String,
    /// Rank in the frequency list (1 = most frequent).
    pub rank: u32,
    /// Dictionary headword (equals `word` for bare roots and particles).
    pub lemma: String,
    /// Morphological root when the word is derived (membaca → baca).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// Display breakdown in order, e.g. `["meN-", "baca"]` or `["ke-", "sehat", "-an"]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affixes: Vec<String>,
    /// Most common meaning first. 1–4 senses.
    pub senses: Vec<Sense>,
    /// Two example sentences (three at most).
    pub examples: Vec<Example>,
    /// Javanese only: the Indonesian equivalent, since the learner meets
    /// Javanese after BIPA 1 and compares the two.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indonesian: Option<String>,
    /// Javanese only: the speech level of this form; its equivalents at the
    /// other levels live in the speech-level table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speech_level: Option<SpeechLevel>,
    /// Usage or register note worth knowing (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Trilingual>,
    /// Loud warning when the word looks like an Arabic word but means
    /// something else (kalimat ≠ «كلمة»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub false_friend: Option<Trilingual>,
    /// The Arabic source word when this is an Arabic loan; the full card lives
    /// in the Arabic Bridge dataset under the same `word`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arabic_origin: Option<String>,
    #[serde(default)]
    pub status: ReviewStatus,
    pub provenance: Provenance,
}

// --- Arabic Bridge -------------------------------------------------------------

/// How the meaning moved from Arabic to the learning language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MeaningShift {
    /// Same meaning as in Arabic.
    Same,
    /// A narrower meaning than the Arabic word.
    Narrowed,
    /// A wider meaning than the Arabic word.
    Broadened,
    /// Related but noticeably different.
    Shifted,
    /// Looks like an Arabic word an Egyptian knows, but means something else:
    /// triggers the False Friends Alarm.
    FalseFriend,
}

/// How sure we are that the word comes from Arabic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// One Arabic Bridge card (§12).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BridgeEntry {
    pub lang: Lang,
    /// The Indonesian/Javanese word, lowercase.
    pub word: String,
    /// The Arabic source word, vocalized where it helps (خَبَر).
    pub arabic: String,
    /// Arabic root letters separated by spaces (خ ب ر); absent for non-triliteral words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// What the Arabic word means.
    pub arabic_meaning: Trilingual,
    /// What the word means in the learning language.
    pub meaning: Trilingual,
    pub shift: MeaningShift,
    /// Explains the shift; required unless `shift` is `same`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_note: Option<Trilingual>,
    /// Sound-change rule ids from `content/bridge/sound-rules.json`, for the
    /// Sound-Shift Detective.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sound_rules: Vec<String>,
    /// Intermediate language when the word did not come straight from Arabic
    /// (e.g. Persian, Malay, Dutch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    pub confidence: Confidence,
    /// Where the etymology was found or checked (`claude-code`, `wiktionary`).
    pub sources: Vec<String>,
    #[serde(default)]
    pub status: ReviewStatus,
}

/// A regular sound change from Arabic, e.g. ف → p.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SoundRule {
    /// Stable snake_case id referenced by `BridgeEntry::sound_rules`.
    pub id: String,
    /// Arabic side, e.g. `ف`.
    pub from: String,
    /// Indonesian side, e.g. `p`.
    pub to: String,
    pub explanation: Trilingual,
    /// Indonesian words that show the rule; each must exist in the dataset.
    pub examples: Vec<String>,
}

// --- Javanese speech levels ------------------------------------------------------

/// A sentence shown at one speech level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LevelExample {
    pub level: SpeechLevel,
    pub text: String,
    pub translation: Trilingual,
}

/// One row of the speech-level table (§13, §17): the same meaning across
/// ngoko, madya, krama, krama inggil and krama andhap. An empty level means
/// the word has no special form there and the nearest lower level is used
/// (krama inggil → krama → madya → ngoko).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SpeechLevelEntry {
    /// Stable snake_case id from the English meaning (`eat`, `house`, `you`).
    pub id: String,
    pub pos: Pos,
    pub meaning: Trilingual,
    /// The standard Indonesian word.
    pub indonesian: String,
    pub ngoko: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub madya: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub krama: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub krama_inggil: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub krama_andhap: Vec<String>,
    /// When to use which form, with Egyptian analogies where they help.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Trilingual>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<LevelExample>,
    pub confidence: Confidence,
    pub sources: Vec<String>,
    #[serde(default)]
    pub status: ReviewStatus,
}

// --- Golden sets -----------------------------------------------------------------

/// The reference set for one runtime AI task (§19): what the bake-off sends
/// to each engine and how answers are judged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GoldenSet {
    /// Task id as in `kabar_protocol::ai::Task` (`analyze_text`, …).
    pub task: String,
    pub version: u32,
    /// Task-specific scoring notes on top of `docs/BAKEOFF.md`'s 0–3 rubric.
    pub rubric: String,
    pub items: Vec<GoldenItem>,
}

/// One golden case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GoldenItem {
    /// Unique within the set, e.g. `analyze-jv-chat-01`.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<Lang>,
    /// Task input, shaped per task (see the set's file header).
    pub input: serde_json::Value,
    /// Reference answer; for typed tasks it follows the task's output shape.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub expect: serde_json::Value,
    /// Must hold for a passing answer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub criteria: Vec<String>,
    /// Must not happen (a wrong meaning, guilt-tripping, leaking personal data…).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub avoid: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Vision tasks: fixture image or stroke file, relative to the repo root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Checked by the learner (or a native speaker for Javanese).
    #[serde(default)]
    pub verified: bool,
}

// --- Dictionary (normalized Wiktionary import) --------------------------------

/// One Wiktionary sense, kept in English as Wiktionary has it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DictSense {
    pub glosses: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<DictExample>,
    /// Lemma this sense points to when it is a form-of / alternative / abbreviation entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form_of: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DictExample {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}

/// An inflected or alternative form listed on a Wiktionary page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DictForm {
    pub form: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// An Arabic etymology link parsed from Wiktionary templates
/// (`bor`/`der`/`inh` with source language `ar`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ArabicEtymon {
    /// Arabic word as written in the template (may be empty when Wiktionary
    /// only names the language).
    pub arabic: String,
    /// Template kind: `bor` (borrowed), `der` (derived), `inh`, `cat` (category only).
    pub kind: String,
    /// Gloss given in the template, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gloss: Option<String>,
    /// Intermediate languages in the etymology chain before Arabic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub via: Vec<String>,
}

/// One Wiktionary entry (word + part of speech), normalized from kaikki.org JSONL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DictEntry {
    pub lang: Lang,
    pub word: String,
    /// Wiktionary part of speech (`noun`, `verb`, `adj`, …), as kaikki writes it.
    pub pos: String,
    pub senses: Vec<DictSense>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forms: Vec<DictForm>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ipa: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etymology: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arabic: Option<ArabicEtymon>,
    /// Javanese speech-level tags found on the entry (`ngoko`, `krama`, `krama-inggil`, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub speech_levels: Vec<String>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gloss_round_trips_and_rejects_unknown_fields() {
        let json = r#"{"lang":"id","word":"kabar","rank":512,"lemma":"kabar",
            "senses":[{"pos":"noun","gloss":{"arz":"خبر","ar":"خبر، نبأ","en":"news"}}],
            "examples":[{"text":"Apa kabar?","translation":{"arz":"إزيك؟","ar":"كيف حالك؟","en":"How are you?"}}],
            "arabic_origin":"خبر","provenance":{"generator":"claude-code","prompt":"gloss@v1"}}"#;
        let entry: GlossEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.status, ReviewStatus::Unverified);
        assert_eq!(entry.senses[0].register, Register::Neutral);
        let again: GlossEntry = serde_json::from_str(&serde_json::to_string(&entry).unwrap()).unwrap();
        assert_eq!(entry, again);

        let typo = json.replace("\"lemma\"", "\"lema\"");
        assert!(serde_json::from_str::<GlossEntry>(&typo).is_err());
    }
}
