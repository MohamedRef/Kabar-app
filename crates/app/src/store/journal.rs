//! The learning journal (spec §6). Reflections are one LWW row each in
//! `journal_entries` (edits replace, deleting is a tombstone, so both sync).
//! The rest of the timeline is read from rows other features already keep:
//! finished lessons from `lesson_events`, taught words from `taught_entries`.
//!
//! Reflections are personal: nothing here is ever handed to the AI router.

use kabar_protocol::Hlc;
use kabar_protocol::journal::{JOURNAL_TABLE, JournalEntry};
use kabar_protocol::learn::TaughtEntry;
use kabar_protocol::sync::OpKind;
use serde_json::Value;

use super::{Result, Store};

/// A finished lesson or passed level test, for the timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LessonDone {
    pub lesson_id: String,
    /// When it was finished (the event's HLC wall clock).
    pub at_ms: u64,
    /// First-try score, 0–100, when the event carries one.
    pub score: Option<u32>,
}

/// A taught word and when it was last saved (taught, edited or approved).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaughtAt {
    pub entry: TaughtEntry,
    pub at_ms: u64,
}

impl Store {
    /// Saves a reflection (new or edited), replacing its row.
    pub fn save_reflection(&mut self, entry: &JournalEntry) -> Result<Hlc> {
        let payload = serde_json::to_value(entry)?;
        Ok(self.write_synced(JOURNAL_TABLE, &entry.id, OpKind::Upsert, Some(payload))?.hlc)
    }

    /// Deletes a reflection (a tombstone, so it goes on every device).
    pub fn delete_reflection(&mut self, id: &str) -> Result<()> {
        self.write_synced(JOURNAL_TABLE, id, OpKind::Delete, None).map(drop)
    }

