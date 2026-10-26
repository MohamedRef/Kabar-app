//! Teach Kabar's personal dictionary (spec §26): one LWW row per
//! `{lang}:{word}`; discarding is a tombstone so it syncs.

use kabar_protocol::Hlc;
use kabar_protocol::learn::TaughtEntry;
use kabar_protocol::sync::OpKind;

use super::{Result, Store};

impl Store {
    /// Saves a taught entry (draft or approved), replacing the word's row.
    pub fn save_taught(&mut self, entry: &TaughtEntry) -> Result<Hlc> {
        let payload = serde_json::to_value(entry)?;
        Ok(self.write_synced("taught_entries", &entry.row_id(), OpKind::Upsert, Some(payload))?.hlc)
    }

    /// Discards a draft or forgets an entry (a tombstone, so it syncs).
    pub fn delete_taught(&mut self, row_id: &str) -> Result<()> {
        self.write_synced("taught_entries", row_id, OpKind::Delete, None).map(drop)
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
