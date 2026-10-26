//! Local SQLite store. Every synced table carries account, device, HLC and a
//! tombstone flag so sync can replay rows without schema changes.
//!
//! Sync: every write to a synced table goes through [`Store::write_synced`],
//! which records the row in `sync_rows` (the merge state, one row per
//! `(table, row_id)`), queues an op in `outbox` and updates the typed table,
//! all in one transaction. Remote ops come in through [`Store::apply_remote`]
//! and merge with the same rules as the hub (`kabar_core::sync`). Tables with
//! no typed table yet live only in `sync_rows`, so nothing from another device
//! is dropped by an older build.

use std::collections::BTreeSet;
use std::path::Path;

use kabar_core::clock::HlcClock;
use kabar_core::settings::Settings;
use kabar_core::sync::{Merge, RowMeta, merge, validate};
use kabar_protocol::pen::PenLabReport;
use kabar_protocol::sync::{MAX_PAYLOAD_BYTES, Op, OpKind, RejectReason, SeqOp};
use kabar_protocol::{DeviceId, Hlc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database is from a newer Kabar (schema {found}, this build knows {known})")]
    TooNew { found: u32, known: u32 },
    #[error("not a valid sync write: {0:?}")]
    Invalid(RejectReason),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Numbered migrations; the index + 1 is the schema version.
const MIGRATIONS: &[&str] = &[
    // 1: device identity, settings, Pen Lab runs.
    "CREATE TABLE meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) STRICT;
    CREATE TABLE settings (
        key         TEXT PRIMARY KEY,
        value       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE pen_lab_runs (
        id          TEXT PRIMARY KEY,
        report      TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;",
    // 2: sync. `sync_rows` is the merge state of every synced row (and the
    // only copy of tables this build doesn't materialize); `outbox` holds ops
    // not yet acked by Kabar Cloud. Settings written before sync are queued.
    "CREATE TABLE sync_rows (
        tbl         TEXT NOT NULL,
        row_id      TEXT NOT NULL,
        payload     TEXT,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (tbl, row_id)
    ) STRICT;
    CREATE TABLE outbox (
        seq   INTEGER PRIMARY KEY AUTOINCREMENT,
        op_id TEXT NOT NULL UNIQUE,
        op    TEXT NOT NULL
    ) STRICT;
    INSERT INTO sync_rows (tbl, row_id, payload, device_id, hlc_wall, hlc_counter, deleted)
        SELECT 'settings', key, json_object('value', json(value)), device_id, hlc_wall, hlc_counter, deleted
        FROM settings;
    INSERT INTO outbox (op_id, op)
        SELECT 'm2-' || device_id || '-' || row_id, json_object(
            'id', 'm2-' || device_id || '-' || row_id, 'table', tbl, 'row_id', row_id, 'kind', 'upsert',
            'hlc', json_object('wall_ms', hlc_wall, 'counter', hlc_counter, 'device', device_id),
            'payload', json(payload))
        FROM sync_rows WHERE tbl = 'settings' AND deleted = 0;",
];

pub struct Store {
    conn: Connection,
    device: DeviceId,
    clock: HlcClock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PenLabRun {
    pub id: String,
    pub hlc: Hlc,
    pub report: PenLabReport,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // The UI and the sync task each hold a connection.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&mut conn)?;

        let device = match conn
            .query_row("SELECT value FROM meta WHERE key = 'device_id'", [], |r| r.get::<_, String>(0))
            .optional()?
        {
            Some(id) => DeviceId(id),
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute("INSERT INTO meta (key, value) VALUES ('device_id', ?1)", [&id])?;
                DeviceId(id)
            }
        };

        // Never issue a timestamp at or below the newest one already written here.
        let last = conn
            .query_row(
                "SELECT hlc_wall, hlc_counter FROM (
                    SELECT hlc_wall, hlc_counter FROM settings WHERE device_id = ?1
                    UNION ALL
                    SELECT hlc_wall, hlc_counter FROM pen_lab_runs WHERE device_id = ?1
                    UNION ALL
                    SELECT hlc_wall, hlc_counter FROM sync_rows WHERE device_id = ?1
                 ) ORDER BY hlc_wall DESC, hlc_counter DESC LIMIT 1",
                [&device.0],
                |r| {
                    Ok(Hlc {
                        wall_ms: r.get::<_, i64>(0)? as u64,
                        counter: r.get(1)?,
                        device: device.clone(),
                    })
                },
            )
            .optional()?;
        let clock = match &last {
            Some(h) => HlcClock::resume(device.clone(), h),
            None => HlcClock::new(device.clone()),
        };

        Ok(Self { conn, device, clock })
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device
    }

    pub fn settings(&self) -> Result<Settings> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings WHERE deleted = 0")?;
        let rows: Vec<(String, String)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(Settings::from_pairs(rows.iter().map(|(k, v)| (k.as_str(), v.as_str()))))
    }

    /// Writes only the keys that changed, each with a fresh HLC.
    pub fn save_settings(&mut self, old: &Settings, new: &Settings) -> Result<()> {
        let tx = self.conn.transaction()?;
        for (key, value) in old.changed_pairs(new) {
            let payload = serde_json::json!({ "value": serde_json::from_str::<Value>(&value)? });
            write_synced(&tx, &mut self.clock, "settings", key, OpKind::Upsert, Some(payload))?;
        }
        tx.commit()?;
        Ok(())
    }

    /// One local write to a synced table: merge state, outbox and typed table
    /// in one transaction. Returns the op that will be pushed.
    pub fn write_synced(
        &mut self,
        table: &str,
        row_id: &str,
        kind: OpKind,
        payload: Option<Value>,
    ) -> Result<Op> {
        let tx = self.conn.transaction()?;
        let op = write_synced(&tx, &mut self.clock, table, row_id, kind, payload)?;
        tx.commit()?;
        Ok(op)
    }

    /// Merges ops from Kabar Cloud and saves `last_seq`, in one transaction.
    /// Returns the tables that changed, so the UI can reload them.
    pub fn apply_remote(&mut self, ops: &[SeqOp], last_seq: u64) -> Result<BTreeSet<String>> {
        let tx = self.conn.transaction()?;
        let mut changed = BTreeSet::new();
        let now = now_ms();
        for SeqOp { op, .. } in ops {
            // The hub already validated it; an older build may not know the
            // table's rules yet, and then it is skipped, never half-applied.
            let Ok(sync) = validate(op, MAX_PAYLOAD_BYTES) else { continue };
            self.clock.observe(&op.hlc, now);
            if merge(sync, op, row_meta(&tx, &op.table, &op.row_id)?.as_ref()) == Merge::Apply {
                store_row(&tx, op)?;
                materialize(&tx, op)?;
                changed.insert(op.table.clone());
            }
        }
        set_meta(&tx, "last_seq", &last_seq.to_string())?;
        tx.commit()?;
        Ok(changed)
    }

    pub fn last_seq(&self) -> Result<u64> {
        Ok(meta(&self.conn, "last_seq")?.and_then(|v| v.parse().ok()).unwrap_or(0))
    }

    pub fn set_last_seq(&self, last_seq: u64) -> Result<()> {
        set_meta(&self.conn, "last_seq", &last_seq.to_string())
    }

    /// The oldest `limit` ops waiting to be pushed.
    pub fn outbox(&self, limit: usize) -> Result<Vec<Op>> {
        let mut stmt = self.conn.prepare("SELECT op FROM outbox ORDER BY seq LIMIT ?1")?;
        let rows: Vec<String> =
            stmt.query_map([limit as i64], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        rows.iter().map(|op| Ok(serde_json::from_str(op)?)).collect()
    }

    pub fn outbox_len(&self) -> Result<u64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get::<_, i64>(0))? as u64)
    }

    pub fn remove_from_outbox<'a>(&mut self, op_ids: impl IntoIterator<Item = &'a str>) -> Result<()> {
        let tx = self.conn.transaction()?;
        for id in op_ids {
            tx.execute("DELETE FROM outbox WHERE op_id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Live rows of a synced table, newest first: `(row_id, payload)`.
    pub fn synced_rows(&self, table: &str) -> Result<Vec<(String, Value)>> {
        let mut stmt = self.conn.prepare(
            "SELECT row_id, payload FROM sync_rows WHERE tbl = ?1 AND deleted = 0 AND payload IS NOT NULL
             ORDER BY hlc_wall DESC, hlc_counter DESC",
        )?;
        let rows: Vec<(String, String)> =
            stmt.query_map([table], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        rows.into_iter().map(|(id, p)| Ok((id, serde_json::from_str(&p)?))).collect()
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        meta(&self.conn, key)
    }

    pub fn set_meta(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => set_meta(&self.conn, key, v),
            None => {
                self.conn.execute("DELETE FROM meta WHERE key = ?1", [key])?;
                Ok(())
            }
        }
    }

    pub fn save_pen_lab_run(&mut self, report: &PenLabReport) -> Result<PenLabRun> {
        let hlc = self.clock.tick(now_ms());
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO pen_lab_runs (id, report, device_id, hlc_wall, hlc_counter)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, serde_json::to_string(report)?, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(PenLabRun { id, hlc, report: report.clone() })
    }

    pub fn pen_lab_runs(&self) -> Result<Vec<PenLabRun>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, report, device_id, hlc_wall, hlc_counter FROM pen_lab_runs
             WHERE deleted = 0 ORDER BY hlc_wall DESC, hlc_counter DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                Hlc { device: DeviceId(r.get(2)?), wall_ms: r.get::<_, i64>(3)? as u64, counter: r.get(4)? },
            ))
        })?;
        rows.map(|row| {
            let (id, report, hlc) = row?;
            Ok(PenLabRun { id, hlc, report: serde_json::from_str(&report)? })
        })
        .collect()
    }
}

