//! Account-scoped sync state, borrowed-device cleanup (#715) and deferred
//! remote ops from newer builds (#687).

use std::collections::BTreeSet;

use kabar_core::clock::HlcClock;
use kabar_core::sync::{Merge, merge, validate};
use kabar_protocol::sync::{MAX_PAYLOAD_BYTES, Op};
use rusqlite::Connection;

use super::sync_rows::{materialize, meta, row_meta, set_meta, store_row};
use super::{Result, Store, now_ms};

const BORROWED_KEY: &str = "local.borrowed";
const ACCOUNT_KEY: &str = "sync_account";
const OTHER_ACCOUNT: &str = "other_account";

/// Wipes every non-`meta` table and resets account-scoped `meta` keys, keeping
/// only this installation's `device_id` and per-device preferences (`device.*`).
pub(super) fn wipe_study_tables(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    let tables: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != 'meta'",
        )?;
        stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    for name in tables {
        tx.execute(&format!("DELETE FROM \"{name}\""), [])?;
    }
    tx.execute("DELETE FROM meta WHERE key != 'device_id' AND key NOT LIKE 'device.%'", [])?;
    tx.commit()?;
    Ok(())
}

/// On launch, if the previous run signed in with "This isn't my device" and
/// closed without signing out, wipes the synced study data before the UI reads
/// anything (#715). On the web, an active tab reload still holds its session in
/// `sessionStorage`, so it keeps its in-memory state until the tab closes.
pub(super) fn clean_borrowed_on_boot(conn: &mut Connection) -> Result<()> {
    if meta(conn, BORROWED_KEY)?.as_deref() != Some("1") {
        return Ok(());
    }
    if crate::cloud::session::tab_session_kept() {
        return Ok(());
    }
    wipe_study_tables(conn)
}

/// Replays remote ops an older build couldn't validate yet once this build
/// knows their table and sync kind (#687).
pub(super) fn replay_deferred(conn: &mut Connection, clock: &mut HlcClock) -> Result<()> {
    let rows: Vec<(i64, String)> = {
        let mut stmt = conn.prepare("SELECT seq, op FROM deferred_ops ORDER BY seq")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
    };
    if rows.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    let now = now_ms();
    for (seq, json) in rows {
        let Ok(op) = serde_json::from_str::<Op>(&json) else {
            tx.execute("DELETE FROM deferred_ops WHERE seq = ?1", [seq])?;
            continue;
        };
        let Ok(sync) = validate(&op, MAX_PAYLOAD_BYTES) else { continue };
        clock.observe(&op.hlc, now);
        if merge(sync, &op, row_meta(&tx, &op.table, &op.row_id)?.as_ref()) == Merge::Apply {
            store_row(&tx, &op)?;
            materialize(&tx, &op)?;
        }
        tx.execute("DELETE FROM deferred_ops WHERE seq = ?1", [seq])?;
    }
    tx.commit()?;
    Ok(())
}

impl Store {
    pub fn is_borrowed(&self) -> bool {
        meta(&self.conn, BORROWED_KEY).ok().flatten().as_deref() == Some("1")
    }

