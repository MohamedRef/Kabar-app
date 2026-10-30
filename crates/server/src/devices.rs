//! Device registry, kept in the account's Durable Object next to the sync
//! hub, so a remote sign-out and closing that device's socket happen in one
//! place.

use kabar_protocol::DeviceId;
use kabar_protocol::cloud::{DeviceInfo, DeviceSummary, Platform};

use crate::sql::{Sql, SqlError, Value, col};
use crate::tokens::REFRESH_TTL_MS;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS devices (
    device_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    model TEXT,
    platform TEXT NOT NULL,
    email TEXT,
    signed_in_ms INTEGER NOT NULL,
    last_seen_ms INTEGER NOT NULL,
    last_sync_ms INTEGER,
    refresh_hash TEXT,
    refresh_expires_ms INTEGER,
    prev_hash TEXT,
    rotated_ms INTEGER,
    revoked_ms INTEGER
)";

/// Schema changes after M2, applied once each by [`migrate`].
pub const SCHEMA_STEPS: &[&str] = &[
    // M7: the device's FCM registration token, for Kak Nur's reminders.
    "ALTER TABLE devices ADD COLUMN push_token TEXT",
    // A refresh token is retired only after its replacement was presented.
    "ALTER TABLE devices ADD COLUMN retired_hash TEXT",
];

/// Creates the table and applies [`SCHEMA_STEPS`]. Safe to call on every start.
pub fn migrate(db: &impl Sql) -> Result<(), SqlError> {
    db.exec_batch(SCHEMA)?;
    crate::sql::migrate(db, "devices_schema", SCHEMA_STEPS)
}

/// A signed-in device that can receive pushes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushTarget {
    pub device: DeviceId,
    pub token: String,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RefreshError {
    #[error("unknown device or token")]
    Unknown,
    #[error("device was signed out")]
    Revoked,
    #[error("refresh token expired")]
    Expired,
    /// An old refresh token came back after its grace period: someone may
    /// have copied it. The device is signed out to be safe.
    #[error("refresh token reused")]
    Reused,
    #[error(transparent)]
    Sql(#[from] SqlError),
}

fn platform_str(p: Platform) -> &'static str {
    match p {
        Platform::Android => "android",
        Platform::Macos => "macos",
        Platform::Web => "web",
        Platform::Other => "other",
    }
}

fn platform_from(s: &str) -> Platform {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).unwrap_or(Platform::Other)
}

/// Keeps user-supplied labels short and printable.
fn clean_label(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_owned()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refreshed {
    pub expires_ms: u64,
    pub email: String,
}

pub struct Devices<'a, S: Sql> {
    db: &'a S,
}

impl<'a, S: Sql> Devices<'a, S> {
    /// Creates the tables if needed, then opens.
    pub fn new(db: &'a S) -> Result<Self, SqlError> {
        migrate(db)?;
        Ok(Self::open(db))
    }

    /// Opens without touching the schema (the Durable Object migrates once at start).
    pub fn open(db: &'a S) -> Self {
        Self { db }
    }

    /// Stores (or with `None`, forgets) a device's FCM token. Returns false
    /// when the device is unknown or signed out.
    pub fn set_push_token(&self, device: &DeviceId, token: Option<&str>) -> Result<bool, SqlError> {
        let rows = self.db.exec(
            "UPDATE devices SET push_token = ? WHERE device_id = ? AND revoked_ms IS NULL RETURNING device_id",
            &[token.into(), device.0.as_str().into()],
        )?;
        Ok(!rows.is_empty())
    }

    /// FCM said the token is gone (app uninstalled, data cleared). Forgets it
    /// unless the device has already sent a newer one.
    pub fn drop_push_token(&self, device: &DeviceId, token: &str) -> Result<(), SqlError> {
        self.db.exec(
            "UPDATE devices SET push_token = NULL WHERE device_id = ? AND push_token = ?",
            &[device.0.as_str().into(), token.into()],
        )?;
        Ok(())
    }

    /// Every signed-in device with a push token.
    pub fn push_targets(&self) -> Result<Vec<PushTarget>, SqlError> {
        let rows = self.db.exec(
            "SELECT device_id, push_token FROM devices
             WHERE revoked_ms IS NULL AND push_token IS NOT NULL ORDER BY device_id",
            &[],
        )?;
        rows.into_iter()
            .map(|r| {
                Ok(PushTarget {
                    device: DeviceId(col(&r, 0, "device_id")?.as_str().unwrap_or_default().to_owned()),
                    token: col(&r, 1, "push_token")?.as_str().unwrap_or_default().to_owned(),
                })
            })
            .collect()
    }

