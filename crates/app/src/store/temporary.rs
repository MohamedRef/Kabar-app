//! A sign-in on someone else's device ("This isn't my device", #715 item 1).
//! The session never outlives the app or the tab, and neither should what it
//! brought here: the journal, chats, notebooks and progress. While such a
//! session is live the database carries a marker; signing out wipes the
//! account's rows, and so does the next start when the app was closed with
//! the marker still set (the session itself was in memory, so it is gone).
//! The device id stays, like after any sign-out.

use std::collections::BTreeSet;

use rusqlite::Connection;

use super::{Result, Store};

/// Set while a temporary sign-in's rows may be in this database.
const MARKER: &str = "account.temporary";

/// What a wipe keeps in `meta`: who this install is, not whose data it held.
const KEEP_META: &[&str] = &["device_id", "device.marked"];

impl Store {
    /// Marks (or unmarks) this database as holding a temporary sign-in.
    pub fn mark_temporary(&mut self, on: bool) -> Result<()> {
        if on {
            self.set_local_value(MARKER, "1")
        } else {
            self.conn.execute("DELETE FROM meta WHERE key = ?1", [MARKER])?;
            Ok(())
        }
    }

    pub fn is_temporary(&self) -> Result<bool> {
        Ok(self.local_value(MARKER)?.is_some())
    }

    /// Deletes every row the account brought or made here, the outbox and
    /// sync position included, so the next person finds a fresh Kabar.
    /// Returns the tables it emptied (for the screens to read again).
    pub fn wipe_account_data(&mut self) -> Result<BTreeSet<String>> {
        let tables = wipe(&mut self.conn)?;
        // Nothing should linger in free pages or the write-ahead log.
        if let Err(e) = self.conn.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);") {
            crate::diag::report!("wiping a temporary sign-in: compacting: {e}");
        }
        Ok(tables)
    }

    /// At start-up: a temporary sign-in the app was closed on is wiped now.
    pub(super) fn wipe_if_left_behind(conn: &mut Connection) -> Result<()> {
        let marked: bool =
            conn.query_row("SELECT EXISTS(SELECT 1 FROM meta WHERE key = ?1)", [MARKER], |r| r.get(0))?;
        if marked {
            let tables = wipe(conn)?;
            crate::diag::report!("wiped a temporary sign-in left behind ({} tables)", tables.len());
            let _ = conn.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);");
        }
        Ok(())
    }
}

fn wipe(conn: &mut Connection) -> Result<BTreeSet<String>> {
    let tables: BTreeSet<String> = {
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != 'meta'",
        )?;
        stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    let tx = conn.transaction()?;
    // Foreign keys are checked once, at the commit, when every table is empty.
    tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    for table in &tables {
        tx.execute(&format!("DELETE FROM \"{table}\""), [])?;
    }
    let keep = KEEP_META.iter().map(|k| format!("'{k}'")).collect::<Vec<_>>().join(", ");
    tx.execute(&format!("DELETE FROM meta WHERE key NOT IN ({keep})"), [])?;
    tx.commit()?;
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use kabar_protocol::coach::ActivityKind;

    use super::super::Store;

    fn with_progress(store: &mut Store) {
        let day = "2026-11-21".parse().unwrap();
        store.add_activity(ActivityKind::Reviews { cards: 10, correct: 8 }, day, 300).unwrap();
        store.set_last_seq(42).unwrap();
        store.set_local_value("batik", "kawung").unwrap();
    }

    fn rows(store: &Store, table: &str) -> i64 {
        store.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn a_wipe_empties_the_account_but_keeps_the_device() {
        let mut store = Store::open_in_memory().unwrap();
        let device = store.device_id().clone();
        with_progress(&mut store);
        store.mark_temporary(true).unwrap();
        assert!(rows(&store, "xp_events") > 0 && store.outbox_len().unwrap() > 0);

        let tables = store.wipe_account_data().unwrap();

        assert!(tables.contains("xp_events") && tables.contains("outbox"));
        assert_eq!((rows(&store, "xp_events"), store.outbox_len().unwrap()), (0, 0));
        assert_eq!(store.last_seq().unwrap(), 0);
        assert_eq!(store.local_value("batik").unwrap(), None);
        assert!(!store.is_temporary().unwrap());
        assert_eq!(store.device_id(), &device);
        let kept: String =
            store.conn.query_row("SELECT value FROM meta WHERE key = 'device_id'", [], |r| r.get(0)).unwrap();
        assert_eq!(kept, device.0);
    }

    #[test]
    fn a_temporary_sign_in_left_behind_is_wiped_at_the_next_start() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        let device = {
            let mut store = Store::open(&path).unwrap();
            with_progress(&mut store);
            store.mark_temporary(true).unwrap();
            store.device_id().clone()
        };
        let store = Store::open(&path).unwrap();
        assert_eq!((rows(&store, "xp_events"), store.outbox_len().unwrap()), (0, 0));
        assert!(!store.is_temporary().unwrap());
        assert_eq!(store.device_id(), &device);
    }

    #[test]
    fn a_remembered_device_keeps_its_data_across_starts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        {
            let mut store = Store::open(&path).unwrap();
            with_progress(&mut store);
            store.mark_temporary(true).unwrap();
            store.mark_temporary(false).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert!(rows(&store, "xp_events") > 0);
        assert_eq!(store.last_seq().unwrap(), 42);
    }
}
