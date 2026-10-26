//! Local SQLite store. Every synced table carries account, device, HLC and a
//! tombstone flag so M2 sync can replay rows without schema changes.

use std::path::Path;

use kabar_core::clock::HlcClock;
use kabar_core::settings::Settings;
use kabar_protocol::learn::TaughtEntry;
use kabar_protocol::pen::PenLabReport;
use kabar_protocol::{DeviceId, Hlc};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database is from a newer Kabar (schema {found}, this build knows {known})")]
    TooNew { found: u32, known: u32 },
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
    // 2: Teach Kabar's personal dictionary (spec §26), LWW per `{lang}:{word}`.
    "CREATE TABLE taught_entries (
        id          TEXT PRIMARY KEY,
        entry       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;",
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
                    SELECT hlc_wall, hlc_counter FROM taught_entries WHERE device_id = ?1
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
            let hlc = self.clock.tick(now_ms());
            tx.execute(
                "INSERT INTO settings (key, value, device_id, hlc_wall, hlc_counter, deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0)
                 ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value, device_id = excluded.device_id,
                    hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter, deleted = 0",
                params![key, value, self.device.0, hlc.wall_ms as i64, hlc.counter],
            )?;
        }
        tx.commit()?;
        Ok(())
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

    /// Discards a draft or forgets an entry: a tombstone, so the delete syncs.
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

    #[test]
    fn taught_entries_replace_and_tombstone() {
        let mut store = Store::open_in_memory().unwrap();
        let entry: TaughtEntry =
            serde_json::from_str(include_str!("../../../fixtures/learn/taught.kalimat.json")).unwrap();
        let a = store.save_taught(&entry).unwrap();
        let mut approved = entry.clone();
        approved.entry.status = kabar_protocol::content::ReviewStatus::Approved;
        let b = store.save_taught(&approved).unwrap();
        assert!(b > a);
        assert_eq!(store.taught_entries().unwrap(), [approved]);
        store.delete_taught(&entry.row_id()).unwrap();
        assert!(store.taught_entries().unwrap().is_empty());
        store.save_taught(&entry).unwrap();
        assert_eq!(store.taught_entries().unwrap(), [entry]);
    }

    #[test]
    fn refuses_a_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(migrate(&mut conn), Err(StoreError::TooNew { found: 99, .. })));
    }
}
