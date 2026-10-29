//! Spots a database that was copied onto another device: Android Auto
//! Backup, Smart Switch or a device-to-device transfer bring `kabar.db`
//! along (`kabar_data_extraction_rules.xml`), and with it the old phone's
//! `device_id`. Two installs with one id would stamp the same HLCs and look
//! like one device to Kabar Cloud.
//!
//! The device id is also written to a marker in a folder Android never backs
//! up or transfers (`no_backup`). A database that says it was marked but has
//! no matching marker beside it came from somewhere else, so it takes a new
//! device id and reads the whole log again. Its synced rows keep the old id:
//! they were written on the other device. What the other device never got to
//! push (its outbox) is stamped again under the new id, because Kabar Cloud
//! only takes a device's own writes, and after a cloud restore the old phone
//! may be gone.
// adapted from batna-app main: crates/app/src/store/install.rs

use std::path::Path;

use kabar_core::clock::HlcClock;
use kabar_protocol::DeviceId;
use kabar_protocol::sync::Op;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{Result, Typed, materialize, now_ms, row_meta, store_row};

const MARKED: &str = "device.marked";

/// The device id to use: `device`, or a fresh one when this database was
/// copied from another install.
pub(super) fn check(conn: &mut Connection, marker: &Path, device: DeviceId) -> Result<DeviceId> {
    let marked: Option<String> =
        conn.query_row("SELECT value FROM meta WHERE key = ?1", [MARKED], |r| r.get(0)).optional()?;
    let on_disk = std::fs::read_to_string(marker).ok().map(|s| s.trim().to_string());
    let device = match (on_disk.as_deref(), marked.is_some()) {
        (Some(id), _) if id == device.0 => return Ok(device),
        // First run with markers (a new install, or one from before them).
        (None, false) => device,
        // Marked elsewhere: a copy.
        _ => {
            let fresh = DeviceId(uuid::Uuid::new_v4().to_string());
            crate::diag::report!("database copied from device {}; this install is now {}", device.0, fresh.0);
            let tx = conn.transaction()?;
            tx.execute("UPDATE meta SET value = ?1 WHERE key = 'device_id'", [&fresh.0])?;
            // Ask for the whole log again under the new name.
            tx.execute("DELETE FROM meta WHERE key IN ('last_seq', 'last_sync_ms')", [])?;
            restamp_outbox(&tx, &fresh)?;
            tx.commit()?;
            fresh
        }
    };
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(marker, &device.0) {
        // Without a marker the next start treats this as a copy again, so
        // leave the database unmarked and try next time.
        crate::diag::report!("install marker {}: {e}", marker.display());
        return Ok(device);
    }
    conn.execute("INSERT OR REPLACE INTO meta (key, value) VALUES (?1, '1')", params![MARKED])?;
    Ok(device)
}

