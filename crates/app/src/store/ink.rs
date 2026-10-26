//! Tablet Studio ink (M9, PLAN §17): notebooks, ink documents (pages,
//! sketches, answers, Reader margin notes), strokes, items and transcripts.
//! No typed tables: every row lives in `sync_rows`, found by the
//! `sync_rows_doc` index on `payload.doc`.

use kabar_core::ink::{simplify_stroke, split_stroke};
use kabar_protocol::ink::{
    DOCUMENTS_TABLE, DocKind, ITEMS_TABLE, InkDocument, InkStroke, InkTranscript, NOTEBOOKS_TABLE, Notebook,
    NotebookItem, STROKES_TABLE, TRANSCRIPTS_TABLE,
};
use kabar_protocol::sync::OpKind;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{Result, Store};

/// Points closer than this to the line through their neighbours are dropped
/// before a stroke is saved (a fifth of a page pixel: invisible).
const SIMPLIFY_PX: f32 = 0.2;

impl Store {
    fn put_ink<T: Serialize>(&mut self, table: &str, row_id: &str, row: &T) -> Result<()> {
        self.write_synced(table, row_id, OpKind::Upsert, Some(serde_json::to_value(row)?)).map(drop)
    }

    fn drop_ink(&mut self, table: &str, row_id: &str) -> Result<()> {
        self.write_synced(table, row_id, OpKind::Delete, None).map(drop)
    }

