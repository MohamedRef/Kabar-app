//! Synced row encoding, materialization and migration helpers.

use super::*;

pub(super) fn write_synced(
    tx: &Transaction<'_>,
    clock: &mut HlcClock,
    table: &str,
    row_id: &str,
    kind: OpKind,
    payload: Option<Value>,
) -> Result<Op> {
    // Newer than whatever this row holds, even if it came from a device whose
    // clock runs ahead, so a local edit always wins over what it replaces.
    let hlc = match row_meta(tx, table, row_id)? {
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
    record_local(tx, op)
}
/// Stores a local op (merge state, typed table) and queues it for push.
pub(super) fn record_local(tx: &Transaction<'_>, op: Op) -> Result<Op> {
    let sync = validate(&op, MAX_PAYLOAD_BYTES).map_err(StoreError::Invalid)?;
    if merge(sync, &op, row_meta(tx, &op.table, &op.row_id)?.as_ref()) == Merge::Ignore {
        // Appending a row that exists, or retracting one twice.
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
pub(super) fn row_meta(conn: &Connection, table: &str, row_id: &str) -> Result<Option<RowMeta>> {
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
pub(super) fn store_row(tx: &Transaction<'_>, op: &Op) -> Result<()> {
    let deleted = op.kind == OpKind::Delete;
    // A tombstone keeps the last payload of an immutable row (strokes and
    // events are retracted, not lost); LWW rows drop it.
    let keep = table_kind(&op.table) == Some(SyncKind::Immutable);
    let payload = op.payload.as_ref().map(serde_json::to_string).transpose()?;
    tx.execute(
        "INSERT INTO sync_rows (tbl, row_id, payload, device_id, hlc_wall, hlc_counter, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(tbl, row_id) DO UPDATE SET
            payload = CASE WHEN excluded.payload IS NULL AND ?8 THEN payload ELSE excluded.payload END,
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
            keep,
        ],
    )?;
    Ok(())
}
/// How a synced table is mirrored into its typed table.
#[derive(Clone, Copy)]
pub(super) enum Typed {
    /// One JSON document per row: `(table, key column, value column)`.
    /// Settings wrap the value as `{ "value": … }`.
    Doc(&'static str, &'static str, &'static str),
    /// Events that carry their own payload.
    XpEvent,
    Review,
    LessonEvent,
}
impl Typed {
    pub(super) fn of(table: &str) -> Option<Self> {
        Some(match table {
            "settings" => Self::Doc("settings", "key", "value"),
            "goals" | "daily_plans" | "reminders" | "reports" => {
                let doc = Doc::from_table(table)?;
                Self::Doc(doc.table(), "id", "value")
            }
            "cards" => Self::Doc("cards", "id", "payload"),
            "sessions" => Self::Doc("sessions", "device", "payload"),
            "taught_entries" => Self::Doc("taught_entries", "id", "entry"),
            "journal_entries" => Self::Doc("journal_entries", "id", "entry"),
            "texts" | "word_status" | "highlights" => {
                let table = ["texts", "word_status", "highlights"].into_iter().find(|t| *t == table)?;
                Self::Doc(table, "id", "payload")
            }
            "xp_events" => Self::XpEvent,
            "review_log" => Self::Review,
            "lesson_events" => Self::LessonEvent,
            _ => return None,
        })
    }

    /// The typed table of an append-only event table.
    pub(super) fn event_table(self) -> Option<&'static str> {
        match self {
            Self::XpEvent => Some("xp_events"),
            Self::Review => Some("review_log"),
            Self::LessonEvent => Some("lesson_events"),
            Self::Doc(..) => None,
        }
    }
}
/// Mirrors a stored op into its typed table, for the tables this build reads.
pub(super) fn materialize(tx: &Transaction<'_>, op: &Op) -> Result<()> {
    let Some(typed) = Typed::of(&op.table) else { return Ok(()) };
    let deleted = op.kind == OpKind::Delete;
    let hlc = &op.hlc;
    let (device, wall, counter) = (&hlc.device.0, hlc.wall_ms as i64, hlc.counter);
    let json = |v: &Value| serde_json::to_string(v);
    match typed {
        Typed::Doc(table, key, column) => {
            let value =
                op.payload.as_ref().and_then(|p| if table == "settings" { p.get("value") } else { Some(p) });
            let value = match value {
                Some(v) if !deleted => Some(json(v)?),
                _ => None,
            };
            tx.execute(
                &format!(
                    "INSERT INTO {table} ({key}, {column}, device_id, hlc_wall, hlc_counter, deleted)
                     VALUES (?1, COALESCE(?2, 'null'), ?3, ?4, ?5, ?6)
                     ON CONFLICT({key}) DO UPDATE SET
                        {column} = COALESCE(?2, {column}), device_id = excluded.device_id,
                        hlc_wall = excluded.hlc_wall, hlc_counter = excluded.hlc_counter,
                        deleted = excluded.deleted"
                ),
                params![op.row_id, value, device, wall, counter, value.is_none() as i64],
            )?;
        }
        _ if deleted => {
            let table = match typed {
                Typed::XpEvent => "xp_events",
                Typed::Review => "review_log",
                _ => "lesson_events",
            };
            tx.execute(
                &format!("UPDATE {table} SET deleted = 1, device_id = ?2, hlc_wall = ?3, hlc_counter = ?4 WHERE id = ?1"),
                params![op.row_id, device, wall, counter],
            )?;
        }
        Typed::XpEvent => {
            let Some(event) = &op.payload else { return Ok(()) };
            let date = event.get("date").and_then(Value::as_str).unwrap_or_default();
            tx.execute(
                "INSERT OR IGNORE INTO xp_events (id, date, event, device_id, hlc_wall, hlc_counter)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![op.row_id, date, json(event)?, device, wall, counter],
            )?;
        }
        Typed::Review => {
            let Some(event) = &op.payload else { return Ok(()) };
            let card = event.get("card_id").and_then(Value::as_str).unwrap_or_default();
            tx.execute(
                "INSERT OR IGNORE INTO review_log (id, card_id, payload, device_id, hlc_wall, hlc_counter)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![op.row_id, card, json(event)?, device, wall, counter],
            )?;
        }
        Typed::LessonEvent => {
            let Some(row) = &op.payload else { return Ok(()) };
            let text = |k: &str| row.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            let payload = json(row.get("payload").unwrap_or(&Value::Null))?;
            tx.execute(
                "INSERT OR IGNORE INTO lesson_events (id, lesson_id, kind, payload, device_id, hlc_wall, hlc_counter)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![op.row_id, text("lesson_id"), text("kind"), payload, device, wall, counter],
            )?;
        }
    }
    Ok(())
}
pub(super) fn meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}
pub(super) fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}
pub(super) fn migrate(conn: &mut Connection) -> Result<()> {
    let known = MIGRATIONS.len() as u32;
    let current: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > known {
        return Err(StoreError::TooNew { found: current, known });
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = i as u32 + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        if version == SYNC_SCHEMA {
            backfill_sync(&tx)?;
        }
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}
/// Queues every row written before sync existed, with its original HLC.
pub(super) fn backfill_sync(tx: &Transaction<'_>) -> Result<()> {
    type Found = (String, String, bool, Hlc);
    // `sql` selects (row id, json, deleted, device_id, hlc_wall, hlc_counter).
    let rows = |sql: &str| -> Result<Vec<Found>> {
        let mut stmt = tx.prepare(sql)?;
        let found = stmt
            .query_map([], |r| {
                let hlc = Hlc {
                    device: DeviceId(r.get(3)?),
                    wall_ms: r.get::<_, i64>(4)? as u64,
                    counter: r.get(5)?,
                };
                Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0, hlc))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(found)
    };
    let sync = "deleted, device_id, hlc_wall, hlc_counter";
    let mut ops = Vec::new();
    // Last-writer-wins documents: their newest state, unless tombstoned.
    let docs = [
        ("settings", format!("SELECT key, value, {sync} FROM settings")),
        ("goals", format!("SELECT id, value, {sync} FROM goals")),
        ("daily_plans", format!("SELECT id, value, {sync} FROM daily_plans")),
        ("reminders", format!("SELECT id, value, {sync} FROM reminders")),
        ("reports", format!("SELECT id, value, {sync} FROM reports")),
        ("cards", format!("SELECT id, payload, {sync} FROM cards")),
        ("sessions", format!("SELECT device, payload, {sync} FROM sessions")),
        ("taught_entries", format!("SELECT id, entry, {sync} FROM taught_entries")),
    ];
    for (table, sql) in docs {
        for (id, value, deleted, hlc) in rows(&sql)? {
            if deleted {
                continue;
            }
            let value: Value = serde_json::from_str(&value)?;
            let payload = if table == "settings" { serde_json::json!({ "value": value }) } else { value };
            ops.push(backfill_op(table, id, OpKind::Upsert, hlc, Some(payload)));
        }
    }
    // Events: appended once; a retracted one is appended, then tombstoned.
    let events = [
        ("xp_events", format!("SELECT id, event, {sync} FROM xp_events")),
        ("review_log", format!("SELECT id, payload, {sync} FROM review_log")),
        (
            "lesson_events",
            format!(
                "SELECT id, json_object('lesson_id', lesson_id, 'kind', kind, 'payload', json(payload)), {sync} FROM lesson_events"
            ),
        ),
    ];
    for (table, sql) in events {
        for (id, payload, deleted, hlc) in rows(&sql)? {
            // An event's own HLC is when it happened; the row's moves on a retraction.
            let created: Value = serde_json::from_str(&payload)?;
            let born = created
                .get("hlc")
                .and_then(|h| serde_json::from_value(h.clone()).ok())
                .unwrap_or(hlc.clone());
            ops.push(backfill_op(table, id.clone(), OpKind::Append, born, Some(created)));
            if deleted && table_kind(table) == Some(SyncKind::Immutable) {
                ops.push(backfill_op(table, id, OpKind::Delete, hlc, None));
            }
        }
    }
    for op in ops {
        store_row(tx, &op)?;
        tx.execute(
            "INSERT INTO outbox (op_id, op) VALUES (?1, ?2)",
            params![op.id, serde_json::to_string(&op)?],
        )?;
    }
    Ok(())
}
pub(super) fn backfill_op(table: &str, row_id: String, kind: OpKind, hlc: Hlc, payload: Option<Value>) -> Op {
    Op { id: uuid::Uuid::now_v7().to_string(), table: table.into(), row_id, kind, hlc, payload }
}