/// Re-issues every queued op that is still its row's newest version under
/// `fresh`, with a stamp after the old one. Superseded ops are dropped: the
/// op that replaced them is queued too, or came from Kabar Cloud.
fn restamp_outbox(tx: &Transaction<'_>, fresh: &DeviceId) -> Result<()> {
    let queued: Vec<String> = {
        let mut stmt = tx.prepare("SELECT op FROM outbox ORDER BY seq")?;
        stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    tx.execute("DELETE FROM outbox", [])?;
    let mut clock = HlcClock::new(fresh.clone());
    for json in queued {
        let Ok(old) = serde_json::from_str::<Op>(&json) else { continue };
        if row_meta(tx, &old.table, &old.row_id)?.is_none_or(|m| m.hlc != old.hlc) {
            continue;
        }
        let op = Op { id: uuid::Uuid::now_v7().to_string(), hlc: clock.observe(&old.hlc, now_ms()), ..old };
        store_row(tx, &op)?;
        materialize(tx, &op)?;
        // Event tables keep the first copy of a row (INSERT OR IGNORE).
        if let Some(table) = Typed::of(&op.table).and_then(Typed::event_table) {
            tx.execute(
                &format!("UPDATE {table} SET device_id = ?2, hlc_wall = ?3, hlc_counter = ?4 WHERE id = ?1"),
                params![op.row_id, op.hlc.device.0, op.hlc.wall_ms as i64, op.hlc.counter],
            )?;
        }
        tx.execute(
            "INSERT INTO outbox (op_id, op) VALUES (?1, ?2)",
            params![op.id, serde_json::to_string(&op)?],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kabar_protocol::coach::ActivityKind;

    use super::super::Store;

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!("kabar-install-{}", uuid::Uuid::new_v4()))
    }

    fn xp(store: &mut Store) -> String {
        let day = "2026-10-29".parse().unwrap();
        store.add_activity(ActivityKind::Reviews { cards: 10, correct: 8 }, day, 300).unwrap().id
    }

    #[test]
    fn the_same_install_keeps_its_device_id() {
        let phone = dir();
        let (db, marker) = (phone.join("files/kabar.db"), phone.join("no_backup/kabar-install"));
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let first = Store::open_marked(&db, Some(&marker)).unwrap().device_id().clone();
        let again = Store::open_marked(&db, Some(&marker)).unwrap().device_id().clone();
        assert_eq!(first, again);
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), first.0);
        let _ = std::fs::remove_dir_all(phone);
    }

    #[test]
    fn a_database_from_before_markers_keeps_its_id_and_gets_marked() {
        let phone = dir();
        let (db, marker) = (phone.join("files/kabar.db"), phone.join("no_backup/kabar-install"));
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let old = Store::open(&db).unwrap().device_id().clone();
        let now = Store::open_marked(&db, Some(&marker)).unwrap().device_id().clone();
        assert_eq!(old, now);
        assert!(marker.exists());
        let _ = std::fs::remove_dir_all(phone);
    }

    #[test]
    fn a_copied_database_takes_a_new_device_id_and_pushes_what_was_left() {
        let phone = dir();
        let (db, marker) = (phone.join("files/kabar.db"), phone.join("no_backup/kabar-install"));
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let old = {
            let mut s = Store::open_marked(&db, Some(&marker)).unwrap();
            let synced = xp(&mut s);
            let sent: Vec<String> = s.outbox(10).unwrap().into_iter().map(|o| o.id).collect();
            s.remove_from_outbox(sent.iter().map(String::as_str)).unwrap();
            s.set_last_seq(42).unwrap();
            let unsent = xp(&mut s);
            (s.device_id().clone(), synced, unsent)
        };
        let (old, synced, unsent) = old;

        // A backup restored on the new phone: the database, not the marker.
        let tablet = dir();
        let (copy, tablet_marker) = (tablet.join("files/kabar.db"), tablet.join("no_backup/kabar-install"));
        std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
        std::fs::copy(&db, &copy).unwrap();
        let mut s = Store::open_marked(&copy, Some(&tablet_marker)).unwrap();
        let new = s.device_id().clone();
        assert_ne!(new, old);
        assert_eq!(s.last_seq().unwrap(), 0, "reads the whole log again");

        // The unsent event goes out as this device's own write; the synced one
        // stays the old phone's.
        let outbox = s.outbox(10).unwrap();
        assert_eq!(outbox.len(), 1);
        assert_eq!(outbox[0].row_id, unsent);
        assert_eq!(outbox[0].hlc.device, new);
        let owner = |id: &str| -> String {
            s.conn.query_row("SELECT device_id FROM xp_events WHERE id = ?1", [id], |r| r.get(0)).unwrap()
        };
        assert_eq!(owner(&synced), old.0);
        assert_eq!(owner(&unsent), new.0);

        // A new write sorts after everything, and the next start is not a copy.
        xp(&mut s);
        let stamps: Vec<_> = s.outbox(10).unwrap().into_iter().map(|o| o.hlc).collect();
        assert!(stamps[0] < stamps[1]);
        drop(s);
        assert_eq!(Store::open_marked(&copy, Some(&tablet_marker)).unwrap().device_id(), &new);

        // The old phone, still in use, keeps its id.
        assert_eq!(Store::open_marked(&db, Some(&marker)).unwrap().device_id(), &old);
        let _ = std::fs::remove_dir_all(phone);
        let _ = std::fs::remove_dir_all(tablet);
    }
}
