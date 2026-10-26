//! Reader tables: texts (LWW), word statuses (LWW per lexeme), highlights
//! (LWW), saved words and quality feedback (append-only).

use std::collections::HashMap;

use kabar_protocol::reader::WordStatus;
use rusqlite::{OptionalExtension, params};

use super::{Result, Store, now_ms};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Pasted,
    Shared,
    Screenshot,
}

impl TextKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pasted => "pasted",
            Self::Shared => "shared",
            Self::Screenshot => "screenshot",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "shared" => Self::Shared,
            "screenshot" => Self::Screenshot,
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

impl Store {
    pub fn save_text(
        &mut self,
        body: &str,
        kind: TextKind,
        source: Option<&str>,
        layout: Option<&str>,
    ) -> Result<SavedText> {
        let hlc = self.clock.tick(now_ms());
        let id = uuid::Uuid::new_v4().to_string();
        let title = title_of(body);
        self.conn.execute(
            "INSERT INTO texts (id, title, body, kind, source, layout, device_id, hlc_wall, hlc_counter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                title,
                body,
                kind.as_str(),
                source,
                layout,
                self.device.0,
                hlc.wall_ms as i64,
                hlc.counter
            ],
        )?;
        Ok(SavedText {
            id,
            title,
            body: body.into(),
            kind,
            source: source.map(Into::into),
            layout: layout.map(Into::into),
        })
    }

    /// Newest first.
    pub fn recent_texts(&self, limit: usize) -> Result<Vec<SavedText>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, body, kind, source, layout FROM texts WHERE deleted = 0
             ORDER BY hlc_wall DESC, hlc_counter DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| {
            Ok(SavedText {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                kind: TextKind::parse(&r.get::<_, String>(3)?),
                source: r.get(4)?,
                layout: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_text(&mut self, id: &str) -> Result<()> {
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            "UPDATE texts SET deleted = 1, device_id = ?2, hlc_wall = ?3, hlc_counter = ?4 WHERE id = ?1",
            params![id, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(())
    }

    /// Statuses for the given lexemes; missing ones are `New`.
    pub fn word_statuses<'a>(
        &self,
        lexemes: impl IntoIterator<Item = &'a str>,
    ) -> Result<HashMap<String, WordStatus>> {
        let mut stmt =
            self.conn.prepare_cached("SELECT status FROM word_status WHERE lexeme = ?1 AND deleted = 0")?;
        let mut out = HashMap::new();
        for lexeme in lexemes {
            if out.contains_key(lexeme) {
                continue;
            }
            let status = stmt
                .query_row([lexeme], |r| r.get::<_, String>(0))
                .optional()?
                .and_then(|s| WordStatus::parse(&s))
                .unwrap_or_default();
            out.insert(lexeme.to_string(), status);
        }
        Ok(out)
    }

    pub fn set_word_status(&mut self, lexeme: &str, status: WordStatus) -> Result<()> {
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            "INSERT INTO word_status (lexeme, status, device_id, hlc_wall, hlc_counter, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)
             ON CONFLICT(lexeme) DO UPDATE SET
                status = excluded.status, device_id = excluded.device_id,
                hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter, deleted = 0",
            params![lexeme, status.as_str(), self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(())
    }

    pub fn add_highlight(
        &mut self,
        text_id: &str,
        start: usize,
        end: usize,
        phrase: &str,
    ) -> Result<Highlight> {
        let hlc = self.clock.tick(now_ms());
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO highlights (id, text_id, start_byte, end_byte, phrase, device_id, hlc_wall, hlc_counter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, text_id, start as i64, end as i64, phrase, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(Highlight { id, text_id: text_id.into(), start, end, phrase: phrase.into() })
    }

    pub fn remove_highlight(&mut self, id: &str) -> Result<()> {
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            "UPDATE highlights SET deleted = 1, device_id = ?2, hlc_wall = ?3, hlc_counter = ?4 WHERE id = ?1",
            params![id, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(())
    }

    pub fn highlights(&self, text_id: &str) -> Result<Vec<Highlight>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text_id, start_byte, end_byte, phrase FROM highlights
             WHERE text_id = ?1 AND deleted = 0 ORDER BY start_byte",
        )?;
        let rows = stmt.query_map([text_id], |r| {
            Ok(Highlight {
                id: r.get(0)?,
                text_id: r.get(1)?,
                start: r.get::<_, i64>(2)? as usize,
                end: r.get::<_, i64>(3)? as usize,
                phrase: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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

    /// 👍 / 👎 on an answer. Append-only; the quality dashboard reads it.
    pub fn add_feedback(&mut self, task: &str, item: &str, up: bool) -> Result<()> {
        let hlc = self.clock.tick(now_ms());
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO quality_feedback (id, task, item, vote, device_id, hlc_wall, hlc_counter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, task, item, if up { 1 } else { -1 }, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
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
    fn statuses_default_to_new_and_last_write_wins() {
        let mut s = Store::open_in_memory().unwrap();
        s.set_word_status("baca", WordStatus::Learning).unwrap();
        s.set_word_status("baca", WordStatus::Known).unwrap();
        let st = s.word_statuses(["baca", "pusat"]).unwrap();
        assert_eq!(st["baca"], WordStatus::Known);
        assert_eq!(st["pusat"], WordStatus::New);
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
        s.set_word_status("kabar", WordStatus::Known).unwrap();
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