    /// Every live reflection, newest day first, then in writing order.
    pub fn reflections(&self) -> Result<Vec<JournalEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT entry FROM journal_entries WHERE deleted = 0
             ORDER BY json_extract(entry, '$.date') DESC, json_extract(entry, '$.created_ms'), id",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|json| Ok(serde_json::from_str(&json?)?)).collect()
    }

    /// Finished lessons and level tests, oldest first. A lesson finished
    /// twice shows twice: both are things the learner did.
    pub fn lesson_completions(&self) -> Result<Vec<LessonDone>> {
        let mut stmt = self.conn.prepare(
            "SELECT lesson_id, hlc_wall, payload FROM lesson_events
             WHERE kind = 'completed' AND deleted = 0 ORDER BY hlc_wall, hlc_counter",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))?;
        rows.map(|row| {
            let (lesson_id, wall, payload) = row?;
            // Lenient: an older or partial payload still shows the lesson.
            let score = serde_json::from_str::<Value>(&payload)
                .ok()
                .and_then(|p| p.get("score")?.as_u64())
                .map(|s| s.min(100) as u32);
            Ok(LessonDone { lesson_id, at_ms: wall as u64, score })
        })
        .collect()
    }

    /// Live taught entries with the time of their last save.
    pub fn taught_with_time(&self) -> Result<Vec<TaughtAt>> {
        let mut stmt = self
            .conn
            .prepare("SELECT entry, hlc_wall FROM taught_entries WHERE deleted = 0 ORDER BY hlc_wall")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        rows.map(|row| {
            let (json, wall) = row?;
            Ok(TaughtAt { entry: serde_json::from_str(&json)?, at_ms: wall as u64 })
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::LocalDate;
    use kabar_protocol::sync::SeqOp;
    use rusqlite::Connection;

    use super::super::MIGRATIONS;
    use super::*;

    fn day(d: u32) -> LocalDate {
        LocalDate::from_ymd(2026, 10, d).unwrap()
    }

    fn note(id: &str, date: LocalDate, text: &str, at: u64) -> JournalEntry {
        JournalEntry { id: id.into(), date, text: text.into(), ink: None, created_ms: at, updated_ms: at }
    }

    #[test]
    fn reflections_save_edit_and_tombstone() {
        let mut store = Store::open_in_memory().unwrap();
        let a = note("a", day(25), "Hari ini saya belajar kata baru.", 1);
        let b = note("b", day(26), "Saya sedang belajar bahasa Indonesia.", 2);
        let c = note("c", day(26), "Besok saya mau ke pasar.", 3);
        for e in [&a, &b, &c] {
            store.save_reflection(e).unwrap();
        }
        assert_eq!(store.reflections().unwrap(), [b.clone(), c.clone(), a.clone()]);

        let edited =
            JournalEntry { text: "Sudah ke pasar!\nMurah sekali.".into(), updated_ms: 9, ..c.clone() };
        let h1 = store.save_reflection(&edited).unwrap();
        assert_eq!(store.reflections().unwrap(), [b.clone(), edited.clone(), a.clone()]);

        store.delete_reflection("b").unwrap();
        assert_eq!(store.reflections().unwrap(), [edited, a]);

        // Every write is queued for sync, in order: 3 saves, 1 edit, 1 delete.
        let ops = store.outbox(10).unwrap();
        assert_eq!(ops.len(), 5);
        assert!(ops.iter().all(|o| o.table == JOURNAL_TABLE));
        assert_eq!(ops[3].hlc, h1);
        assert_eq!(ops[4].kind, OpKind::Delete);
    }

    #[test]
    fn reflections_sync_and_the_newest_edit_wins() {
        let mut phone = Store::open_in_memory().unwrap();
        let mut tablet = Store::open_in_memory().unwrap();
        let first = note("r1", day(26), "Saya sedang belajar bahasa Indonesia.", 1);
        phone.save_reflection(&first).unwrap();
        let relay = |from: &Store| -> Vec<SeqOp> {
            from.outbox(50)
                .unwrap()
                .into_iter()
                .enumerate()
                .map(|(i, op)| SeqOp { seq: i as u64 + 1, op })
                .collect()
        };
        let ops = relay(&phone);
        let changed = tablet.apply_remote(&ops, 1).unwrap();
        assert!(changed.contains(JOURNAL_TABLE));
        assert_eq!(tablet.reflections().unwrap(), std::slice::from_ref(&first));

        // Edited on the tablet later: the phone takes the edit.
        let edit = JournalEntry { text: "Sudah bisa!".into(), updated_ms: 5, ..first.clone() };
        tablet.save_reflection(&edit).unwrap();
        let op = tablet.outbox(50).unwrap().pop().unwrap();
        phone.apply_remote(&[SeqOp { seq: 2, op }], 2).unwrap();
        assert_eq!(phone.reflections().unwrap(), [edit]);

        // Deleted on the phone: gone on the tablet too.
        phone.delete_reflection("r1").unwrap();
        let op = phone.outbox(50).unwrap().pop().unwrap();
        tablet.apply_remote(&[SeqOp { seq: 3, op }], 3).unwrap();
        assert!(tablet.reflections().unwrap().is_empty());
    }

    #[test]
    fn migration_keeps_reflections_synced_before_the_table_existed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        let early = note("early", day(24), "Selamat pagi!", 1);
        {
            // A build at schema 7 kept another device's reflection in `sync_rows` only.
            let mut conn = Connection::open(&path).unwrap();
            for (i, sql) in MIGRATIONS[..7].iter().enumerate() {
                let tx = conn.transaction().unwrap();
                tx.execute_batch(sql).unwrap();
                tx.pragma_update(None, "user_version", i as u32 + 1).unwrap();
                tx.commit().unwrap();
            }
            conn.execute(
                "INSERT INTO sync_rows (tbl, row_id, payload, device_id, hlc_wall, hlc_counter)
                 VALUES ('journal_entries', 'early', ?1, 'tab-s11', 7, 0)",
                [serde_json::to_string(&early).unwrap()],
            )
            .unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.reflections().unwrap(), [early]);
        let version: u32 = store.conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version as usize, MIGRATIONS.len());
    }

    #[test]
    fn lesson_completions_and_taught_words_have_times() {
        let mut store = Store::open_in_memory().unwrap();
        store.record_lesson_event("bipa1.u01.l01", "completed", &serde_json::json!({ "score": 81 })).unwrap();
        store.record_lesson_event("bipa1.u01.l01", "started", &serde_json::json!({})).unwrap();
        store.record_lesson_event("bipa1.test", "completed", &serde_json::json!({})).unwrap();
        let done = store.lesson_completions().unwrap();
        assert_eq!(
            done.iter().map(|d| d.lesson_id.as_str()).collect::<Vec<_>>(),
            ["bipa1.u01.l01", "bipa1.test"]
        );
        assert_eq!(done[0].score, Some(81));
        assert_eq!(done[1].score, None);
        assert!(done[0].at_ms > 0 && done[1].at_ms >= done[0].at_ms);

        let entry: TaughtEntry =
            serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
        let hlc = store.save_taught(&entry).unwrap();
        assert_eq!(store.taught_with_time().unwrap(), [TaughtAt { entry, at_ms: hlc.wall_ms }]);
    }
}