fn write_synced(
    tx: &Transaction<'_>,
    clock: &mut HlcClock,
    table: &str,
    row_id: &str,
    kind: OpKind,
    payload: Option<Value>,
) -> Result<Op> {
    // Newer than whatever this row holds, even if it came from a device whose
    // clock runs ahead, so a local edit always wins over what it replaces.
    let existing = row_meta(tx, table, row_id)?;
    let hlc = match &existing {
        Some(m) => clock.observe(&m.hlc, now_ms()),
        None => clock.tick(now_ms()),
    };
    let op = Op {
        id: uuid::Uuid::now_v7().to_string(),
        table: table.into(),
        row_id: row_id.into(),
        kind,
        hlc,
        payload,
    };
    let sync = validate(&op, MAX_PAYLOAD_BYTES).map_err(StoreError::Invalid)?;
    if merge(sync, &op, existing.as_ref()) == Merge::Ignore {
        // Appending a row that exists, or re-deleting a stroke.
        return Ok(op);
    }
    store_row(tx, &op)?;
    tx.execute(
        "INSERT INTO outbox (op_id, op) VALUES (?1, ?2)",
        params![op.id, serde_json::to_string(&op)?],
    )?;
    materialize(tx, &op)?;
    Ok(op)
}

fn row_meta(conn: &Connection, table: &str, row_id: &str) -> Result<Option<RowMeta>> {
    Ok(conn
        .query_row(
            "SELECT device_id, hlc_wall, hlc_counter, deleted FROM sync_rows WHERE tbl = ?1 AND row_id = ?2",
            [table, row_id],
            |r| {
                Ok(RowMeta {
                    hlc: Hlc {
                        device: DeviceId(r.get(0)?),
                        wall_ms: r.get::<_, i64>(1)? as u64,
                        counter: r.get(2)?,
                    },
                    deleted: r.get::<_, i64>(3)? != 0,
                })
            },
        )
        .optional()?)
}

