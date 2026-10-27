//! Teach Kabar (`docs/SPEC.md` §26): entries the learner taught Kabar, kept
//! in a personal dictionary that syncs (table `taught_entries`, LWW per
//! `{lang}:{word}`) and shows above the prebuilt glosses in popovers.
//!
//! An entry is a content-pack [`GlossEntry`] plus what only a taught entry
//! has: the sources it came from and the sentence it was met in. Popovers,
//! lessons and the next content batch read one gloss shape.
//!
//! These rows sync between devices that may run different builds, so unlike
//! the content files they never reject unknown fields: an older build keeps
//! a newer build's extra fields (in [`TaughtEntry::extra`]) and writes them
//! back untouched, instead of failing the whole dictionary.

use serde::{Deserialize, Serialize};

use crate::content::{GlossEntry, Lang, MeaningShift, Provenance, ReviewStatus, Trilingual};

/// Sync table holding [`TaughtEntry`] rows.
pub const TAUGHT_TABLE: &str = "taught_entries";
/// `provenance.generator` of every taught entry: the "taught by you" badge.
pub const TAUGHT_GENERATOR: &str = "learner";
/// Bumped when [`TaughtEntry`] changes incompatibly.
pub const TAUGHT_SCHEMA_VERSION: u32 = 1;
/// Most characters of one source excerpt kept with an entry (sync payloads
/// stay far below the 64 KiB op limit).
pub const MAX_EXCERPT_CHARS: usize = 2000;

/// Where a piece of the explanation came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Text the learner pasted or typed (from Google, a friend, a dictionary).
    Pasted,
    /// A web page the learner linked; `excerpt` is the page text Kabar used.
    Link,
    /// A screenshot, read with on-device OCR. Treated as personal.
    Screenshot,
    /// Kabar's free in-app lookup (Wiktionary's definition API).
    Wiktionary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeachSource {
    pub kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// What Kabar read from this source (scrubbed, at most [`MAX_EXCERPT_CHARS`]).
    pub excerpt: String,
}

/// The Arabic Bridge part of a taught entry (§12). The full card shape lives
/// in `content::BridgeEntry`; this keeps what a single word needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaughtBridge {
    /// The Arabic source word, vocalized where it helps.
    pub arabic: String,
    /// Root letters separated by spaces (خ ب ر).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    pub shift: MeaningShift,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_note: Option<Trilingual>,
}

/// Javanese speech level of the word and its counterparts (§13).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechLevels {
    /// Level of this word: `ngoko`, `krama`, `krama_inggil` or `neutral`.
    pub level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ngoko: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub krama: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub krama_inggil: Option<String>,
}

/// One word or phrase the learner taught Kabar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaughtEntry {
    pub schema: u32,
    /// The gloss itself. `status` is the review state (`unverified` = draft),
    /// `provenance.generator` is [`TAUGHT_GENERATOR`], `rank` is 0.
    /// [`GlossEntry`] is strict for content files, so a synced one is read
    /// leniently: top-level fields this build doesn't know yet are dropped
    /// from it (see [`lenient_gloss`]).
    #[serde(deserialize_with = "lenient_gloss")]
    pub entry: GlossEntry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<TaughtBridge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<SpeechLevels>,
    /// The sentence the learner met the word in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub sources: Vec<TeachSource>,
    /// Where the model's version disagreed with the source or was unsure,
    /// shown on the review screen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doubts: Vec<Trilingual>,
    /// Engine that wrote the draft (`workers_ai`, …).
    pub engine: String,
    /// The learner changed fields by hand after the draft.
    #[serde(default)]
    pub edited: bool,
    /// Fields from a newer build, kept so re-saving here doesn't lose them.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Reads a [`GlossEntry`] from a synced row, dropping the top-level fields
/// this build doesn't know (they come from a newer build). Anything else
/// wrong is still an error, and the store skips that one row.
fn lenient_gloss<'de, D: serde::Deserializer<'de>>(d: D) -> Result<GlossEntry, D::Error> {
    use serde::de::Error;
    let mut value = serde_json::Value::deserialize(d)?;
    loop {
        match serde_json::from_value::<GlossEntry>(value.clone()) {
            Ok(entry) => return Ok(entry),
            Err(e) => {
                let unknown = unknown_field(&e.to_string());
                let removed = unknown.and_then(|f| value.as_object_mut()?.remove(&f));
                if removed.is_none() {
                    return Err(D::Error::custom(e));
                }
            }
        }
    }
}

/// The field named by serde's "unknown field `x`, expected …" message.
fn unknown_field(message: &str) -> Option<String> {
    let rest = message.strip_prefix("unknown field `")?;
    Some(rest[..rest.find('`')?].to_owned())
}

impl TaughtEntry {
    /// Sync row id: one entry per (language, word).
    pub fn row_id(&self) -> String {
        row_id(self.entry.lang, &self.entry.word)
    }

    pub fn is_approved(&self) -> bool {
        self.entry.status == ReviewStatus::Approved
    }

    pub fn is_draft(&self) -> bool {
        self.entry.status == ReviewStatus::Unverified
    }

    /// The provenance every taught entry carries.
    pub fn provenance(prompt: &str) -> Provenance {
        Provenance { generator: TAUGHT_GENERATOR.into(), prompt: prompt.into() }
    }
}

pub fn row_id(lang: Lang, word: &str) -> String {
    format!("{}:{}", lang.code(), word.trim().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Example, Pos, Sense};

    fn tri(s: &str) -> Trilingual {
        Trilingual { arz: s.into(), ar: s.into(), en: s.into() }
    }

    #[test]
    fn round_trips_and_keys_by_language_and_word() {
        let t = TaughtEntry {
            schema: TAUGHT_SCHEMA_VERSION,
            entry: GlossEntry {
                lang: Lang::Jv,
                word: "dicul".into(),
                rank: 0,
                lemma: "diculke".into(),
                root: Some("cul".into()),
                affixes: vec!["di-".into(), "cul".into()],
                senses: vec![Sense { pos: Pos::Verb, gloss: tri("sold"), register: Default::default() }],
                examples: vec![Example { text: "arp dicul piro".into(), translation: tri("x"), sense: 0 }],
                note: None,
                false_friend: None,
                arabic_origin: None,
                indonesian: None,
                speech_level: None,
                status: ReviewStatus::Unverified,
                provenance: TaughtEntry::provenance("learn_entry@v1"),
            },
            bridge: None,
            levels: Some(SpeechLevels { level: "ngoko".into(), ..Default::default() }),
            context: Some("arp dicul piro".into()),
            sources: vec![TeachSource {
                kind: SourceKind::Pasted,
                url: None,
                title: None,
                excerpt: "dicul = diculke".into(),
            }],
            doubts: vec![],
            engine: "workers_ai".into(),
            edited: false,
            extra: Default::default(),
        };
        assert_eq!(t.row_id(), "jv:dicul");
        assert!(t.is_draft() && !t.is_approved());
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<TaughtEntry>(&json).unwrap(), t);
        assert_eq!(row_id(Lang::Id, " Mantul "), "id:mantul");
    }
}