    fn ink_row<T: DeserializeOwned>(&self, table: &str, row_id: &str) -> Result<Option<T>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload FROM sync_rows WHERE tbl = ?1 AND row_id = ?2 AND deleted = 0 AND payload IS NOT NULL",
        )?;
        let mut rows = stmt.query([table, row_id])?;
        match rows.next()? {
            Some(r) => Ok(Some(serde_json::from_str(&r.get::<_, String>(0)?)?)),
            None => Ok(None),
        }
    }

    /// Live rows of `table` whose payload has `doc` = `doc`. Rows a newer
    /// build wrote in a shape this one can't read are skipped.
    fn ink_rows_of<T: DeserializeOwned>(&self, table: &str, doc: &str) -> Result<Vec<T>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload FROM sync_rows
             WHERE tbl = ?1 AND json_extract(payload, '$.doc') = ?2 AND deleted = 0 AND payload IS NOT NULL",
        )?;
        let rows: Vec<String> =
            stmt.query_map([table, doc], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(rows.iter().filter_map(|p| serde_json::from_str(p).ok()).collect())
    }

    fn ink_all<T: DeserializeOwned>(&self, table: &str) -> Result<Vec<T>> {
        Ok(self.synced_rows(table)?.into_iter().filter_map(|(_, p)| serde_json::from_value(p).ok()).collect())
    }

    pub fn save_notebook(&mut self, notebook: &Notebook) -> Result<()> {
        self.put_ink(NOTEBOOKS_TABLE, &notebook.id, notebook)
    }

    /// Notebooks, oldest first.
    pub fn notebooks(&self) -> Result<Vec<Notebook>> {
        let mut all: Vec<Notebook> = self.ink_all(NOTEBOOKS_TABLE)?;
        all.sort_by(|a, b| a.created_ms.cmp(&b.created_ms).then_with(|| a.id.cmp(&b.id)));
        Ok(all)
    }

    pub fn save_ink_document(&mut self, doc: &InkDocument) -> Result<()> {
        self.put_ink(DOCUMENTS_TABLE, &doc.id, doc)
    }

    pub fn ink_document(&self, id: &str) -> Result<Option<InkDocument>> {
        self.ink_row(DOCUMENTS_TABLE, id)
    }

    /// Removes a document; its strokes stay as history but nothing shows them.
    pub fn delete_ink_document(&mut self, id: &str) -> Result<()> {
        self.drop_ink(DOCUMENTS_TABLE, id)
    }

    /// Documents of one kind, in page order (then creation).
    pub fn documents(&self, kind: DocKind) -> Result<Vec<InkDocument>> {
        let mut all: Vec<InkDocument> =
            self.ink_all::<InkDocument>(DOCUMENTS_TABLE)?.into_iter().filter(|d| d.kind == kind).collect();
        all.sort_by(|a, b| (a.order, a.created_ms, &a.id).cmp(&(b.order, b.created_ms, &b.id)));
        Ok(all)
    }

    /// A notebook's pages, in order.
    pub fn pages(&self, notebook: &str) -> Result<Vec<InkDocument>> {
        Ok(self
            .documents(DocKind::Page)?
            .into_iter()
            .filter(|d| d.notebook.as_deref() == Some(notebook))
            .collect())
    }

    /// Saves a finished stroke: simplified, then split when it is longer than
    /// one op may carry. Returns the saved pieces (the first keeps the id).
    pub fn add_stroke(&mut self, stroke: &InkStroke) -> Result<Vec<InkStroke>> {
        let simple = simplify_stroke(stroke, SIMPLIFY_PX);
        let pieces = split_stroke(simple, || uuid::Uuid::now_v7().to_string());
        let tx = self.conn.transaction()?;
        for p in &pieces {
            super::write_synced(
                &tx,
                &mut self.clock,
                STROKES_TABLE,
                &p.id,
                OpKind::Append,
                Some(serde_json::to_value(p)?),
            )?;
        }
        tx.commit()?;
        Ok(pieces)
    }

    /// Erases a stroke: a tombstone, so it disappears on every device.
    pub fn erase_stroke(&mut self, id: &str) -> Result<()> {
        self.drop_ink(STROKES_TABLE, id)
    }

    /// A document's strokes in drawing order.
    pub fn strokes(&self, doc: &str) -> Result<Vec<InkStroke>> {
        let mut all: Vec<InkStroke> = self.ink_rows_of(STROKES_TABLE, doc)?;
        all.sort_by(|a, b| a.started_ms.cmp(&b.started_ms).then_with(|| a.id.cmp(&b.id)));
        Ok(all)
    }

    pub fn save_item(&mut self, item: &NotebookItem) -> Result<()> {
        self.put_ink(ITEMS_TABLE, &item.id, item)
    }

    pub fn delete_item(&mut self, id: &str) -> Result<()> {
        self.drop_ink(ITEMS_TABLE, id)
    }

    pub fn items(&self, doc: &str) -> Result<Vec<NotebookItem>> {
        let mut all: Vec<NotebookItem> = self.ink_rows_of(ITEMS_TABLE, doc)?;
        all.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(all)
    }

    pub fn save_transcript(&mut self, t: &InkTranscript) -> Result<()> {
        self.put_ink(TRANSCRIPTS_TABLE, &t.row_id(), t)
    }

    pub fn transcripts(&self, doc: &str) -> Result<Vec<InkTranscript>> {
        self.ink_rows_of(TRANSCRIPTS_TABLE, doc)
    }

    /// Every transcript and item, for notebook search.
    pub fn ink_search_rows(&self) -> Result<(Vec<InkTranscript>, Vec<NotebookItem>)> {
        Ok((self.ink_all(TRANSCRIPTS_TABLE)?, self.ink_all(ITEMS_TABLE)?))
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::content::Lang;
    use kabar_protocol::ink::{InkTool, ItemKind, TranscriptSource};
    use kabar_protocol::pen::PointerKind;

    use super::*;

    fn stroke(id: &str, doc: &str, n: usize, started_ms: u64) -> InkStroke {
        // A zigzag, so simplification keeps every point.
        InkStroke {
            id: id.into(),
            doc: doc.into(),
            tool: InkTool::Pen,
            color: "ink".into(),
            width: 2.0,
            pointer: PointerKind::Pen,
            xs: (0..n as i32).map(|i| i * 30).collect(),
            ys: (0..n).map(|i| if i % 2 == 0 { 0 } else { 50 }).collect(),
            ps: vec![120; n],
            ts: (0..n as u32).map(|i| i * 8).collect(),
            started_ms,
        }
    }

    fn page(id: &str, notebook: &str, order: u32) -> InkDocument {
        InkDocument {
            id: id.into(),
            kind: DocKind::Page,
            notebook: Some(notebook.into()),
            title: format!("Halaman {order}"),
            lang: Lang::Id,
            order,
            anchor: None,
            size: None,
            created_ms: 1,
        }
    }

    #[test]
    fn notebooks_pages_and_strokes_round_trip() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .save_notebook(&Notebook {
                id: "nb".into(),
                title: "Unit 1".into(),
                unit: Some("bipa1.u01".into()),
                lang: Lang::Id,
                created_ms: 5,
            })
            .unwrap();
        store.save_ink_document(&page("p2", "nb", 2)).unwrap();
        store.save_ink_document(&page("p1", "nb", 1)).unwrap();
        store.save_ink_document(&page("other", "nb2", 1)).unwrap();
        assert_eq!(store.notebooks().unwrap().len(), 1);
        let pages: Vec<_> = store.pages("nb").unwrap().into_iter().map(|d| d.id).collect();
        assert_eq!(pages, ["p1", "p2"]);
        assert_eq!(store.documents(DocKind::Gambar).unwrap(), []);

        store.add_stroke(&stroke("s2", "p1", 10, 200)).unwrap();
        store.add_stroke(&stroke("s1", "p1", 10, 100)).unwrap();
        store.add_stroke(&stroke("x", "p2", 10, 100)).unwrap();
        let ids: Vec<_> = store.strokes("p1").unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["s1", "s2"]);

        // A long stroke is split into joined pieces.
        let pieces = store.add_stroke(&stroke("long", "p2", 4500, 300)).unwrap();
        assert_eq!(pieces.len(), 3);
        assert_eq!(store.strokes("p2").unwrap().len(), 4);

        // Erasing is a tombstone; appending the same stroke again can't revive it.
        store.erase_stroke("s1").unwrap();
        store.add_stroke(&stroke("s1", "p1", 10, 100)).unwrap();
        let ids: Vec<_> = store.strokes("p1").unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["s2"]);

        store.delete_ink_document("p2").unwrap();
        assert!(store.ink_document("p2").unwrap().is_none());
        assert_eq!(store.ink_document("p1").unwrap().unwrap().title, "Halaman 1");
    }

    #[test]
    fn items_and_transcripts_belong_to_their_page() {
        let mut store = Store::open_in_memory().unwrap();
        let card = NotebookItem {
            id: "c1".into(),
            doc: "p1".into(),
            kind: ItemKind::WordCard { lang: Lang::Id, word: "pasar".into() },
            x: 10.0,
            y: 20.0,
            w: 160.0,
            h: 60.0,
        };
        store.save_item(&card).unwrap();
        store.save_item(&NotebookItem { x: 40.0, ..card.clone() }).unwrap();
        assert_eq!(store.items("p1").unwrap()[0].x, 40.0);
        assert!(store.items("p2").unwrap().is_empty());

        let t = InkTranscript {
            doc: "p1".into(),
            group: "s1".into(),
            strokes: vec!["s1".into()],
            text: "Saya sedang membaca buku.".into(),
            lang: Lang::Id,
            source: TranscriptSource::MlKit,
            confidence: Some(0.8),
            bbox: [0.0, 0.0, 100.0, 20.0],
        };
        store.save_transcript(&t).unwrap();
        store.save_transcript(&InkTranscript { source: TranscriptSource::Typed, ..t.clone() }).unwrap();
        let got = store.transcripts("p1").unwrap();
        assert_eq!((got.len(), got[0].source), (1, TranscriptSource::Typed));
        let (ts, items) = store.ink_search_rows().unwrap();
        assert_eq!((ts.len(), items.len()), (1, 1));
        store.delete_item("c1").unwrap();
        assert!(store.items("p1").unwrap().is_empty());
        assert_eq!(store.outbox_len().unwrap(), 5);
    }
}