fn store_row(tx: &Transaction<'_>, op: &Op) -> Result<()> {
    let deleted = op.kind == OpKind::Delete;
    // A tombstone keeps the last payload of an immutable row (strokes are
    // erased, not lost); LWW rows drop it.
    let payload = op.payload.as_ref().map(serde_json::to_string).transpose()?;
    tx.execute(
        "INSERT INTO sync_rows (tbl, row_id, payload, device_id, hlc_wall, hlc_counter, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(tbl, row_id) DO UPDATE SET
            payload = CASE WHEN excluded.deleted = 1 AND excluded.payload IS NULL
                           THEN CASE WHEN ?8 THEN payload ELSE NULL END
                           ELSE excluded.payload END,
            device_id = excluded.device_id, hlc_wall = excluded.hlc_wall,
            hlc_counter = excluded.hlc_counter, deleted = excluded.deleted",
        params![
            op.table,
            op.row_id,
            payload,
            op.hlc.device.0,
            op.hlc.wall_ms as i64,
            op.hlc.counter,
            deleted as i64,
            kabar_protocol::sync::table_kind(&op.table) == Some(kabar_protocol::sync::SyncKind::Immutable),
        ],
    )?;
    Ok(())
}

/// Mirrors a stored op into its typed table, for the tables this build reads.
fn materialize(tx: &Transaction<'_>, op: &Op) -> Result<()> {
    if op.table == "settings" {
        let value = match (&op.kind, op.payload.as_ref().and_then(|p| p.get("value"))) {
            (OpKind::Delete, _) | (_, None) => None,
            (_, Some(v)) => Some(serde_json::to_string(v)?),
        };
        tx.execute(
            "INSERT INTO settings (key, value, device_id, hlc_wall, hlc_counter, deleted)
             VALUES (?1, COALESCE(?2, 'null'), ?3, ?4, ?5, ?6)
             ON CONFLICT(key) DO UPDATE SET
                value = COALESCE(?2, value), device_id = excluded.device_id,
                hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter, deleted = excluded.deleted",
            params![
                op.row_id,
                value,
                op.hlc.device.0,
                op.hlc.wall_ms as i64,
                op.hlc.counter,
                value.is_none() as i64
            ],
        )?;
    }
    Ok(())
}