    /// Prepares the store for a newly signed-in session: binds `sync_account`
    /// (resetting `last_seq` and holding the old account's outbox if another
    /// account was signed in here before, #687) and marks temporary sessions
    /// so sign-out or the next launch leaves no study data behind (#715).
    pub fn begin_account(&mut self, account_id: &str, borrowed: bool) -> Result<()> {
        let tx = self.conn.transaction()?;
        match meta(&tx, ACCOUNT_KEY)? {
            Some(prev) if prev != account_id => {
                super::held::hold_all(&tx, OTHER_ACCOUNT, now_ms())?;
                tx.execute("DELETE FROM meta WHERE key IN ('last_seq', 'last_sync_ms')", [])?;
                set_meta(&tx, ACCOUNT_KEY, account_id)?;
            }
            None => set_meta(&tx, ACCOUNT_KEY, account_id)?,
            Some(_) => {}
        }
        if borrowed {
            set_meta(&tx, BORROWED_KEY, "1")?;
        } else {
            tx.execute("DELETE FROM meta WHERE key = ?1", [BORROWED_KEY])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Wipes every study table, cache, outbox and sync cursor after a
    /// temporary ("This isn't my device") session signs out (#715). Returns
    /// the UI table names so open screens reload to a clean state.
    pub fn forget_study(&mut self) -> Result<BTreeSet<String>> {
        wipe_study_tables(&mut self.conn)?;
        self.clock = HlcClock::new(self.device.clone());
        Ok([
            "settings",
            "goals",
            "daily_plans",
            "xp_events",
            "reminders",
            "reports",
            "taught_entries",
            "word_status",
            "journal_entries",
            "texts",
            "cards",
            "lesson_events",
        ]
        .into_iter()
        .map(String::from)
        .collect())
    }
}

#[cfg(test)]
mod tests {
    use kabar_protocol::sync::{Op, OpKind, SeqOp};
    use kabar_protocol::{DeviceId, Hlc};

    use super::*;

    #[test]
    fn deferred_ops_for_future_tables_replay_once_known() {
        let mut s = Store::open_in_memory().unwrap();
        let future = SeqOp {
            seq: 1,
            op: Op {
                id: "op-future".into(),
                table: "future_notes".into(),
                row_id: "r1".into(),
                kind: OpKind::Upsert,
                hlc: Hlc { wall_ms: 100, counter: 0, device: DeviceId("tab".into()) },
                payload: Some(serde_json::json!({ "text": "halo" })),
            },
        };
        assert!(s.apply_remote(&[future], 1).unwrap().is_empty());
        assert_eq!(s.last_seq().unwrap(), 1);
        let kept: i64 = s.conn.query_row("SELECT COUNT(*) FROM deferred_ops", [], |r| r.get(0)).unwrap();
        assert_eq!(kept, 1, "unknown table op is deferred instead of lost");

        // Simulate an app update where the deferred row is now a known table (`word_status`).
        let known = Op {
            id: "op-known".into(),
            table: "word_status".into(),
            row_id: "id:rumah".into(),
            kind: OpKind::Upsert,
            hlc: Hlc { wall_ms: 100, counter: 0, device: DeviceId("tab".into()) },
            payload: Some(serde_json::json!({ "lexeme": "rumah", "lang": "id", "status": "known" })),
        };
        let encoded = serde_json::to_string(&known).unwrap();
        s.conn.execute("UPDATE deferred_ops SET op = ?1 WHERE seq = 1", [encoded]).unwrap();
        let (_, bytes) = s.snapshot().unwrap();
        let upgraded = Store::from_snapshot(&bytes).unwrap();
        assert_eq!(upgraded.synced_rows("word_status").unwrap().len(), 1);
        let remaining: i64 =
            upgraded.conn.query_row("SELECT COUNT(*) FROM deferred_ops", [], |r| r.get(0)).unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn switching_accounts_resets_last_seq_and_holds_previous_outbox() {
        let mut s = Store::open_in_memory().unwrap();
        s.begin_account("acc-a", false).unwrap();
        s.set_last_seq(42).unwrap();
        let word = serde_json::json!({ "lexeme": "rumah", "lang": "id", "status": "known" });
        s.write_synced("word_status", "id:rumah", OpKind::Upsert, Some(word)).unwrap();
        assert_eq!(s.outbox_len().unwrap(), 1);

        // Same account keeps cursor and outbox.
        s.begin_account("acc-a", false).unwrap();
        assert_eq!(s.last_seq().unwrap(), 42);
        assert_eq!(s.outbox_len().unwrap(), 1);

        // Another account resets cursor to 0 and holds A's unsent op.
        s.begin_account("acc-b", false).unwrap();
        assert_eq!(s.last_seq().unwrap(), 0);
        assert_eq!(s.outbox_len().unwrap(), 0);
        assert_eq!(s.held_len().unwrap(), 1);
    }

    #[test]
    fn borrowed_session_wipes_study_on_sign_out_and_next_boot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("borrowed.db");
        let mut s = Store::open(&path).unwrap();
        let dev = s.device_id().clone();
        s.begin_account("acc-a", true).unwrap();
        let word = serde_json::json!({ "lexeme": "buku", "lang": "id", "status": "learning" });
        s.write_synced("word_status", "id:buku", OpKind::Upsert, Some(word)).unwrap();
        s.ai_cache_put("personal-1", r#"{"ok":true}"#).unwrap();
        assert!(s.is_borrowed());
        drop(s);

        // App closed without explicit sign-out: next launch wipes borrowed study data, keeps device_id.
        let mut reopened = Store::open(&path).unwrap();
        assert!(!reopened.is_borrowed());
        assert_eq!(reopened.device_id(), &dev);
        assert!(reopened.synced_rows("word_status").unwrap().is_empty());
        assert_eq!(reopened.ai_cache_get("personal-1").unwrap(), None);

        // Explicit sign-out wipe also clears everything in place.
        reopened.begin_account("acc-a", true).unwrap();
        let word = serde_json::json!({ "lexeme": "air", "lang": "id", "status": "known" });
        reopened.write_synced("word_status", "id:air", OpKind::Upsert, Some(word)).unwrap();
        let changed = reopened.forget_study().unwrap();
        assert!(changed.contains("settings") && changed.contains("word_status"));
        assert!(!reopened.is_borrowed());
        assert!(reopened.synced_rows("word_status").unwrap().is_empty());
        assert_eq!(reopened.outbox_len().unwrap(), 0);
    }
}