    /// Records a sign-in: creates the device or refreshes its details, and
    /// clears an earlier remote sign-out. Returns the refresh token's expiry.
    pub fn register(
        &self,
        info: &DeviceInfo,
        email: &str,
        refresh_hash: &str,
        now_ms: u64,
    ) -> Result<u64, SqlError> {
        let expires = now_ms + REFRESH_TTL_MS;
        let name = clean_label(&info.name, 60);
        let name = if name.is_empty() { info.device_id.0.clone() } else { name };
        self.db.exec(
            "INSERT INTO devices (device_id, name, model, platform, email, signed_in_ms, last_seen_ms,
                                  refresh_hash, refresh_expires_ms, prev_hash, rotated_ms, revoked_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
             ON CONFLICT (device_id) DO UPDATE SET
                name = excluded.name, model = excluded.model, platform = excluded.platform,
                email = excluded.email, signed_in_ms = excluded.signed_in_ms,
                last_seen_ms = excluded.last_seen_ms, refresh_hash = excluded.refresh_hash,
                refresh_expires_ms = excluded.refresh_expires_ms,
                prev_hash = NULL, retired_hash = NULL, rotated_ms = NULL, revoked_ms = NULL",
            &[
                info.device_id.0.as_str().into(),
                name.into(),
                info.model.as_deref().map(|m| clean_label(m, 60)).into(),
                platform_str(info.platform).into(),
                email.into(),
                now_ms.into(),
                now_ms.into(),
                refresh_hash.into(),
                expires.into(),
            ],
        )?;
        Ok(expires)
    }

    /// Rotates a refresh token without signing a device out when Keychain
    /// or Keystore could not persist the replacement. The previous token
    /// remains valid until the newly issued token is actually presented;
    /// presenting a retired token is treated as reuse and revokes the device.
    pub fn refresh(
        &self,
        device: &DeviceId,
        presented_hash: &str,
        new_hash: &str,
        now_ms: u64,
    ) -> Result<Refreshed, RefreshError> {
        let rows = self.db.exec(
            "SELECT refresh_hash, refresh_expires_ms, prev_hash, retired_hash, revoked_ms, email
             FROM devices WHERE device_id = ?",
            &[device.0.as_str().into()],
        )?;
        let row = rows.first().ok_or(RefreshError::Unknown)?;
        if !matches!(col(row, 4, "revoked_ms")?, Value::Null) {
            return Err(RefreshError::Revoked);
        }
        let current = col(row, 0, "refresh_hash")?.as_str();
        let expires = col(row, 1, "refresh_expires_ms")?.as_u64().unwrap_or(0);
        let prev = col(row, 2, "prev_hash")?.as_str();
        let retired = col(row, 3, "retired_hash")?.as_str();
        let email = col(row, 5, "email")?.as_str().unwrap_or_default().to_owned();

        // Only use of the current token confirms the previous rotation.
        // A client holding the previous token may retry even after a long
        // offline period; it could not have stored the latest one.
        let commit = if current == Some(presented_hash) {
            true
        } else if prev == Some(presented_hash) {
            false
        } else if retired == Some(presented_hash) {
            self.revoke(device, now_ms)?;
            return Err(RefreshError::Reused);
        } else {
            return Err(RefreshError::Unknown);
        };
        if expires <= now_ms {
            return Err(RefreshError::Expired);
        }

        let new_expires = now_ms + REFRESH_TTL_MS;
        let sql = if commit {
            "UPDATE devices SET retired_hash = prev_hash, prev_hash = refresh_hash, refresh_hash = ?,
                                refresh_expires_ms = ?, rotated_ms = ?, last_seen_ms = ?
             WHERE device_id = ?"
        } else {
            "UPDATE devices SET refresh_hash = ?, refresh_expires_ms = ?, rotated_ms = ?, last_seen_ms = ?
             WHERE device_id = ?"
        };
        self.db.exec(
            sql,
            &[new_hash.into(), new_expires.into(), now_ms.into(), now_ms.into(), device.0.as_str().into()],
        )?;
        Ok(Refreshed { expires_ms: new_expires, email })
    }

    /// Signs a device out. Returns false if it doesn't exist.
    pub fn revoke(&self, device: &DeviceId, now_ms: u64) -> Result<bool, SqlError> {
        let rows = self.db.exec(
            "UPDATE devices SET revoked_ms = COALESCE(revoked_ms, ?), refresh_hash = NULL, prev_hash = NULL,
                                retired_hash = NULL, push_token = NULL
             WHERE device_id = ? RETURNING device_id",
            &[now_ms.into(), device.0.as_str().into()],
        )?;
        Ok(!rows.is_empty())
    }