fn meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let known = MIGRATIONS.len() as u32;
    let current: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > known {
        return Err(StoreError::TooNew { found: current, known });
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use kabar_core::settings::Theme;

    use super::*;

    #[test]
    fn device_id_and_settings_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");

        let mut store = Store::open(&path).unwrap();
        let device = store.device_id().clone();
        let old = store.settings().unwrap();
        let new = Settings { theme: Theme::Dark, welcomed: true, ..old.clone() };
        store.save_settings(&old, &new).unwrap();
        drop(store);

        let store = Store::open(&path).unwrap();
        assert_eq!(store.device_id(), &device);
        assert_eq!(store.settings().unwrap(), new);
    }

    #[test]
    fn pen_lab_runs_newest_first() {
        let mut store = Store::open_in_memory().unwrap();
        let a =
            store.save_pen_lab_run(&PenLabReport { device_model: Some("a".into()), ..Default::default() });
        let b =
            store.save_pen_lab_run(&PenLabReport { device_model: Some("b".into()), ..Default::default() });
        assert!(b.unwrap().hlc > a.unwrap().hlc);
        let runs = store.pen_lab_runs().unwrap();
        assert_eq!(runs[0].report.device_model.as_deref(), Some("b"));
        assert_eq!(runs.len(), 2);
    }

    /// What the hub would send: the ops in `from`'s outbox, numbered.
    fn relay(from: &Store, first_seq: u64) -> Vec<SeqOp> {
        from.outbox(100)
            .unwrap()
            .into_iter()
            .enumerate()
            .map(|(i, op)| SeqOp { seq: first_seq + i as u64, op })
            .collect()
    }

    fn word(status: &str) -> Option<Value> {
        Some(serde_json::json!({ "lexeme": "rumah", "lang": "id", "status": status }))
    }

    #[test]
    fn a_word_saved_on_the_phone_reaches_the_tablet() {
        let mut phone = Store::open_in_memory().unwrap();
        let mut tablet = Store::open_in_memory().unwrap();
        phone.write_synced("word_status", "id:rumah", OpKind::Upsert, word("learning")).unwrap();
        let ops = relay(&phone, 1);
        assert_eq!(ops.len(), 1);

        let changed = tablet.apply_remote(&ops, 1).unwrap();
        assert!(changed.contains("word_status"));
        assert_eq!(tablet.synced_rows("word_status").unwrap()[0].1["status"], "learning");
        assert_eq!(tablet.last_seq().unwrap(), 1);
        // Remote ops never go back out.
        assert_eq!(tablet.outbox_len().unwrap(), 0);
        // Replays change nothing.
        assert!(tablet.apply_remote(&ops, 1).unwrap().is_empty());
    }

    #[test]
    fn a_local_edit_beats_the_remote_row_it_replaces() {
        let mut phone = Store::open_in_memory().unwrap();
        let mut tablet = Store::open_in_memory().unwrap();
        // The phone's clock runs two minutes ahead.
        let mut op = phone.write_synced("word_status", "id:rumah", OpKind::Upsert, word("learning")).unwrap();
        op.hlc.wall_ms += 120_000;
        tablet.apply_remote(&[SeqOp { seq: 1, op: op.clone() }], 1).unwrap();
        let edit = tablet.write_synced("word_status", "id:rumah", OpKind::Upsert, word("known")).unwrap();
        assert!(edit.hlc > op.hlc);
        // And the phone, receiving the older op later, keeps the newer one.
        let mut late = op.clone();
        late.hlc.wall_ms -= 1;
        late.id = "late".into();
        assert!(tablet.apply_remote(&[SeqOp { seq: 2, op: late }], 2).unwrap().is_empty());
        assert_eq!(tablet.synced_rows("word_status").unwrap()[0].1["status"], "known");
    }

    #[test]
    fn remote_settings_update_the_settings_table() {
        let mut phone = Store::open_in_memory().unwrap();
        let mut tablet = Store::open_in_memory().unwrap();
        let old = phone.settings().unwrap();
        let new = Settings { theme: Theme::Dark, ..old.clone() };
        phone.save_settings(&old, &new).unwrap();
        tablet.apply_remote(&relay(&phone, 1), 1).unwrap();
        assert_eq!(tablet.settings().unwrap().theme, Theme::Dark);
    }

    #[test]
    fn acked_ops_leave_the_outbox_in_order() {
        let mut store = Store::open_in_memory().unwrap();
        let a = store.write_synced("word_status", "id:a", OpKind::Upsert, word("new")).unwrap();
        let b = store.write_synced("word_status", "id:b", OpKind::Upsert, word("new")).unwrap();
        let ids: Vec<String> = store.outbox(10).unwrap().into_iter().map(|o| o.id).collect();
        assert_eq!(ids, [a.id.clone(), b.id.clone()]);
        store.remove_from_outbox([a.id.as_str()]).unwrap();
        assert_eq!(store.outbox(10).unwrap()[0].id, b.id);
    }

    #[test]
    fn invalid_writes_are_refused() {
        let mut store = Store::open_in_memory().unwrap();
        let err = store.write_synced("nope", "x", OpKind::Upsert, word("new")).unwrap_err();
        assert!(matches!(err, StoreError::Invalid(RejectReason::UnknownTable)));
        assert_eq!(store.outbox_len().unwrap(), 0);
    }

    #[test]
    fn unknown_tables_from_newer_builds_are_kept() {
        let mut phone = Store::open_in_memory().unwrap();
        let mut tablet = Store::open_in_memory().unwrap();
        let payload = Some(serde_json::json!({ "title": "Sek tak lapor pusat" }));
        phone.write_synced("texts", "t1", OpKind::Upsert, payload).unwrap();
        tablet.apply_remote(&relay(&phone, 1), 1).unwrap();
        assert_eq!(tablet.synced_rows("texts").unwrap()[0].0, "t1");
    }

    #[test]
    fn settings_from_before_sync_are_queued() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        {
            let mut conn = Connection::open(&path).unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute_batch(MIGRATIONS[0]).unwrap();
            tx.execute(
                "INSERT INTO settings (key, value, device_id, hlc_wall, hlc_counter) VALUES ('theme', '\"dark\"', 'mac', 5, 0)",
                [],
            )
            .unwrap();
            tx.pragma_update(None, "user_version", 1).unwrap();
            tx.commit().unwrap();
        }
        let store = Store::open(&path).unwrap();
        let ops = store.outbox(10).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].id, "m2-mac-theme");
        assert_eq!(ops[0].payload, Some(serde_json::json!({ "value": "dark" })));
        assert_eq!(ops[0].hlc, Hlc { wall_ms: 5, counter: 0, device: DeviceId("mac".into()) });
        assert_eq!(store.settings().unwrap().theme, Theme::Dark);
    }

    #[test]
    fn refuses_a_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(migrate(&mut conn), Err(StoreError::TooNew { found: 99, .. })));
    }
}
