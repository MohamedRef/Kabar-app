//! Ink and the Tablet Studio (`docs/SPEC.md` §4, PLAN §17): strokes,
//! notebooks and their pages, items placed on a page, and recognized text.
//!
//! Sync tables: `notebooks` and `ink_documents` (LWW), `ink_strokes`
//! (immutable: appended once, erased by a tombstone), `notebook_items` and
//! `ink_transcripts` (LWW).

use serde::{Deserialize, Serialize};

use crate::content::Lang;
use crate::pen::PointerKind;

/// Bumped when an ink type changes incompatibly.
pub const INK_SCHEMA_VERSION: u32 = 1;
/// Coordinates are stored as integers in 1/10 page pixels.
pub const COORD_SCALE: f32 = 10.0;
/// Longer strokes are split, so one op stays far below the sync payload limit.
pub const MAX_STROKE_POINTS: usize = 2000;

pub const NOTEBOOKS_TABLE: &str = "notebooks";
pub const DOCUMENTS_TABLE: &str = "ink_documents";
pub const STROKES_TABLE: &str = "ink_strokes";
pub const ITEMS_TABLE: &str = "notebook_items";
pub const TRANSCRIPTS_TABLE: &str = "ink_transcripts";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InkTool {
    #[default]
    Pen,
    /// Wide, translucent, drawn under the ink.
    Highlighter,
    /// Pressure-sensitive brush (Gambar!, later Aksara Jawa).
    Brush,
}

/// One finished stroke, as the ink module hands it over. Immutable once saved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InkStroke {
    /// UUIDv7, so ids sort by creation time.
    pub id: String,
    /// The [`InkDocument`] it belongs to.
    pub doc: String,
    #[serde(default)]
    pub tool: InkTool,
    /// A theme token (`ink`, `dawn`, `sawah`, `gold`) or `#rrggbb`.
    pub color: String,
    /// Base width in page pixels; pressure scales it.
    pub width: f32,
    pub pointer: PointerKind,
    /// Page coordinates × [`COORD_SCALE`].
    pub xs: Vec<i32>,
    pub ys: Vec<i32>,
    /// Pressure 0–255 per point; empty when the pointer has none (mouse).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ps: Vec<u8>,
    /// Milliseconds since the stroke started, per point.
    pub ts: Vec<u32>,
    /// Unix ms when the stroke started.
    pub started_ms: u64,
}

impl InkStroke {
    pub fn len(&self) -> usize {
        self.xs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.xs.is_empty()
    }

    /// Point arrays agree in length and stay within [`MAX_STROKE_POINTS`].
    pub fn is_well_formed(&self) -> bool {
        let n = self.xs.len();
        n > 0
            && n <= MAX_STROKE_POINTS
            && self.ys.len() == n
            && self.ts.len() == n
            && (self.ps.is_empty() || self.ps.len() == n)
            && self.width.is_finite()
            && self.width > 0.0
    }
}

/// A notebook: one per unit, or a free one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notebook {
    pub id: String,
    pub title: String,
    /// Curriculum unit id (`bipa1.u01`) for unit notebooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub lang: Lang,
    pub created_ms: u64,
}

/// What an ink canvas is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocKind {
    /// An infinite page in a notebook.
    Page,
    /// One Gambar! sketch.
    Gambar,
    /// Listening Companion scribbles.
    Listening,
    /// A mind map (its nodes are [`NotebookItem`]s).
    MindMap,
    /// A handwritten lesson answer.
    LessonAnswer,
    /// A margin note next to a passage in the Reader; see [`InkAnchor`].
    ReaderMargin,
}

/// Where a Reader margin note is attached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InkAnchor {
    /// The Reader text's id.
    pub text_id: String,
    /// Byte range of the passage (paragraph or chat bubble) in the text.
    pub start_byte: usize,
    pub end_byte: usize,
    /// `start` or `end`: which margin, in reading direction.
    pub side: String,
}

/// One canvas: a notebook page, a sketch, a lesson answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InkDocument {
    pub id: String,
    pub kind: DocKind,
    /// The notebook a page belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notebook: Option<String>,
    pub title: String,
    pub lang: Lang,
    /// Page order inside its notebook.
    #[serde(default)]
    pub order: u32,
    /// Reader margin notes: the passage it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<InkAnchor>,
    /// Canvas size for fixed-size canvases (margin notes, answers); pages are infinite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f32; 2]>,
    pub created_ms: u64,
}

/// Something placed on a page besides ink.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemKind {
    /// A live word card: gloss, audio and hover come from the dictionary.
    WordCard { lang: Lang, word: String },
    /// A typed note.
    Text { text: String },
    /// An arrow between two items, or to a free end point.
    Arrow {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
        /// Free end, page pixels (used when `to` is absent).
        x2: f32,
        y2: f32,
    },
    /// A mind-map node; `parent` is another node's item id.
    MapNode {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gloss: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        depth: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotebookItem {
    pub id: String,
    pub doc: String,
    pub kind: ItemKind,
    /// Top-left corner and size, page pixels.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Where recognized text came from, most trusted last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptSource {
    /// The `read_handwriting` vision task.
    Ai,
    /// ML Kit Digital Ink Recognition, on the device.
    MlKit,
    /// The learner typed or corrected it.
    Typed,
}

/// Recognized text for a group of strokes (a line or a word), searchable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InkTranscript {
    pub doc: String,
    /// Stable id of the stroke group (its first stroke's id).
    pub group: String,
    pub strokes: Vec<String>,
    pub text: String,
    pub lang: Lang,
    pub source: TranscriptSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Bounding box `[x, y, w, h]`, page pixels, to jump to a search hit.
    pub bbox: [f32; 4],
}

impl InkTranscript {
    /// Sync row id.
    pub fn row_id(&self) -> String {
        format!("{}:{}", self.doc, self.group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::{SyncKind, table_kind};

    #[test]
    fn tables_are_synced_with_the_right_rules() {
        assert_eq!(table_kind(NOTEBOOKS_TABLE), Some(SyncKind::Lww));
        assert_eq!(table_kind(DOCUMENTS_TABLE), Some(SyncKind::Lww));
        assert_eq!(table_kind(STROKES_TABLE), Some(SyncKind::Immutable));
        assert_eq!(table_kind(ITEMS_TABLE), Some(SyncKind::Lww));
        assert_eq!(table_kind(TRANSCRIPTS_TABLE), Some(SyncKind::Lww));
    }

    #[test]
    fn item_and_stroke_json() {
        let card: NotebookItem = serde_json::from_str(
            r#"{"id":"i1","doc":"d1","kind":{"type":"word_card","lang":"id","word":"pasar"},"x":10,"y":20,"w":160,"h":64}"#,
        )
        .unwrap();
        assert_eq!(card.kind, ItemKind::WordCard { lang: Lang::Id, word: "pasar".into() });

        let s = InkStroke {
            id: "s1".into(),
            doc: "d1".into(),
            tool: InkTool::Pen,
            color: "ink".into(),
            width: 2.0,
            pointer: PointerKind::Pen,
            xs: vec![0, 10],
            ys: vec![0, 5],
            ps: vec![128, 200],
            ts: vec![0, 8],
            started_ms: 1,
        };
        assert!(s.is_well_formed());
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("\"tool\":\"highlighter\""));
        assert_eq!(serde_json::from_str::<InkStroke>(&json).unwrap(), s);
        assert!(!InkStroke { ps: vec![1], ..s.clone() }.is_well_formed());
        assert!(!InkStroke { xs: vec![], ys: vec![], ts: vec![], ps: vec![], ..s }.is_well_formed());
    }
}
