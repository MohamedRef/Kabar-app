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

    /// Lanjut's position, unless a reset of its level came after it.
    pub(super) fn position_after_resets(&self) -> Result<Option<LessonPosition>> {
        let row = self
            .conn
            .query_row(
                "SELECT payload, hlc_wall, hlc_counter, device_id FROM sessions WHERE deleted = 0
                 ORDER BY hlc_wall DESC, hlc_counter DESC, device_id DESC LIMIT 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        Hlc {
                            wall_ms: r.get::<_, i64>(1)? as u64,
                            counter: r.get(2)?,
                            device: DeviceId(r.get(3)?),
                        },
                    ))
                },
            )
            .optional()?;
        let Some((payload, hlc)) = row else { return Ok(None) };
        let position: Option<LessonPosition> = serde_json::from_str(&payload)?;
        let resets = self.resets()?;
        Ok(position.filter(|p| resets.lesson_counts(&p.lesson_id, &hlc)))
    }
}
