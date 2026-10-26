//! Teach Kabar's personal dictionary (spec §26): one LWW row per
//! `{lang}:{word}`; discarding is a tombstone so it syncs.

use kabar_protocol::Hlc;
use kabar_protocol::learn::TaughtEntry;
use rusqlite::params;

use super::{Result, Store, now_ms};

impl Store {
    /// Saves a taught entry (draft or approved), replacing the word's row.
    pub fn save_taught(&mut self, entry: &TaughtEntry) -> Result<Hlc> {
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            "INSERT INTO taught_entries (id, entry, device_id, hlc_wall, hlc_counter, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)
             ON CONFLICT(id) DO UPDATE SET
                entry = excluded.entry, device_id = excluded.device_id,
                hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter, deleted = 0",
            params![
                entry.row_id(),
                serde_json::to_string(entry)?,
                self.device.0,
                hlc.wall_ms as i64,
                hlc.counter
            ],
        )?;
        Ok(hlc)
    }

    /// Discards a draft or forgets an entry.
    pub fn delete_taught(&mut self, row_id: &str) -> Result<()> {
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            "UPDATE taught_entries SET deleted = 1, device_id = ?2, hlc_wall = ?3, hlc_counter = ?4
             WHERE id = ?1",
            params![row_id, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(())
    }

    /// Every live taught entry, in word order.
    pub fn taught_entries(&self) -> Result<Vec<TaughtEntry>> {
        let mut stmt = self.conn.prepare("SELECT entry FROM taught_entries WHERE deleted = 0 ORDER BY id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|json| Ok(serde_json::from_str(&json?)?)).collect()
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::content::ReviewStatus;

    use super::*;

    #[test]
    fn taught_entries_replace_and_tombstone() {
        let mut store = Store::open_in_memory().unwrap();
        let entry: TaughtEntry =
            serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
        let a = store.save_taught(&entry).unwrap();
        let mut approved = entry.clone();
        approved.entry.status = ReviewStatus::Approved;
        let b = store.save_taught(&approved).unwrap();
        assert!(b > a);
        assert_eq!(store.taught_entries().unwrap(), [approved]);
        store.delete_taught(&entry.row_id()).unwrap();
        assert!(store.taught_entries().unwrap().is_empty());
        store.save_taught(&entry).unwrap();
        assert_eq!(store.taught_entries().unwrap(), [entry]);
    }
}
