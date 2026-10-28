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
//! a newer build's fields, at any depth (in [`TaughtEntry::kept`]), and
//! writes them back when it saves the entry, instead of failing the whole
//! dictionary or erasing them for every device.

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
///
/// Its serde impls wrap the derived ones (`remote = "Self"`) to keep
/// [`KeptFields`]: see the module docs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct TaughtEntry {
    pub schema: u32,
    /// The gloss itself. `status` is the review state (`unverified` = draft),
    /// `provenance.generator` is [`TAUGHT_GENERATOR`], `rank` is 0.
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
    /// Fields from a newer build that this one doesn't know, kept so saving
    /// the entry here writes them back.
    #[serde(skip)]
    pub kept: KeptFields,
}

/// Fields of a decoded row that this build doesn't know, by JSON pointer
/// (`/entry/senses/0/audio`), with their values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeptFields(Vec<(String, serde_json::Value)>);

impl KeptFields {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The kept value at `pointer`, if any.
    pub fn get(&self, pointer: &str) -> Option<&serde_json::Value> {
        self.0.iter().find(|(p, _)| p == pointer).map(|(_, v)| v)
    }

    /// Puts each kept field back into `value` (a serialized entry). A field
    /// whose parent no longer exists here (a sense this device removed) goes
    /// with it.
    fn restore(&self, value: &mut serde_json::Value) {
        for (pointer, kept) in &self.0 {
            let Some((parent, key)) = pointer.rsplit_once('/') else { continue };
            if let Some(obj) = value.pointer_mut(parent).and_then(|v| v.as_object_mut()) {
                obj.entry(unescape(key)).or_insert_with(|| kept.clone());
            }
        }
    }
}

impl<'de> Deserialize<'de> for TaughtEntry {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let raw = serde_json::Value::deserialize(d)?;
        let mut unknown = Vec::new();
        let mut note = |path: serde_ignored::Path<'_>| unknown.push(pointer(&path));
        let mut entry = TaughtEntry::deserialize(serde_ignored::Deserializer::new(&raw, &mut note))
            .map_err(D::Error::custom)?;
        let kept = unknown.into_iter().filter_map(|p| {
            let value = raw.pointer(&p)?.clone();
            Some((p, value))
        });
        entry.kept = KeptFields(kept.collect());
        Ok(entry)
    }
}

impl Serialize for TaughtEntry {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        if self.kept.is_empty() {
            return Known(self).serialize(s);
        }
        let mut value = serde_json::to_value(Known(self)).map_err(S::Error::custom)?;
        self.kept.restore(&mut value);
        value.serialize(s)
    }
}

/// The fields this build knows, through the derived impls.
struct Known<'a>(&'a TaughtEntry);

impl Serialize for Known<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TaughtEntry::serialize(self.0, s)
    }
}

/// A `serde_ignored` path as a JSON pointer.
fn pointer(path: &serde_ignored::Path<'_>) -> String {
    use serde_ignored::Path;
    match path {
        Path::Root => String::new(),
        Path::Seq { parent, index } => format!("{}/{index}", pointer(parent)),
        Path::Map { parent, key } => {
            format!("{}/{}", pointer(parent), key.replace('~', "~0").replace('/', "~1"))
        }
        Path::Some { parent } | Path::NewtypeStruct { parent } | Path::NewtypeVariant { parent } => {
            pointer(parent)
        }
    }
}

fn unescape(key: &str) -> String {
    key.replace("~1", "/").replace("~0", "~")
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

    fn sample() -> TaughtEntry {
        TaughtEntry {
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
            kept: Default::default(),
        }
    }

    #[test]
    fn round_trips_and_keys_by_language_and_word() {
        let t = sample();
        assert_eq!(t.row_id(), "jv:dicul");
        assert!(t.is_draft() && !t.is_approved());
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<TaughtEntry>(&json).unwrap(), t);
        assert_eq!(row_id(Lang::Id, " Mantul "), "id:mantul");
    }

    /// What a newer build might write: a field on the entry, one in the
    /// gloss, one inside a sense and one inside a source.
    fn newer_row() -> serde_json::Value {
        let mut row = serde_json::to_value(sample()).unwrap();
        row["mnemonic"] = "di + cul".into();
        row["entry"]["audio_hint"] = "di-CUL".into();
        row["entry"]["senses"][0]["image"] = serde_json::json!({ "id": "sell", "tags": ["market"] });
        row["sources"][0]["fetched_ms"] = 1_761_000_000_000_u64.into();
        row
    }

    #[test]
    fn an_older_build_keeps_a_newer_builds_fields_at_any_depth() {
        let row = newer_row();
        // Older device reads the row, the learner edits it, the device saves it.
        let mut older: TaughtEntry = serde_json::from_value(row.clone()).unwrap();
        assert_eq!(older.kept.get("/entry/senses/0/image").unwrap()["id"], "sell");
        older.edited = true;
        older.context = Some("arep dicul pira".into());
        let saved = serde_json::to_value(&older).unwrap();
        // Back on the newer device nothing it wrote is missing.
        let mut expected = row;
        expected["edited"] = true.into();
        expected["context"] = "arep dicul pira".into();
        assert_eq!(saved, expected);
        // And a second older round trip is stable.
        let again: TaughtEntry = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), saved);
    }

    #[test]
    fn a_field_cleared_here_stays_cleared() {
        let mut older: TaughtEntry = serde_json::from_value(newer_row()).unwrap();
        older.levels = None;
        older.entry.senses.clear();
        let saved = serde_json::to_value(&older).unwrap();
        assert!(saved.get("levels").is_none());
        // The sense went, so its newer field goes with it; the others stay.
        assert_eq!(saved["entry"]["senses"], serde_json::json!([]));
        assert_eq!(saved["entry"]["audio_hint"], "di-CUL");
        assert_eq!(saved["mnemonic"], "di + cul");
    }

    #[test]
    fn content_parsing_stays_strict() {
        let mut row = serde_json::to_value(sample().entry).unwrap();
        row["senses"][0]["glos"] = "typo".into();
        let err = crate::content::from_json_strict::<GlossEntry>(&row.to_string()).unwrap_err();
        assert!(err.to_string().contains("senses.0.glos"), "{err}");
        row["senses"][0].as_object_mut().unwrap().remove("glos");
        crate::content::from_json_strict::<GlossEntry>(&row.to_string()).unwrap();
    }
}
