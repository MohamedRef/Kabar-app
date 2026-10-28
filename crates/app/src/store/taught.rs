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

    /// Every live taught entry, in word order. A row this build can't read
    /// (written by a much newer one, say) is reported and skipped, so one
    /// word never hides the rest of the dictionary.
    pub fn taught_entries(&self) -> Result<Vec<TaughtEntry>> {
        let mut stmt =
            self.conn.prepare("SELECT id, entry FROM taught_entries WHERE deleted = 0 ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, json) = row?;
            if let Some(entry) = read_taught(&id, &json) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }
}

/// Decodes one stored row, reporting it when it can't be read.
pub(super) fn read_taught(id: &str, json: &str) -> Option<TaughtEntry> {
    serde_json::from_str(json)
        .map_err(|e| crate::diag::report!("skipping unreadable taught word {id}: {e}"))
        .ok()
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

    #[test]
    fn a_row_from_a_newer_build_still_loads_and_keeps_its_fields() {
        let mut store = Store::open_in_memory().unwrap();
        let mut json: serde_json::Value =
            serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
        json["mnemonic"] = "kali + mat".into();
        json["entry"]["audio_hint"] = "ka-LI-mat".into();
        let entry: TaughtEntry = serde_json::from_value(json).unwrap();
        assert_eq!(entry.kept.get("/mnemonic").unwrap(), "kali + mat");
        store.save_taught(&entry).unwrap();
        let loaded = store.taught_entries().unwrap();
        assert_eq!(loaded, [entry]);
        // Saving it again (say, after an edit here) writes the newer field back.
        let saved = serde_json::to_value(&loaded[0]).unwrap();
        assert_eq!(saved["mnemonic"], "kali + mat");
        assert_eq!(saved["entry"]["audio_hint"], "ka-LI-mat");
    }

    #[test]
    fn one_unreadable_row_is_skipped_not_the_whole_dictionary() {
        let mut store = Store::open_in_memory().unwrap();
        let entry: TaughtEntry =
            serde_json::from_str(include_str!("../../../../fixtures/learn/taught.kalimat.json")).unwrap();
        store.save_taught(&entry).unwrap();
        store
            .conn
            .execute(
                "INSERT INTO taught_entries (id, entry, device_id, hlc_wall, hlc_counter) \
                 VALUES ('id:zzz', '{\"entry\": 7}', 'dev', 1, 0)",
                [],
            )
            .unwrap();
        assert_eq!(store.taught_entries().unwrap(), [entry]);
    }
}
