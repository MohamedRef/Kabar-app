//! Reader tables: texts, word statuses (per `{lang}:{lexeme}`) and highlights
//! sync last-writer-wins; quality feedback syncs append-only; saved words stay
//! on the device.

use std::collections::HashMap;

use kabar_protocol::reader::WordStatus;
use kabar_protocol::sync::{MAX_PAYLOAD_BYTES, OpKind};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{Result, Store, now_ms};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Pasted,
    Shared,
    Screenshot,
    /// Imported from a link; `source` is the URL.
    Web,
}

impl TextKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pasted => "pasted",
            Self::Shared => "shared",
            Self::Screenshot => "screenshot",
            Self::Web => "web",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "shared" => Self::Shared,
            "screenshot" => Self::Screenshot,
            "web" => Self::Web,
            _ => Self::Pasted,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavedText {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: TextKind,
    pub source: Option<String>,
    /// For screenshots: the structured layout (JSON of `ScreenDoc`).
    pub layout: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Highlight {
    pub id: String,
    pub text_id: String,
    /// Byte range in the text's body.
    pub start: usize,
    pub end: usize,
    pub phrase: String,
}

/// What syncs for a text; `id` is the row id.
#[derive(Serialize, Deserialize)]
struct TextRow {
    title: String,
    body: String,
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layout: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct HighlightRow {
    text_id: String,
    start: usize,
    end: usize,
    phrase: String,
}

/// `{lang}:{lexeme}` → (lang, lexeme). Keys without a language are Indonesian.
fn split_key(key: &str) -> (&str, &str) {
    key.split_once(':').unwrap_or(("id", key))
}

impl Store {
    /// Saves and syncs a text. A text too big for one sync op (a very long
    /// article, roughly 30+ pages) stays on this device.
    pub fn save_text(
        &mut self,
        body: &str,
        kind: TextKind,
        source: Option<&str>,
        layout: Option<&str>,
    ) -> Result<SavedText> {
        self.insert_text(title_of(body), body, kind, source, layout)
    }

    /// A page imported from a link, under the page's own title.
    pub fn save_web_text(&mut self, title: Option<&str>, body: &str, url: &str) -> Result<SavedText> {
        let title = title.map(str::trim).filter(|t| !t.is_empty()).map_or_else(|| title_of(body), title_of);
        self.insert_text(title, body, TextKind::Web, Some(url), None)
    }

    fn insert_text(
        &mut self,
        title: String,
        body: &str,
        kind: TextKind,
        source: Option<&str>,
        layout: Option<&str>,
    ) -> Result<SavedText> {
        let id = uuid::Uuid::new_v4().to_string();
        let row = TextRow {
            title: title.clone(),
            body: body.into(),
            kind: kind.as_str().into(),
            source: source.map(Into::into),
            layout: layout.map(Into::into),
        };
        self.put_text(&id, &row)?;
        Ok(SavedText {
            id,
            title,
            body: body.into(),
            kind,
            source: source.map(Into::into),
            layout: layout.map(Into::into),
        })
    }

    /// Replaces a saved text's body and layout, keeping its id, kind and
    /// source (the AI pass restructured a screenshot). Last write wins.
    pub fn update_text(&mut self, text: &SavedText, body: &str, layout: Option<&str>) -> Result<SavedText> {
        let title = title_of(body);
        let row = TextRow {
            title: title.clone(),
            body: body.into(),
            kind: text.kind.as_str().into(),
            source: text.source.clone(),
            layout: layout.map(Into::into),
        };
        self.put_text(&text.id, &row)?;
        Ok(SavedText { title, body: body.into(), layout: layout.map(Into::into), ..text.clone() })
    }

    fn put_text(&mut self, id: &str, row: &TextRow) -> Result<()> {
        let payload = serde_json::to_value(row)?;
        if serde_json::to_string(&payload)?.len() < MAX_PAYLOAD_BYTES - 1024 {
            self.write_synced("texts", id, OpKind::Upsert, Some(payload))?;
        } else {
            let hlc = self.clock.tick(now_ms());
            self.conn.execute(
                "INSERT INTO texts (id, payload, device_id, hlc_wall, hlc_counter) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET payload = excluded.payload,
                     hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter",
                params![id, payload.to_string(), self.device.0, hlc.wall_ms as i64, hlc.counter],
            )?;
        }
        Ok(())
    }

    /// Newest first.
    pub fn recent_texts(&self, limit: usize) -> Result<Vec<SavedText>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, payload FROM texts WHERE deleted = 0
             ORDER BY hlc_wall DESC, hlc_counter DESC LIMIT ?1",
        )?;
        let rows: Vec<(String, String)> = stmt
            .query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, json)| {
                let row: TextRow = serde_json::from_str(&json).ok()?;
                Some(SavedText {
                    id,
                    title: row.title,
                    body: row.body,
                    kind: TextKind::parse(&row.kind),
                    source: row.source,
                    layout: row.layout,
                })
            })
            .collect())
    }

    pub fn delete_text(&mut self, id: &str) -> Result<()> {
        self.write_synced("texts", id, OpKind::Delete, None)?;
        Ok(())
    }

    /// Statuses for the given `{lang}:{lexeme}` keys; missing ones are `New`.
    pub fn word_statuses<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a str>,
    ) -> Result<HashMap<String, WordStatus>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT json_extract(payload, '$.status') FROM word_status WHERE id = ?1 AND deleted = 0",
        )?;
        let mut out = HashMap::new();
        for key in keys {
            if out.contains_key(key) {
                continue;
            }
            let status = stmt
                .query_row([key], |r| r.get::<_, Option<String>>(0))
                .optional()?
                .flatten()
                .and_then(|s| WordStatus::parse(&s))
                .unwrap_or_default();
            out.insert(key.to_string(), status);
        }
        Ok(out)
    }

    /// Last writer wins across devices, per `{lang}:{lexeme}`.
    pub fn set_word_status(&mut self, key: &str, status: WordStatus) -> Result<()> {
        let (lang, lexeme) = split_key(key);
        let payload = serde_json::json!({ "lexeme": lexeme, "lang": lang, "status": status.as_str() });
        self.write_synced("word_status", key, OpKind::Upsert, Some(payload))?;
        Ok(())
    }

    pub fn add_highlight(
        &mut self,
        text_id: &str,
        start: usize,
        end: usize,
        phrase: &str,
    ) -> Result<Highlight> {
        let id = uuid::Uuid::new_v4().to_string();
        let row = HighlightRow { text_id: text_id.into(), start, end, phrase: phrase.into() };
        self.write_synced("highlights", &id, OpKind::Upsert, Some(serde_json::to_value(&row)?))?;
        Ok(Highlight { id, text_id: text_id.into(), start, end, phrase: phrase.into() })
    }

    pub fn remove_highlight(&mut self, id: &str) -> Result<()> {
        self.write_synced("highlights", id, OpKind::Delete, None)?;
        Ok(())
    }

    pub fn highlights(&self, text_id: &str) -> Result<Vec<Highlight>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, payload FROM highlights
             WHERE json_extract(payload, '$.text_id') = ?1 AND deleted = 0",
        )?;
        let rows: Vec<(String, String)> =
            stmt.query_map([text_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut out: Vec<Highlight> = rows
            .into_iter()
            .filter_map(|(id, json)| {
                let h: HighlightRow = serde_json::from_str(&json).ok()?;
                Some(Highlight { id, text_id: h.text_id, start: h.start, end: h.end, phrase: h.phrase })
            })
            .collect();
        out.sort_by_key(|h| h.start);
        Ok(out)
    }

    /// "Add to review": remembers the word with the sentence it was met in,
    /// so M6 can build a cloze card from it. Append-only.
    pub fn save_word(
        &mut self,
        lexeme: &str,
        surface: &str,
        sentence: &str,
        text_id: Option<&str>,
    ) -> Result<()> {
        let hlc = self.clock.tick(now_ms());
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO saved_words (id, lexeme, surface, sentence, text_id, device_id, hlc_wall, hlc_counter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, lexeme, surface, sentence, text_id, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(())
    }

    pub fn saved_word_count(&self, lexeme: &str) -> Result<usize> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM saved_words WHERE lexeme = ?1 AND deleted = 0",
            [lexeme],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// 👍 / 👎 on an answer. Append-only and synced; the quality dashboard
    /// reads it from `synced_rows("quality_feedback")`.
    pub fn add_feedback(&mut self, task: &str, item: &str, up: bool) -> Result<()> {
        let id = uuid::Uuid::now_v7().to_string();
        let payload = serde_json::json!({ "task": task, "item": item, "vote": if up { 1 } else { -1 } });
        self.write_synced("quality_feedback", &id, OpKind::Append, Some(payload))?;
        Ok(())
    }
}

/// The first line, trimmed to about 60 characters on a char boundary.
fn title_of(body: &str) -> String {
    let first = body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    match first.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", first[..i].trim_end()),
        None => first.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_newest_first_and_deletable() {
        let mut s = Store::open_in_memory().unwrap();
        let a = s.save_text("Saya sedang membaca buku.", TextKind::Pasted, None, None).unwrap();
        let b = s
            .save_text("Sek tak lapor pusat\nng prabowo?", TextKind::Shared, Some("com.whatsapp"), None)
            .unwrap();
        let texts = s.recent_texts(10).unwrap();
        assert_eq!(texts.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), [b.id.as_str(), a.id.as_str()]);
        assert_eq!(texts[0].title, "Sek tak lapor pusat");
        s.delete_text(&a.id).unwrap();
        assert_eq!(s.recent_texts(10).unwrap().len(), 1);
    }

    #[test]
    fn update_text_keeps_id_kind_and_source() {
        let mut s = Store::open_in_memory().unwrap();
        let t = s
            .save_text(
                "Hari ini\nSek tak lapor pusat",
                TextKind::Screenshot,
                Some("com.whatsapp"),
                Some("{}"),
            )
            .unwrap();
        let u = s.update_text(&t, "Sek tak lapor pusat\nng prabowo?", Some(r#"{"items":[]}"#)).unwrap();
        assert_eq!(
            (u.id.as_str(), u.kind, u.source.as_deref()),
            (t.id.as_str(), t.kind, Some("com.whatsapp"))
        );
        let texts = s.recent_texts(10).unwrap();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], u);
        assert_eq!(texts[0].title, "Sek tak lapor pusat");
    }

    #[test]
    fn statuses_default_to_new_and_last_write_wins() {
        let mut s = Store::open_in_memory().unwrap();
        s.set_word_status("id:baca", WordStatus::Learning).unwrap();
        s.set_word_status("id:baca", WordStatus::Known).unwrap();
        let st = s.word_statuses(["id:baca", "id:pusat"]).unwrap();
        assert_eq!(st["id:baca"], WordStatus::Known);
        assert_eq!(st["id:pusat"], WordStatus::New);
    }

    #[test]
    fn reader_writes_go_through_sync() {
        let mut s = Store::open_in_memory().unwrap();
        s.set_word_status("jv:arep", WordStatus::Learning).unwrap();
        let t = s.save_text("arp dicul piro", TextKind::Pasted, None, None).unwrap();
        s.add_highlight(&t.id, 4, 9, "dicul").unwrap();
        s.add_feedback("gloss", "jv:cul", false).unwrap();
        let ops = s.outbox(10).unwrap();
        let tables: Vec<&str> = ops.iter().map(|o| o.table.as_str()).collect();
        assert_eq!(tables, ["word_status", "texts", "highlights", "quality_feedback"]);
        assert_eq!(ops[0].row_id, "jv:arep");
        assert_eq!(ops[0].payload.as_ref().unwrap()["lexeme"], "arep");
        assert_eq!(ops[0].payload.as_ref().unwrap()["lang"], "jv");
        assert_eq!(ops[3].kind, OpKind::Append);
        assert_eq!(s.synced_rows("quality_feedback").unwrap().len(), 1);
    }

    #[test]
    fn remote_reader_rows_show_up() {
        let mut a = Store::open_in_memory().unwrap();
        let mut b = Store::open_in_memory().unwrap();
        a.set_word_status("id:kabar", WordStatus::Known).unwrap();
        let t = a.save_text("Apa kabar?", TextKind::Pasted, None, None).unwrap();
        let h = a.add_highlight(&t.id, 0, 3, "Apa").unwrap();
        let ops: Vec<_> = a
            .outbox(10)
            .unwrap()
            .into_iter()
            .enumerate()
            .map(|(i, op)| kabar_protocol::sync::SeqOp { seq: i as u64 + 1, op })
            .collect();
        b.apply_remote(&ops, ops.len() as u64).unwrap();
        assert_eq!(b.word_statuses(["id:kabar"]).unwrap()["id:kabar"], WordStatus::Known);
        assert_eq!(b.recent_texts(5).unwrap()[0].body, "Apa kabar?");
        assert_eq!(b.highlights(&t.id).unwrap(), [h]);
    }

    #[test]
    fn huge_texts_stay_on_the_device() {
        let mut s = Store::open_in_memory().unwrap();
        let body = "Saya sedang membaca buku. ".repeat(4000);
        let t = s.save_text(&body, TextKind::Pasted, None, None).unwrap();
        assert!(s.outbox(10).unwrap().is_empty());
        assert_eq!(s.recent_texts(1).unwrap()[0].id, t.id);
    }

    #[test]
    fn highlights_and_saved_words() {
        let mut s = Store::open_in_memory().unwrap();
        let t = s.save_text("arp dicul piro", TextKind::Pasted, None, None).unwrap();
        let h = s.add_highlight(&t.id, 4, 9, "dicul").unwrap();
        assert_eq!(s.highlights(&t.id).unwrap(), vec![h.clone()]);
        s.remove_highlight(&h.id).unwrap();
        assert!(s.highlights(&t.id).unwrap().is_empty());
        s.save_word("cul", "dicul", "arp dicul piro", Some(&t.id)).unwrap();
        assert_eq!(s.saved_word_count("cul").unwrap(), 1);
        s.add_feedback("gloss", "cul", true).unwrap();
    }

    #[test]
    fn titles_are_short_first_lines() {
        assert_eq!(title_of("\n  piye dhe  \nx"), "piye dhe");
        let long = "iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan minat wa ".repeat(2);
        assert!(title_of(&long).ends_with('…'));
    }

    #[test]
    fn clock_resumes_past_reader_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("k.db");
        let mut s = Store::open(&path).unwrap();
        s.set_word_status("id:kabar", WordStatus::Known).unwrap();
        let written: (i64, u32) = s
            .conn
            .query_row("SELECT hlc_wall, hlc_counter FROM word_status", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        drop(s);
        let mut s = Store::open(&path).unwrap();
        let next = s.clock.tick(0);
        assert!((next.wall_ms as i64, next.counter) > written);
    }
}
