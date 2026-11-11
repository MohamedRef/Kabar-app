//! Two connections share the database file: the UI's and the sync engine's
//! (`cloud::start`). With WAL only one writes at a time. Both used to wait
//! up to five seconds for the other's lock, which on the UI thread is a
//! frozen screen (engineering audit F16). Now:
//!
//! - the UI waits at most [`UI_BUSY`], its latency budget for a tap. Past
//!   that the write fails and the screen's own saving state takes over (the
//!   lesson summary's «احفظ تاني», the read notice…), never a silent loss;
//! - the sync thread, off the UI thread, keeps waiting up to [`SYNC_BUSY`]:
//!   its transactions are short, so the UI rarely waits on it at all.
// Batna has the same split: batna-app main crates/app/src/store/contention.rs.

use std::time::Duration;

use super::{Result, Store};

/// How long a UI write waits for the sync thread's lock.
pub const UI_BUSY: Duration = Duration::from_millis(750);
/// How long the sync thread waits for the UI's lock.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // the web syncs in-process
pub const SYNC_BUSY: Duration = Duration::from_secs(5);

impl Store {
    /// How long this connection waits for another's write lock.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn set_busy_timeout(&self, wait: Duration) -> Result<()> {
        self.conn.busy_timeout(wait)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use kabar_protocol::sync::OpKind;

    use super::*;

    fn word() -> Option<serde_json::Value> {
        Some(serde_json::json!({ "lexeme": "rumah", "lang": "id", "status": "new" }))
    }

    /// The sync connection holds the write lock: a UI write fails within
    /// its budget instead of freezing for five seconds, and goes through as
    /// soon as the lock is free.
    #[test]
    fn a_ui_write_waits_its_budget_then_fails_and_works_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        let mut ui = Store::open(&path).unwrap();
        let sync = Store::open(&path).unwrap();
        sync.set_busy_timeout(SYNC_BUSY).unwrap();

        sync.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        let t = Instant::now();
        assert!(ui.write_synced("word_status", "id:rumah", OpKind::Upsert, word()).is_err());
        let waited = t.elapsed();
        assert!(waited >= UI_BUSY.mul_f32(0.9), "it waited for the lock: {waited:?}");
        assert!(waited < UI_BUSY * 3, "within the budget, not five seconds: {waited:?}");

        sync.conn.execute_batch("COMMIT").unwrap();
        ui.write_synced("word_status", "id:rumah", OpKind::Upsert, word()).unwrap();
        assert_eq!(sync.outbox_len().unwrap(), 1);
    }
}
