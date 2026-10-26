//! Local SQLite store. Every synced table carries account, device, HLC and a
//! tombstone flag so M2 sync can replay rows without schema changes.

use std::collections::BTreeSet;
use std::path::Path;

use kabar_core::clock::HlcClock;
use kabar_core::settings::Settings;
use kabar_protocol::coach::{
    ActivityEvent, ActivityKind, CoachProfile, DailyPlan, LocalDate, PROFILE_ROW_ID, REMINDER_ROW_ID,
    ReminderState, WeeklyReport,
};
use kabar_protocol::lesson::LessonPosition;
use kabar_protocol::pen::PenLabReport;
use kabar_protocol::{DeviceId, Hlc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

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
    // 5: Reader (M5): texts, word statuses, highlights, saved words, feedback.
    "CREATE TABLE texts (
        id          TEXT PRIMARY KEY,
        title       TEXT NOT NULL,
        body        TEXT NOT NULL,
        kind        TEXT NOT NULL,
        source      TEXT,
        layout      TEXT,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE word_status (
        lexeme      TEXT PRIMARY KEY,
        status      TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE TABLE highlights (
        id          TEXT PRIMARY KEY,
        text_id     TEXT NOT NULL,
        start_byte  INTEGER NOT NULL,
        end_byte    INTEGER NOT NULL,
        phrase      TEXT NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;
    CREATE INDEX highlights_text ON highlights (text_id);
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
    ) STRICT;
    CREATE TABLE quality_feedback (
        id          TEXT PRIMARY KEY,
        task        TEXT NOT NULL,
        item        TEXT NOT NULL,
        vote        INTEGER NOT NULL,
        account_id  TEXT,
        device_id   TEXT NOT NULL,
        hlc_wall    INTEGER NOT NULL,
        hlc_counter INTEGER NOT NULL,
        deleted     INTEGER NOT NULL DEFAULT 0
    ) STRICT;",
];

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
    "texts",
    "word_status",
    "highlights",
    "saved_words",
    "quality_feedback",
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
    fn table(self) -> &'static str {
        match self {
            Self::Goals => "goals",
            Self::DailyPlans => "daily_plans",
            Self::Reminders => "reminders",
            Self::Reports => "reports",
        }
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

    /// Last-writer-wins upsert with a fresh HLC.
    fn put_doc<T: Serialize>(&mut self, doc: Doc, id: &str, value: &T) -> Result<()> {
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            &format!(
                "INSERT INTO {} (id, value, device_id, hlc_wall, hlc_counter, deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0)
                 ON CONFLICT(id) DO UPDATE SET
                    value = excluded.value, device_id = excluded.device_id,
                    hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter, deleted = 0",
                doc.table()
            ),
            params![id, serde_json::to_string(value)?, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
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
        self.conn.execute(
            "INSERT INTO xp_events (id, date, event, device_id, hlc_wall, hlc_counter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.id,
                date.to_string(),
                serde_json::to_string(&event)?,
                self.device.0,
                event.hlc.wall_ms as i64,
                event.hlc.counter
            ],
        )?;
        Ok(event)
    }

    /// Undo for an event (a plan item unticked, a purchase taken back): a
    /// tombstone, so sync removes it on every device.
    pub fn retract_activity(&mut self, id: &str) -> Result<()> {
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            "UPDATE xp_events SET deleted = 1, device_id = ?2, hlc_wall = ?3, hlc_counter = ?4 WHERE id = ?1",
            params![id, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(())
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

    /// A device-local value that never syncs (e.g. the batik in use here).
    pub fn local_value(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_local_value(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
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

    /// Appends a lesson event (`completed`, ...). Append-only: never updated.
    pub fn record_lesson_event(
        &mut self,
        lesson_id: &str,
        kind: &str,
        payload: &impl serde::Serialize,
    ) -> Result<LessonEvent> {
        let hlc = self.clock.tick(now_ms());
        let id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::to_string(payload)?;
        self.conn.execute(
            "INSERT INTO lesson_events (id, lesson_id, kind, payload, device_id, hlc_wall, hlc_counter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, lesson_id, kind, payload, self.device.0, hlc.wall_ms as i64, hlc.counter],
        )?;
        Ok(LessonEvent { id, lesson_id: lesson_id.to_owned(), kind: kind.to_owned(), payload, hlc })
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
        let hlc = self.clock.tick(now_ms());
        self.conn.execute(
            "INSERT INTO sessions (device, payload, device_id, hlc_wall, hlc_counter, deleted)
             VALUES (?1, ?2, ?1, ?3, ?4, ?5)
             ON CONFLICT(device) DO UPDATE SET
                payload = excluded.payload, device_id = excluded.device_id,
                hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter, deleted = excluded.deleted",
            params![self.device.0, payload, hlc.wall_ms as i64, hlc.counter, deleted],
        )?;
        Ok(())
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
pub use reader::{Highlight, SavedText, TextKind};
// Teach Kabar's personal dictionary (spec §26).
mod taught;

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

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use kabar_core::coach::history;
    use kabar_core::settings::Theme;
    use kabar_protocol::coach::{LeagueResult, PlanItem, PlanItemKind, PlanSize};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> LocalDate {
        LocalDate::from_ymd(y, m, d).unwrap()
    }

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
    fn coach_profile_round_trips_and_overwrites() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.coach_profile().unwrap(), None);
        let mut p = CoachProfile { name: "Mohamed".into(), ..Default::default() };
        store.save_coach_profile(&p).unwrap();
        p.utc_offset_min = 420;
        store.save_coach_profile(&p).unwrap();
        assert_eq!(store.coach_profile().unwrap(), Some(p));
    }

    #[test]
    fn plans_are_keyed_by_date() {
        let mut store = Store::open_in_memory().unwrap();
        let today = day(2026, 10, 26);
        let plan = DailyPlan {
            date: today,
            size: PlanSize::Quick,
            comeback: false,
            items: vec![PlanItem { id: "reading".into(), kind: PlanItemKind::Reading, minutes: 5 }],
        };
        store.save_plan(&plan).unwrap();
        assert_eq!(store.plan_for(today).unwrap(), Some(plan));
        assert_eq!(store.plan_for(today.add_days(1)).unwrap(), None);
    }

    #[test]
    fn activity_is_appended_filtered_and_retractable() {
        let mut store = Store::open_in_memory().unwrap();
        let today = day(2026, 10, 26);
        let a = store
            .add_activity(ActivityKind::Reviews { cards: 10, correct: 8 }, today.add_days(-2), 300)
            .unwrap();
        let words = ActivityKind::NewWords { words: vec!["membaca".into(), "buku".into()] };
        let b = store.add_activity(words, today, 120).unwrap();
        let c =
            store.add_activity(ActivityKind::PlanItemDone { item_id: "review".into() }, today, 300).unwrap();
        assert!(c.hlc > b.hlc && b.hlc > a.hlc);
        assert_eq!(store.activity(None).unwrap().len(), 3);
        let recent = store.activity(Some(today)).unwrap();
        assert_eq!(recent, vec![b.clone(), c.clone()]);
        assert_eq!(history::day(&recent, today).minutes(), 7);

        store.retract_activity(&c.id).unwrap();
        assert_eq!(store.activity(Some(today)).unwrap(), vec![b]);
    }

    #[test]
    fn reminders_and_reports() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.reminder_state().unwrap(), ReminderState::default());
        let state = ReminderState { snoozed_until_ms: Some(42), ..Default::default() };
        store.save_reminder_state(&state).unwrap();
        assert_eq!(store.reminder_state().unwrap(), state);

        assert_eq!(store.latest_report_week().unwrap(), None);
        for (week, minutes) in [(day(2026, 10, 12), 90), (day(2026, 10, 19), 120)] {
            store
                .save_report(&WeeklyReport {
                    week_start: week,
                    minutes,
                    days_studied: 4,
                    new_words: 12,
                    retention: Some(0.85),
                    streak: 4,
                    strongest: None,
                    weakest: None,
                    league: LeagueResult { xp: 300, rank: 1, of: 2 },
                    next_focus: vec![],
                })
                .unwrap();
        }
        assert_eq!(store.latest_report_week().unwrap(), Some(day(2026, 10, 19)));
        assert_eq!(store.report_for(day(2026, 10, 12)).unwrap().unwrap().minutes, 90);
    }

    #[test]
    fn clock_resumes_past_coach_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        let mut store = Store::open(&path).unwrap();
        let e =
            store.add_activity(ActivityKind::Reviews { cards: 1, correct: 1 }, day(2026, 10, 26), 0).unwrap();
        // A row stamped far ahead of wall time, as after a skewed clock.
        let ahead = e.hlc.wall_ms + 3_600_000;
        store.conn.execute("UPDATE xp_events SET hlc_wall = ?1", [ahead as i64]).unwrap();
        drop(store);

        let mut store = Store::open(&path).unwrap();
        let next = store.add_activity(ActivityKind::Reviews { cards: 1, correct: 1 }, day(2026, 10, 26), 0);
        assert!(next.unwrap().hlc.wall_ms >= ahead);
    }

    #[test]
    fn local_values_overwrite() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.local_value("batik").unwrap(), None);
        store.set_local_value("batik", "kawung").unwrap();
        store.set_local_value("batik", "parang").unwrap();
        assert_eq!(store.local_value("batik").unwrap().as_deref(), Some("parang"));
    }

    #[test]
    fn completed_lessons_skip_other_kinds_and_tombstones() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.completed_lessons().unwrap().is_empty());
        let rows = [
            ("e1", "bipa1.u01.l01", "completed", 0),
            ("e2", "bipa1.u01.l01", "completed", 0),
            ("e3", "bipa1.u01.l02", "started", 0),
            ("e4", "bipa1.u01.l03", "completed", 1),
            ("e5", "bipa1.test", "completed", 0),
        ];
        for (id, lesson, kind, deleted) in rows {
            store
                .conn
                .execute(
                    "INSERT INTO lesson_events (id, lesson_id, kind, payload, device_id, hlc_wall, hlc_counter, deleted)
                     VALUES (?1, ?2, ?3, '{}', 'd', 1, 0, ?4)",
                    params![id, lesson, kind, deleted],
                )
                .unwrap();
        }
        let done = store.completed_lessons().unwrap();
        assert_eq!(done.into_iter().collect::<Vec<_>>(), ["bipa1.test", "bipa1.u01.l01"]);
    }

    #[test]
    fn lesson_events_are_appended_in_order() {
        use kabar_protocol::lesson::LessonCompleted;
        let mut store = Store::open_in_memory().unwrap();
        let done = LessonCompleted {
            score: 81,
            xp: 19,
            duration_ms: 420_000,
            mistakes: vec!["sore".into()],
            results: vec![],
        };
        let a = store.record_lesson_event("bipa1.u01.l01", "completed", &done).unwrap();
        let b = store.record_lesson_event("bipa1.u01.l01", "completed", &done).unwrap();
        store.record_lesson_event("bipa1.u01.l02", "completed", &done).unwrap();
        assert!(b.hlc > a.hlc);
        let events = store.lesson_events("bipa1.u01.l01").unwrap();
        assert_eq!(events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), [a.id.as_str(), b.id.as_str()]);
        let back: LessonCompleted = serde_json::from_str(&events[0].payload).unwrap();
        assert_eq!(back, done);
    }

    #[test]
    fn position_is_last_writer_wins_and_survives_reopen() {
        use kabar_protocol::lesson::LessonStep;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kabar.db");
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.position().unwrap(), None);
        let at = |id: &str| LessonPosition {
            lesson_id: "bipa1.u01.l02".into(),
            step: LessonStep::Exercise { id: id.into(), retry: false },
            progress: None,
        };
        store.save_position(&at("e03")).unwrap();
        store.save_position(&at("e04")).unwrap();
        drop(store);

        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.position().unwrap(), Some(at("e04")));
        let rows: i64 = store.conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 1, "one row per device");
        store.clear_position().unwrap();
        assert_eq!(store.position().unwrap(), None);
    }

    #[test]
    fn refuses_a_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(migrate(&mut conn), Err(StoreError::TooNew { found: 99, .. })));
    }
}