    pub fn is_active(&self, device: &DeviceId) -> Result<bool, SqlError> {
        let rows = self.db.exec(
            "SELECT 1 FROM devices WHERE device_id = ? AND revoked_ms IS NULL",
            &[device.0.as_str().into()],
        )?;
        Ok(!rows.is_empty())
    }

    /// Notes activity; `synced` when ops moved in either direction.
    pub fn touch(&self, device: &DeviceId, now_ms: u64, synced: bool) -> Result<(), SqlError> {
        let id: Value = device.0.as_str().into();
        if synced {
            self.db.exec(
                "UPDATE devices SET last_seen_ms = ?, last_sync_ms = ? WHERE device_id = ?",
                &[now_ms.into(), now_ms.into(), id],
            )?;
        } else {
            self.db.exec("UPDATE devices SET last_seen_ms = ? WHERE device_id = ?", &[now_ms.into(), id])?;
        }
        Ok(())
    }

    /// Everything the Devices screen shows, most recently seen first.
    pub fn list(&self, current: &DeviceId) -> Result<Vec<DeviceSummary>, SqlError> {
        let rows = self.db.exec(
            "SELECT device_id, name, model, platform, signed_in_ms, last_seen_ms, last_sync_ms, revoked_ms
             FROM devices ORDER BY last_seen_ms DESC",
            &[],
        )?;
        rows.into_iter()
            .map(|row| {
                let id = col(&row, 0, "device_id")?.as_str().unwrap_or_default().to_owned();
                Ok(DeviceSummary {
                    current: id == current.0,
                    device_id: DeviceId(id),
                    name: col(&row, 1, "name")?.as_str().unwrap_or_default().to_owned(),
                    model: col(&row, 2, "model")?.as_str().map(str::to_owned),
                    platform: platform_from(col(&row, 3, "platform")?.as_str().unwrap_or_default()),
                    signed_in_at_ms: col(&row, 4, "signed_in_ms")?.as_u64().unwrap_or(0),
                    last_seen_ms: col(&row, 5, "last_seen_ms")?.as_u64().unwrap_or(0),
                    last_sync_ms: col(&row, 6, "last_sync_ms")?.as_u64(),
                    revoked: !matches!(col(&row, 7, "revoked_ms")?, Value::Null),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::testing::MemSql;

    const NOW: u64 = 1_793_011_856_000;

    fn info(id: &str, name: &str, platform: Platform) -> DeviceInfo {
        DeviceInfo {
            device_id: DeviceId(id.into()),
            name: name.into(),
            model: Some("SM-S928B".into()),
            platform,
        }
    }

    fn id(s: &str) -> DeviceId {
        DeviceId(s.into())
    }

    #[test]
    fn sign_in_list_and_remote_sign_out() {
        let db = MemSql::new();
        let d = Devices::new(&db).unwrap();
        d.register(&info("s24u", "Galaxy S24 Ultra", Platform::Android), "me@example.com", "h1", NOW)
            .unwrap();
        d.register(&info("tab", "Galaxy Tab S11", Platform::Android), "me@example.com", "h2", NOW + 10)
            .unwrap();
        d.touch(&id("s24u"), NOW + 20, true).unwrap();

        let list = d.list(&id("tab")).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].device_id, id("s24u"));
        assert_eq!(list[0].last_sync_ms, Some(NOW + 20));
        assert!(!list[0].current && list[1].current);
        assert_eq!(list[1].last_sync_ms, None);

        assert!(d.revoke(&id("s24u"), NOW + 30).unwrap());
        assert!(!d.is_active(&id("s24u")).unwrap());
        assert!(d.is_active(&id("tab")).unwrap());
        assert_eq!(d.refresh(&id("s24u"), "h1", "h3", NOW + 40), Err(RefreshError::Revoked));
        assert!(d.list(&id("tab")).unwrap()[0].revoked);
        assert!(!d.revoke(&id("ghost"), NOW).unwrap());

        // Signing in again on the phone brings it back.
        d.register(&info("s24u", "Galaxy S24 Ultra", Platform::Android), "me@example.com", "h4", NOW + 50)
            .unwrap();
        assert!(d.is_active(&id("s24u")).unwrap());
    }

    #[test]
    fn refresh_rotates_and_detects_reuse() {
        let db = MemSql::new();
        let d = Devices::new(&db).unwrap();
        d.register(&info("mac", "MacBook Air", Platform::Macos), "me@example.com", "h1", NOW).unwrap();

        assert_eq!(d.refresh(&id("mac"), "wrong", "x", NOW), Err(RefreshError::Unknown));
        let r = d.refresh(&id("mac"), "h1", "h2", NOW + 1000).unwrap();
        assert_eq!(r, Refreshed { expires_ms: NOW + 1000 + REFRESH_TTL_MS, email: "me@example.com".into() });
        // A lost response or failed Keychain write can leave h1 on disk.
        // Retrying it after the former two-minute grace must NOT revoke.
        let late = NOW + 20 * 60 * 1000;
        d.refresh(&id("mac"), "h1", "h3", late).unwrap();
        assert!(d.is_active(&id("mac")).unwrap());
        d.refresh(&id("mac"), "h1", "h4", late + 1000).unwrap();
        // Presenting h4 confirms the rotation; h1 becomes retired.
        d.refresh(&id("mac"), "h4", "h5", late + 2000).unwrap();
        assert_eq!(
            d.refresh(&id("mac"), "h1", "h6", late + 3000),
            Err(RefreshError::Reused)
        );
        assert!(!d.is_active(&id("mac")).unwrap());
    }

    #[test]
    fn refresh_tokens_expire() {
        let db = MemSql::new();
        let d = Devices::new(&db).unwrap();
        d.register(&info("mac", "MacBook Air", Platform::Macos), "me@example.com", "h1", NOW).unwrap();
        assert_eq!(d.refresh(&id("mac"), "h1", "h2", NOW + REFRESH_TTL_MS), Err(RefreshError::Expired));
    }

    #[test]
    fn push_tokens_follow_sign_in_and_out() {
        let db = MemSql::new();
        let d = Devices::new(&db).unwrap();
        d.register(&info("s24u", "Galaxy S24 Ultra", Platform::Android), "me@example.com", "h1", NOW)
            .unwrap();
        d.register(&info("tab", "Galaxy Tab S11", Platform::Android), "me@example.com", "h2", NOW).unwrap();
        assert!(d.set_push_token(&id("s24u"), Some("tok-phone")).unwrap());
        assert!(d.set_push_token(&id("tab"), Some("tok-tab")).unwrap());
        assert!(!d.set_push_token(&id("ghost"), Some("x")).unwrap());
        assert_eq!(d.push_targets().unwrap().len(), 2);

        // A remote sign-out forgets the token, and a revoked device can't set one.
        d.revoke(&id("tab"), NOW).unwrap();
        assert!(!d.set_push_token(&id("tab"), Some("tok-tab")).unwrap());
        assert_eq!(
            d.push_targets().unwrap(),
            vec![PushTarget { device: id("s24u"), token: "tok-phone".into() }]
        );

        // FCM's "unregistered" only clears the token it was about.
        d.set_push_token(&id("s24u"), Some("tok-phone-2")).unwrap();
        d.drop_push_token(&id("s24u"), "tok-phone").unwrap();
        assert_eq!(d.push_targets().unwrap().len(), 1);
        d.drop_push_token(&id("s24u"), "tok-phone-2").unwrap();
        assert!(d.push_targets().unwrap().is_empty());
    }

    #[test]
    fn m2_tables_gain_the_push_column() {
        let db = MemSql::new();
        // A Durable Object created by M2: devices table without push_token.
        db.exec_batch(SCHEMA).unwrap();
        db.exec(
            "INSERT INTO devices (device_id, name, platform, signed_in_ms, last_seen_ms) VALUES ('mac', 'Mac', 'macos', 1, 1)",
            &[],
        )
        .unwrap();
        migrate(&db).unwrap();
        migrate(&db).unwrap();
        let d = Devices::open(&db);
        assert!(d.set_push_token(&id("mac"), Some("t")).unwrap());
    }

    #[test]
    fn labels_are_cleaned() {
        let db = MemSql::new();
        let d = Devices::new(&db).unwrap();
        d.register(&info("x", "  \u{7}Tab\n ", Platform::Other), "me@example.com", "h", NOW).unwrap();
        d.register(&info("y", "", Platform::Web), "me@example.com", "h", NOW).unwrap();
        let list = d.list(&id("x")).unwrap();
        let name = |dev: &str| list.iter().find(|s| s.device_id.0 == dev).unwrap().name.clone();
        assert_eq!(name("x"), "Tab");
        assert_eq!(name("y"), "y");
    }
}
