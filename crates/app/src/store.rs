//! Local SQLite store. Every synced table carries account, device, HLC and a
//! tombstone flag so sync can replay rows without schema changes.
//!
//! Sync: every write to a synced table goes through [`Store::write_synced`]
//! (or `append_event` for events that carry their own HLC), which records
//! the row in `sync_rows` (the merge state, one row per `(table, row_id)`),
//! queues an op in `outbox` and updates the typed table, all in one
//! transaction. Remote ops come in through [`Store::apply_remote`] and merge
//! with the same rules as the hub (`kabar_core::sync`). Tables with no typed
//! table yet live only in `sync_rows`, so nothing from another device is
//! dropped by an older build. A new typed table needs a `Typed` entry and a
//! line in `backfill_sync`.

mod preferences;

mod sync_rows;
use sync_rows::*;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kabar_core::clock::HlcClock;
use kabar_core::settings::Settings;
use kabar_core::sync::{Merge, RowMeta, merge, validate};
use kabar_protocol::coach::{
    ActivityEvent, ActivityKind, CoachProfile, DailyPlan, LocalDate, PROFILE_ROW_ID, REMINDER_ROW_ID,
    ReminderState, WeeklyReport,
};
use kabar_protocol::lesson::{LessonCompleted, LessonPosition, LessonVerified, Verification};
use kabar_protocol::pen::PenLabReport;
use kabar_protocol::sync::{MAX_PAYLOAD_BYTES, Op, OpKind, RejectReason, SeqOp, SyncKind, table_kind};
use kabar_protocol::{DeviceId, Hlc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
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
    // 2: Kak Nur, the coach (M7). Table names match Kabar Cloud's sync tables.
    "CREATE TABLE goals (
        id          TEXT PRIMARY KEY,
        value       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE daily_plans (
        id          TEXT PRIMARY KEY,
        value       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE xp_events (
        id          TEXT PRIMARY KEY,
        date        TEXT NOT NULL,
        event       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX xp_events_date ON xp_events (date);
    CREATE TABLE reminders (
        id          TEXT PRIMARY KEY,
        value       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE reports (
        id          TEXT PRIMARY KEY,
        value       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;",
    // 3: learning (M6/M8). Lesson and review events are append-only; cards and the
    // Lanjut position are last-writer-wins by HLC. Payloads are protocol JSON.
    "CREATE TABLE lesson_events (
        id          TEXT PRIMARY KEY,
        lesson_id   TEXT NOT NULL,
        kind        TEXT NOT NULL,
        payload     TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX lesson_events_lesson ON lesson_events (lesson_id);
    CREATE TABLE sessions (
        device      TEXT PRIMARY KEY,
        payload     TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE cards (
        id          TEXT PRIMARY KEY,
        payload     TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE review_log (
        id          TEXT PRIMARY KEY,
        card_id     TEXT NOT NULL,
        payload     TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX review_log_card ON review_log (card_id);",
    // 4: Teach Kabar's personal dictionary (spec §26), LWW per `{lang}:{word}`.
    "CREATE TABLE taught_entries (
        id          TEXT PRIMARY KEY,
        entry       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;",
    // 5: sync. `sync_rows` is the merge state of every synced row (and the
    // only copy of tables this build doesn't materialize); `outbox` holds ops
    // not yet acked by Kabar Cloud. Rows written before sync are queued by
    // `backfill_sync`.
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
    ) STRICT;",
    // 6: Reader (M5). Texts, word statuses and highlights sync (one JSON
    // document per row, mirrored from `sync_rows`); quality feedback lives
    // only in `sync_rows` (append-only). Saved words stay on the device until
    // the review deck takes them over.
    "CREATE TABLE texts (
        id          TEXT PRIMARY KEY,
        payload     TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE word_status (
        id          TEXT PRIMARY KEY,
        payload     TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE highlights (
        id          TEXT PRIMARY KEY,
        payload     TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX highlights_text ON highlights (json_extract(payload, '$.text_id'));
    CREATE TABLE saved_words (
        id          TEXT PRIMARY KEY,
        lexeme      TEXT NOT NULL,
        surface     TEXT NOT NULL,
        sentence    TEXT NOT NULL,
        text_id     TEXT,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;",
    // 7: validated AI answers on this device, keyed by `CacheKey::id()`
    // (task, engine, prompt version, input hash). Local only: every device
    // can re-ask, and Kabar Cloud keeps the shared copy in D1.
    "CREATE TABLE ai_cache (
        key        TEXT PRIMARY KEY,
        json       TEXT NOT NULL,
        created_ms INTEGER NOT NULL
    ) STRICT;",
    // 8: learning journal reflections (spec §6), LWW per reflection id. Rows
    // another device sent before this build knew the table are copied over
    // from `sync_rows`, so nothing synced earlier is lost.
    "CREATE TABLE journal_entries (
        id          TEXT PRIMARY KEY,
        entry       TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX journal_entries_date ON journal_entries (json_extract(entry, '$.date'));
    INSERT INTO journal_entries (id, entry, account_id, device_id, hlc_wall, hlc_counter, deleted)
        SELECT row_id, payload, account_id, device_id, hlc_wall, hlc_counter, deleted
        FROM sync_rows WHERE tbl = 'journal_entries' AND payload IS NOT NULL;",
    // 9: Tablet Studio (M9). Ink lives only in `sync_rows`; this finds a
    // page's strokes, items and transcripts without scanning every row.
    "CREATE INDEX sync_rows_doc ON sync_rows (tbl, json_extract(payload, '$.doc'));",
];

/// The schema version that added `sync_rows`.
const SYNC_SCHEMA: u32 = 5;

/// Every synced table; the clock resumes past the newest row in any of them.
const SYNCED_TABLES: &[&str] = &[
    "settings",
    "pen_lab_runs",
    "goals",
    "daily_plans",
    "xp_events",
    "reminders",
    "reports",
    "lesson_events",
    "sessions",
    "cards",
    "review_log",
    "taught_entries",
    "sync_rows",
    "texts",
    "saved_words",
    "journal_entries",
];

/// LWW tables that hold one JSON document per row id.
#[derive(Clone, Copy, Debug)]
enum Doc {
    Goals,
    DailyPlans,
    Reminders,
    Reports,
}

impl Doc {
    const ALL: [Self; 4] = [Self::Goals, Self::DailyPlans, Self::Reminders, Self::Reports];

    fn table(self) -> &'static str {
        match self {
            Self::Goals => "goals",
            Self::DailyPlans => "daily_plans",
            Self::Reminders => "reminders",
            Self::Reports => "reports",
        }
    }

    fn from_table(table: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.table() == table)
    }
}

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
        Self::open_marked(path, None)
    }

    /// Opens the database and, with a `marker` in a folder that is never
    /// backed up or transferred, gives a copy restored from another device
    /// its own device id (`store/install.rs`).
    pub fn open_marked(path: &Path, marker: Option<&Path>) -> Result<Self> {
        Self::init(Connection::open(path)?, marker)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?, None)
    }

    /// Every later write fails, as on a full or locked disk (tests of the
    /// screens' error paths).
    #[cfg(test)]
    pub fn refuse_writes(&self) {
        self.conn.execute_batch("PRAGMA query_only = ON").unwrap();
    }

    #[cfg(test)]
    pub fn allow_writes(&self) {
        self.conn.execute_batch("PRAGMA query_only = OFF").unwrap();
    }

    fn init(mut conn: Connection, marker: Option<&Path>) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // The UI and the sync engine each hold a connection.
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
        let device = match marker {
            Some(m) => install::check(&mut conn, m, device)?,
            None => device,
        };

        // Never issue a timestamp at or below the newest one already written here.
        let union = SYNCED_TABLES
            .iter()
            .map(|t| format!("SELECT hlc_wall, hlc_counter FROM {t} WHERE device_id = ?1"))
            .collect::<Vec<_>>()
            .join(" UNION ALL ");
        let last = conn
            .query_row(
                &format!(
                    "SELECT hlc_wall, hlc_counter FROM ({union})
                     ORDER BY hlc_wall DESC, hlc_counter DESC LIMIT 1"
                ),
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

    /// Changes whenever anything in the database may have changed: rows this
    /// connection wrote, or a commit from another one (the sync engine). For
    /// caching data derived from the store, like the review deck.
    pub fn change_stamp(&self) -> Result<(u64, i64)> {
        let others: i64 = self.conn.query_row("PRAGMA data_version", [], |r| r.get(0))?;
        Ok((self.conn.total_changes(), others))
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device
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

    pub fn ai_cache_get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT json FROM ai_cache WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn ai_cache_put(&self, key: &str, json: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO ai_cache (key, json, created_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET json = excluded.json, created_ms = excluded.created_ms",
            params![key, json, now_ms() as i64],
        )?;
        Ok(())
    }

    pub fn ai_cache_remove(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM ai_cache WHERE key = ?1", [key])?;
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
}

// --- Kak Nur (M7) --------------------------------------------------------------

impl Store {
    fn doc<T: DeserializeOwned>(&self, doc: Doc, id: &str) -> Result<Option<T>> {
        let json = self
            .conn
            .query_row(
                &format!("SELECT value FROM {} WHERE id = ?1 AND deleted = 0", doc.table()),
                [id],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    /// Last-writer-wins upsert with a fresh HLC, synced.
    fn put_doc<T: Serialize>(&mut self, doc: Doc, id: &str, value: &T) -> Result<()> {
        self.write_synced(doc.table(), id, OpKind::Upsert, Some(serde_json::to_value(value)?)).map(drop)
    }

    /// Records an event that carries its own HLC (append-only and immutable
    /// tables), synced. Appending an id that exists is a no-op.
    fn append_event(&mut self, table: &str, row_id: &str, hlc: Hlc, payload: Value) -> Result<()> {
        let op = Op {
            id: uuid::Uuid::now_v7().to_string(),
            table: table.into(),
            row_id: row_id.into(),
            kind: OpKind::Append,
            hlc,
            payload: Some(payload),
        };
        let tx = self.conn.transaction()?;
        record_local(&tx, op)?;
        tx.commit()?;
        Ok(())
    }

    pub fn coach_profile(&self) -> Result<Option<CoachProfile>> {
        self.doc(Doc::Goals, PROFILE_ROW_ID)
    }

    pub fn save_coach_profile(&mut self, profile: &CoachProfile) -> Result<()> {
        self.put_doc(Doc::Goals, PROFILE_ROW_ID, profile)
    }

    pub fn plan_for(&self, date: LocalDate) -> Result<Option<DailyPlan>> {
        self.doc(Doc::DailyPlans, &date.to_string())
    }

    pub fn save_plan(&mut self, plan: &DailyPlan) -> Result<()> {
        self.put_doc(Doc::DailyPlans, &plan.date.to_string(), plan)
    }

    pub fn reminder_state(&self) -> Result<ReminderState> {
        Ok(self.doc(Doc::Reminders, REMINDER_ROW_ID)?.unwrap_or_default())
    }

    pub fn save_reminder_state(&mut self, state: &ReminderState) -> Result<()> {
        self.put_doc(Doc::Reminders, REMINDER_ROW_ID, state)
    }

    pub fn report_for(&self, week_start: LocalDate) -> Result<Option<WeeklyReport>> {
        self.doc(Doc::Reports, &week_start.to_string())
    }

    pub fn save_report(&mut self, report: &WeeklyReport) -> Result<()> {
        self.put_doc(Doc::Reports, &report.week_start.to_string(), report)
    }

    /// The `week_start` of the newest stored report.
    pub fn latest_report_week(&self) -> Result<Option<LocalDate>> {
        let id = self
            .conn
            .query_row("SELECT id FROM reports WHERE deleted = 0 ORDER BY id DESC LIMIT 1", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?;
        Ok(id.and_then(|id| id.parse().ok()))
    }

    /// Appends an event to the activity log; it is never changed afterwards.
    pub fn add_activity(
        &mut self,
        kind: ActivityKind,
        date: LocalDate,
        seconds: u32,
    ) -> Result<ActivityEvent> {
        let at_ms = now_ms();
        let hlc = self.clock.tick(at_ms);
        let event = ActivityEvent { id: uuid::Uuid::new_v4().to_string(), hlc, date, at_ms, kind, seconds };
        self.append_event("xp_events", &event.id, event.hlc.clone(), serde_json::to_value(&event)?)?;
        Ok(event)
    }

    /// Undo for an event (a plan item unticked, a purchase taken back): a
    /// tombstone, so sync removes it on every device.
    pub fn retract_activity(&mut self, id: &str) -> Result<()> {
        self.write_synced("xp_events", id, OpKind::Delete, None).map(drop)
    }

    /// The activity log from `since` on (or all of it), oldest first.
    pub fn activity(&self, since: Option<LocalDate>) -> Result<Vec<ActivityEvent>> {
        let since = since.map(|d| d.to_string()).unwrap_or_default();
        let mut stmt = self.conn.prepare(
            "SELECT event FROM xp_events WHERE deleted = 0 AND date >= ?1
             ORDER BY date, hlc_wall, hlc_counter",
        )?;
        let rows = stmt.query_map([since], |r| r.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
}

// --- Lessons and Lanjut (M8) ------------------------------------------------------

impl Store {
    /// Ids of finished lessons and passed level tests, for the Journey Map.
    pub fn completed_lessons(&self) -> Result<BTreeSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT lesson_id FROM lesson_events WHERE kind = 'completed' AND deleted = 0",
        )?;
        let ids = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// Lessons whose content the learner checked, on any device, and how:
    /// their newest `verified` / `unverified` event says checked, or (kept
    /// from before the two were separated on 2026-10-29) a pass recorded
    /// under the old rule (`kabar_core::lesson::verifies_by_pass`). Their
    /// "not reviewed yet" flag comes off, Javanese included. Finishing a
    /// lesson alone never adds one.
    pub fn verified_lessons(&self) -> Result<BTreeMap<String, Verification>> {
        let mut stmt = self.conn.prepare(
            "SELECT lesson_id, kind, payload FROM lesson_events
             WHERE kind IN ('completed', 'verified', 'unverified') AND deleted = 0
             ORDER BY hlc_wall, hlc_counter, device_id",
        )?;
        let rows: Vec<(String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut checked: BTreeMap<String, bool> = BTreeMap::new();
        let mut passed: BTreeMap<String, u32> = BTreeMap::new();
        for (id, kind, payload) in rows {
            match kind.as_str() {
                "verified" => {
                    checked.insert(id, true);
                }
                "unverified" => {
                    checked.insert(id, false);
                }
                _ => {
                    let Ok(done) = serde_json::from_str::<LessonCompleted>(&payload) else { continue };
                    if kabar_core::lesson::verifies_by_pass(&done) {
                        let best = passed.entry(id).or_default();
                        *best = (*best).max(done.score);
                    }
                }
            }
        }
        let mut out: BTreeMap<String, Verification> =
            passed.into_iter().map(|(id, score)| (id, Verification::PassedEarlier { score })).collect();
        out.extend(checked.into_iter().filter(|(_, on)| *on).map(|(id, _)| (id, Verification::Checked)));
        Ok(out)
    }

    /// Ids of the lessons in [`Store::verified_lessons`].
    pub fn approved_lessons(&self) -> Result<BTreeSet<String>> {
        Ok(self.verified_lessons()?.into_keys().collect())
    }

    /// The learner checked a lesson's content: a synced `verified` event,
    /// and its review cards lose the unverified flag. Checking a lesson
    /// that is already checked adds nothing.
    pub fn verify_lesson(&mut self, lesson_id: &str) -> Result<()> {
        if self.verified_lessons()?.get(lesson_id) != Some(&Verification::Checked) {
            self.record_lesson_event(lesson_id, "verified", &LessonVerified::default())?;
        }
        self.set_lesson_cards_verified(lesson_id, true)
    }

    /// Takes the learner's check back. `lesson_events` is append-only, so
    /// this is a correction: an `unverified` event that outranks the check
    /// by HLC on every device. The cards go back to unverified unless an
    /// old-rule pass still verifies the lesson.
    pub fn unverify_lesson(&mut self, lesson_id: &str) -> Result<()> {
        if self.verified_lessons()?.get(lesson_id) == Some(&Verification::Checked) {
            self.record_lesson_event(lesson_id, "unverified", &LessonVerified::default())?;
        }
        let still = self.verified_lessons()?.contains_key(lesson_id);
        self.set_lesson_cards_verified(lesson_id, still)
    }

    /// Appends a lesson event (`completed`, ...). Append-only: never updated.
    pub fn record_lesson_event(
        &mut self,
        lesson_id: &str,
        kind: &str,
        payload: &impl serde::Serialize,
    ) -> Result<LessonEvent> {
        let hlc = self.clock.tick(now_ms());
        let id = uuid::Uuid::new_v4().to_string();
        let value = serde_json::to_value(payload)?;
        let row = serde_json::json!({ "lesson_id": lesson_id, "kind": kind, "payload": value });
        self.append_event("lesson_events", &id, hlc.clone(), row)?;
        let payload = serde_json::to_string(&value)?;
        Ok(LessonEvent { id, lesson_id: lesson_id.to_owned(), kind: kind.to_owned(), payload, hlc })
    }

    /// The newest placement result (a `placed` event on lesson id
    /// `placement`), from any device.
    pub fn placement(&self) -> Result<Option<kabar_protocol::test::PlacementResult>> {
        let last = self.lesson_events("placement")?.into_iter().rev().find(|e| e.kind == "placed");
        Ok(match last {
            Some(e) => Some(serde_json::from_str(&e.payload)?),
            None => None,
        })
    }

    /// A lesson's events, oldest first.
    pub fn lesson_events(&self, lesson_id: &str) -> Result<Vec<LessonEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, lesson_id, kind, payload, device_id, hlc_wall, hlc_counter FROM lesson_events
             WHERE deleted = 0 AND lesson_id = ?1 ORDER BY hlc_wall, hlc_counter",
        )?;
        let rows = stmt.query_map([lesson_id], |r| {
            Ok(LessonEvent {
                id: r.get(0)?,
                lesson_id: r.get(1)?,
                kind: r.get(2)?,
                payload: r.get(3)?,
                hlc: Hlc {
                    device: DeviceId(r.get(4)?),
                    wall_ms: r.get::<_, i64>(5)? as u64,
                    counter: r.get(6)?,
                },
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Saves this device's Lanjut position (one row per device, LWW by HLC).
    pub fn save_position(&mut self, position: &LessonPosition) -> Result<()> {
        self.write_session(&serde_json::to_string(position)?, false)
    }

    /// Forgets this device's position (the lesson was finished). A tombstone,
    /// so other devices stop offering to continue it once synced.
    pub fn clear_position(&mut self) -> Result<()> {
        self.write_session("null", true)
    }

    fn write_session(&mut self, payload: &str, deleted: bool) -> Result<()> {
        let device = self.device.0.clone();
        let (kind, payload) = if deleted {
            (OpKind::Delete, None)
        } else {
            (OpKind::Upsert, Some(serde_json::from_str(payload)?))
        };
        self.write_synced("sessions", &device, kind, payload).map(drop)
    }

    /// The newest unfinished lesson position from any device (Lanjut).
    pub fn position(&self) -> Result<Option<LessonPosition>> {
        let payload = self
            .conn
            .query_row(
                "SELECT payload FROM sessions WHERE deleted = 0
                 ORDER BY hlc_wall DESC, hlc_counter DESC, device_id DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        Ok(match payload {
            Some(p) => Some(serde_json::from_str(&p)?),
            None => None,
        })
    }
}

/// One row of `lesson_events`; `payload` is protocol JSON for `kind`.
#[derive(Clone, Debug, PartialEq)]
pub struct LessonEvent {
    pub id: String,
    pub lesson_id: String,
    pub kind: String,
    pub payload: String,
    pub hlc: Hlc,
}

// Review cards and the review log (M6).
mod reader;
mod reviews;
pub use reader::{Highlight, SavedText, SavedWord, TextKind};
// Tablet Studio ink: notebooks, pages, strokes, items, transcripts (M9).
mod ink;
// Teach Kabar's personal dictionary (spec §26).
mod taught;
// Persona chats (M12): conversations and their messages.
mod chat;
// The learning journal: reflections and the timeline's extra sources.
mod journal;
pub use journal::{LessonDone, TaughtAt};
// One-click full export (spec §7).
pub mod export;
// A database restored from another device takes its own device id.
mod install;

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
