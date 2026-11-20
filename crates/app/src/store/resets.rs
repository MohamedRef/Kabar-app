//! The reset markers (`kabar_core::reset`, ported from Batna #1341): one
//! synced setting per scope whose row stamp is the cutoff, and the readers
//! that leave out progress behind it. Nothing here deletes a row.

use kabar_core::reset::{PREFIX, Resets, Scope};
use kabar_protocol::Hlc;
use kabar_protocol::lesson::LessonPosition;
use kabar_protocol::srs::ReviewEvent;
use kabar_protocol::sync::OpKind;

use super::{DeviceId, OptionalExtension, Result, Store};

impl Store {
    /// Every reset in force, by its row's stamp (tombstoned ones are gone).
    pub fn resets(&self) -> Result<Resets> {
        let mut stmt = self.conn.prepare(
            "SELECT key, hlc_wall, hlc_counter, device_id FROM settings
             WHERE substr(key, 1, length(?1)) = ?1 AND deleted = 0",
        )?;
        let rows = stmt
            .query_map([PREFIX], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    Hlc {
                        wall_ms: r.get::<_, i64>(1)? as u64,
                        counter: r.get(2)?,
                        device: DeviceId(r.get(3)?),
                    },
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Resets::from_rows(rows))
    }

    /// Restarts `scope` now: one synced setting. Returns its stamp.
    pub fn put_reset(&mut self, scope: &Scope) -> Result<Hlc> {
        let payload = serde_json::json!({ "value": { "scope": scope.id() } });
        self.atomically(|s| {
            // The stamp is the cutoff, so it outranks every row already here,
            // other devices' too. This connection's clock only knows its own
            // rows (the sync engine's connection observes what arrives), and
            // a device whose clock runs ahead (the hub takes up to five
            // minutes) would otherwise keep answers the reset meant to leave
            // out (#687).
            if let Some(newest) = s.newest_stamp()? {
                s.clock.observe(&newest, super::now_ms());
            }
            Ok(s.write_synced("settings", &scope.key(), OpKind::Upsert, Some(payload))?.hlc)
        })
    }

    /// The newest stamp of any synced row, whichever device wrote it.
    fn newest_stamp(&self) -> Result<Option<Hlc>> {
        Ok(self
            .conn
            .query_row(
                "SELECT device_id, hlc_wall, hlc_counter FROM sync_rows
                 ORDER BY hlc_wall DESC, hlc_counter DESC LIMIT 1",
                [],
                |r| {
                    Ok(Hlc {
                        device: DeviceId(r.get(0)?),
                        wall_ms: r.get::<_, i64>(1)? as u64,
                        counter: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    /// Takes a reset back: a tombstone on its setting, on every device.
    pub fn remove_reset(&mut self, scope: &Scope) -> Result<()> {
        self.write_synced("settings", &scope.key(), OpKind::Delete, None).map(drop)
    }

    /// The answers that count as progress: [`Store::review_log`] less what a
    /// reset left out. The deck and word stats replay this; activity
    /// (minutes, streak days) keeps reading the whole log.
    pub fn progress_log(&self) -> Result<Vec<ReviewEvent>> {
        Ok(self.resets()?.progress(self.review_log()?))
    }

    /// Lanjut's position: the newest live session row whose lesson was not
    /// reset or finished at or after that row's stamp (#711).
    pub(super) fn position_after_resets(&self) -> Result<Option<LessonPosition>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload, hlc_wall, hlc_counter, device_id FROM sessions WHERE deleted = 0
             ORDER BY hlc_wall DESC, hlc_counter DESC, device_id DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    Hlc {
                        wall_ms: r.get::<_, i64>(1)? as u64,
                        counter: r.get(2)?,
                        device: DeviceId(r.get(3)?),
                    },
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if rows.is_empty() {
            return Ok(None);
        }
        let resets = self.resets()?;
        for (payload, hlc) in rows {
            let Some(pos) = serde_json::from_str::<Option<LessonPosition>>(&payload)? else { continue };
            if resets.lesson_counts(&pos.lesson_id, &hlc) && !self.completed_since(&pos.lesson_id, &hlc)? {
                return Ok(Some(pos));
            }
        }
        Ok(None)
    }

    fn completed_since(&self, lesson_id: &str, since: &Hlc) -> Result<bool> {
        let mut stmt = self.conn.prepare(
            "SELECT hlc_wall, hlc_counter, device_id FROM lesson_events
             WHERE lesson_id = ?1 AND kind = 'completed' AND deleted = 0 AND hlc_wall >= ?2",
        )?;
        let mut rows = stmt.query(rusqlite::params![lesson_id, since.wall_ms as i64])?;
        while let Some(r) = rows.next()? {
            let hlc = Hlc {
                wall_ms: r.get::<_, i64>(0)? as u64,
                counter: r.get(1)?,
                device: DeviceId(r.get(2)?),
            };
            if hlc >= *since {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
