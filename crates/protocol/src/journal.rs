//! The learning journal's reflections (spec §6): a short note the learner
//! writes about a day, shown in that day's group of the timeline. The rest of
//! the journal is derived from the activity log and is never stored twice.
//!
//! Sync: table [`JOURNAL_TABLE`], one row per reflection id, last-writer-wins
//! by HLC (a reflection can be edited); deleting is a tombstone.
//!
//! Reflections are personal. They never go to an AI engine, not even a
//! "private" one: nothing in the AI router takes a [`JournalEntry`].

use serde::{Deserialize, Serialize};

use crate::coach::LocalDate;

/// Sync table holding [`JournalEntry`] rows.
pub const JOURNAL_TABLE: &str = "journal_entries";
/// Bumped when [`JournalEntry`] changes incompatibly.
pub const JOURNAL_SCHEMA_VERSION: u32 = 1;
/// Longest typed reflection, in characters. Keeps a row far below the sync
/// payload limit (`sync::MAX_PAYLOAD_BYTES`), even in Arabic script.
pub const MAX_REFLECTION_CHARS: usize = 4000;

/// One reflection on one day.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Unique per reflection (UUIDv7); also the sync row id.
    pub id: String,
    /// The local day the reflection belongs to.
    pub date: LocalDate,
    /// Typed text, possibly several lines. May be empty when `ink` is set.
    #[serde(default)]
    pub text: String,
    /// A handwritten reflection (tablet). See [`InkRef`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink: Option<InkRef>,
    pub created_ms: u64,
    /// Last edit; equal to `created_ms` until the learner edits it.
    #[serde(default)]
    pub updated_ms: u64,
}

impl JournalEntry {
    /// Nothing worth keeping: no text and no handwriting.
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty() && self.ink.is_none()
    }
}

/// Handwriting seam. Strokes are immutable and live in the synced
/// `ink_documents` / `ink_strokes` tables (Tablet Studio); a reflection only
/// points at its ink document, so an edit never rewrites strokes and a
/// reflection row stays small. Nothing writes this yet: Kabar has no
/// reusable ink canvas until Tablet Studio (M9), so reflections are typed
/// for now, and a build that knows ink fills this in without a schema change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InkRef {
    /// Row id in `ink_documents`.
    pub document_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflections_round_trip() {
        let e = JournalEntry {
            id: "0192f0a0-0000-7000-8000-000000000001".into(),
            date: LocalDate::from_ymd(2026, 10, 26).unwrap(),
            text: "Saya sedang belajar bahasa Indonesia.\nBesok saya mau ke pasar.".into(),
            ink: None,
            created_ms: 1_793_011_856_000,
            updated_ms: 1_793_011_856_000,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains(r#""date":"2026-10-26""#));
        assert!(!json.contains("ink"), "no handwriting, no key: {json}");
        assert_eq!(serde_json::from_str::<JournalEntry>(&json).unwrap(), e);

        let inked = JournalEntry { text: String::new(), ink: Some(InkRef { document_id: "d1".into() }), ..e };
        let back: JournalEntry = serde_json::from_str(&serde_json::to_string(&inked).unwrap()).unwrap();
        assert_eq!(back, inked);
        assert!(!back.is_blank());
    }

    #[test]
    fn older_rows_default_missing_fields() {
        let e: JournalEntry =
            serde_json::from_str(r#"{"id":"a","date":"2026-10-25","created_ms":5}"#).unwrap();
        assert_eq!(e.text, "");
        assert_eq!(e.updated_ms, 0);
        assert!(e.is_blank());
    }
}
