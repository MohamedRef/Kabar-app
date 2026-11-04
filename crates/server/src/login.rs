//! Username + password sign-in (docs/CLOUD.md §4.1), for devices that
//! don't have Mohamed's Google account.
//!
//! Each username has its own small Durable Object (`Login`, named
//! `u:<username>`), so a sign-in finds its account without a database, and
//! the object being single-threaded makes the wrong-password count exact.
//! The account's own object remembers which username it has ([`AccountLogin`]),
//! so a change releases the old one.
//!
//! The app stretches the password with Argon2id and sends only the 32-byte
//! key (`batna_protocol::cloud::PasswordSignIn`): the Worker's 10 ms CPU
//! budget can't afford a slow hash, and the device can. Here the key gets a
//! random salt and one SHA-256, compared in constant time.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};

use crate::sql::{Sql, SqlError, Value, col};
use crate::tokens::sha256_hex;

/// Wrong passwords allowed before the username locks.
pub const LOCK_AFTER: u32 = 5;
/// The first lock; each further wrong password doubles it, up to [`MAX_LOCK_MS`].
pub const FIRST_LOCK_MS: u64 = 60_000;
/// One hour at most, so a stranger hammering the username can't keep Mohamed
/// out for long, and still only gets about 24 guesses a day.
pub const MAX_LOCK_MS: u64 = 60 * 60_000;

/// The key from the app: base64url of exactly 32 bytes.
pub fn parse_key(b64: &str) -> Option<[u8; 32]> {
    URL_SAFE_NO_PAD.decode(b64.trim()).ok()?.try_into().ok()
}

/// What the Login object stores for a key: never the key itself.
pub fn verifier(salt_hex: &str, key: &[u8; 32]) -> String {
    let mut input = Vec::with_capacity(64 + salt_hex.len());
    input.extend_from_slice(b"kabar-login-verifier-v1:");
    input.extend_from_slice(salt_hex.as_bytes());
    input.push(b':');
    input.extend_from_slice(key);
    sha256_hex(&input)
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// How long a username stays locked after `failures` wrong passwords in a row.
pub fn lock_for(failures: u32) -> Option<u64> {
    let over = failures.checked_sub(LOCK_AFTER)?;
    Some(FIRST_LOCK_MS.saturating_mul(1u64 << over.min(20)).min(MAX_LOCK_MS))
}

pub const LOGIN_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS login (
    one INTEGER PRIMARY KEY CHECK (one = 1),
    account_id TEXT NOT NULL,
    email TEXT NOT NULL,
    salt TEXT NOT NULL,
    verifier TEXT NOT NULL,
    updated_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS tries (
    one INTEGER PRIMARY KEY CHECK (one = 1),
    failures INTEGER NOT NULL,
    locked_until_ms INTEGER NOT NULL
)";

/// The answer to a sign-in attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Checked {
    Ok {
        account_id: String,
        email: String,
    },
    /// Wrong password, or nobody has this username: the same answer.
    Wrong,
    Locked {
        retry_after_ms: u64,
    },
}

/// One username's Durable Object.
pub struct Login<'a, S: Sql> {
    db: &'a S,
}

impl<'a, S: Sql> Login<'a, S> {
    pub fn new(db: &'a S) -> Result<Self, SqlError> {
        db.exec_batch(LOGIN_SCHEMA)?;
        Ok(Self { db })
    }

    fn owner(&self) -> Result<Option<(String, String, String, String)>, SqlError> {
        let rows = self.db.exec("SELECT account_id, email, salt, verifier FROM login WHERE one = 1", &[])?;
        let Some(row) = rows.first() else { return Ok(None) };
        let text = |i, name| Ok::<_, SqlError>(col(row, i, name)?.as_str().unwrap_or_default().to_owned());
        Ok(Some((text(0, "account_id")?, text(1, "email")?, text(2, "salt")?, text(3, "verifier")?)))
    }

    /// Gives the username to `account_id` with a new password. False when
    /// another account has it. A new password also clears the lock.
    pub fn set(
        &self,
        account_id: &str,
        email: &str,
        salt_hex: &str,
        verifier: &str,
        now_ms: u64,
    ) -> Result<bool, SqlError> {
        if let Some((owner, ..)) = self.owner()?
            && owner != account_id
        {
            return Ok(false);
        }
        self.db.exec(
            "INSERT INTO login (one, account_id, email, salt, verifier, updated_ms) VALUES (1, ?, ?, ?, ?, ?)
             ON CONFLICT (one) DO UPDATE SET account_id = excluded.account_id, email = excluded.email,
                salt = excluded.salt, verifier = excluded.verifier, updated_ms = excluded.updated_ms",
            &[account_id.into(), email.into(), salt_hex.into(), verifier.into(), now_ms.into()],
        )?;
        self.db.exec("DELETE FROM tries", &[])?;
        Ok(true)
    }

    /// Releases the username, if `account_id` holds it.
    pub fn clear(&self, account_id: &str) -> Result<bool, SqlError> {
        let rows =
            self.db.exec("DELETE FROM login WHERE account_id = ? RETURNING one", &[account_id.into()])?;
        if !rows.is_empty() {
            self.db.exec("DELETE FROM tries", &[])?;
        }
        Ok(!rows.is_empty())
    }

    /// Checks a key, counting wrong ones. Nothing is written for a username
    /// nobody has, so guessing names leaves no storage behind.
    pub fn check(&self, key: &[u8; 32], now_ms: u64) -> Result<Checked, SqlError> {
        let Some((account_id, email, salt, stored)) = self.owner()? else {
            return Ok(Checked::Wrong);
        };
        let rows = self.db.exec("SELECT failures, locked_until_ms FROM tries WHERE one = 1", &[])?;
        let (failures, locked_until) = match rows.first() {
            Some(row) => (
                col(row, 0, "failures")?.as_u64().unwrap_or(0) as u32,
                col(row, 1, "locked_until_ms")?.as_u64().unwrap_or(0),
            ),
            None => (0, 0),
        };
        if locked_until > now_ms {
            return Ok(Checked::Locked { retry_after_ms: locked_until - now_ms });
        }
        if same(&verifier(&salt, key), &stored) {
            self.db.exec("DELETE FROM tries", &[])?;
            return Ok(Checked::Ok { account_id, email });
        }
        let failures = failures.saturating_add(1);
        let lock = lock_for(failures);
        self.db.exec(
            "INSERT INTO tries (one, failures, locked_until_ms) VALUES (1, ?, ?)
             ON CONFLICT (one) DO UPDATE SET failures = excluded.failures,
                locked_until_ms = excluded.locked_until_ms",
            &[u64::from(failures).into(), lock.map_or(0, |l| now_ms + l).into()],
        )?;
        Ok(match lock {
            Some(retry_after_ms) => Checked::Locked { retry_after_ms },
            None => Checked::Wrong,
        })
    }
}

pub const ACCOUNT_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS login_name (
    one INTEGER PRIMARY KEY CHECK (one = 1),
    username TEXT NOT NULL
)";

/// The account object's side: which username it has.
pub struct AccountLogin<'a, S: Sql> {
    db: &'a S,
}

impl<'a, S: Sql> AccountLogin<'a, S> {
    /// Opens without touching the schema (`account::migrate` creates it).
    pub fn open(db: &'a S) -> Self {
        Self { db }
    }

    pub fn username(&self) -> Result<Option<String>, SqlError> {
        let rows = self.db.exec("SELECT username FROM login_name WHERE one = 1", &[])?;
        Ok(rows.into_iter().next().and_then(|r| r.into_iter().next()).and_then(Value::into_string))
    }

    pub fn set_username(&self, username: Option<&str>) -> Result<(), SqlError> {
        match username {
            Some(u) => self.db.exec(
                "INSERT INTO login_name (one, username) VALUES (1, ?)
                 ON CONFLICT (one) DO UPDATE SET username = excluded.username",
                &[u.into()],
            )?,
            None => self.db.exec("DELETE FROM login_name", &[])?,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::testing::MemSql;

    const NOW: u64 = 1_793_011_856_000;
    const KEY: [u8; 32] = [7; 32];

    fn set(l: &Login<MemSql>, account: &str, key: &[u8; 32]) -> bool {
        let salt = "00112233445566778899aabbccddeeff";
        l.set(account, "me@example.com", salt, &verifier(salt, key), NOW).unwrap()
    }

    #[test]
    fn the_right_key_signs_in_and_a_wrong_one_does_not() {
        let db = MemSql::new();
        let l = Login::new(&db).unwrap();
        assert_eq!(l.check(&KEY, NOW).unwrap(), Checked::Wrong, "nobody has this username yet");
        assert!(set(&l, "gacc", &KEY));
        assert_eq!(
            l.check(&KEY, NOW).unwrap(),
            Checked::Ok { account_id: "gacc".into(), email: "me@example.com".into() }
        );
        assert_eq!(l.check(&[8; 32], NOW).unwrap(), Checked::Wrong);
    }

    #[test]
    fn the_key_itself_is_never_stored() {
        let db = MemSql::new();
        let l = Login::new(&db).unwrap();
        set(&l, "gacc", &KEY);
        let dump = format!("{:?}", db.exec("SELECT * FROM login", &[]).unwrap());
        assert!(!dump.contains(&URL_SAFE_NO_PAD.encode(KEY)));
        assert!(!dump.contains(&sha256_hex(&KEY)));
        // Another salt gives another verifier for the same key.
        assert_ne!(verifier("aa", &KEY), verifier("bb", &KEY));
    }

    #[test]
    fn wrong_passwords_lock_the_username_for_longer_each_time() {
        let db = MemSql::new();
        let l = Login::new(&db).unwrap();
        set(&l, "gacc", &KEY);
        let wrong = [9; 32];
        for _ in 1..LOCK_AFTER {
            assert_eq!(l.check(&wrong, NOW).unwrap(), Checked::Wrong);
        }
        assert_eq!(l.check(&wrong, NOW).unwrap(), Checked::Locked { retry_after_ms: FIRST_LOCK_MS });
        // Locked: even the right password waits, and the wait isn't extended.
        assert_eq!(
            l.check(&KEY, NOW + 1000).unwrap(),
            Checked::Locked { retry_after_ms: FIRST_LOCK_MS - 1000 }
        );
        // After the lock, one more wrong one locks for twice as long.
        let later = NOW + FIRST_LOCK_MS;
        assert_eq!(l.check(&wrong, later).unwrap(), Checked::Locked { retry_after_ms: 2 * FIRST_LOCK_MS });
        // The right one after the wait signs in and resets the count.
        let after = later + 2 * FIRST_LOCK_MS;
        assert!(matches!(l.check(&KEY, after).unwrap(), Checked::Ok { .. }));
        assert_eq!(l.check(&wrong, after).unwrap(), Checked::Wrong);
    }

    #[test]
    fn locks_never_pass_an_hour() {
        assert_eq!(lock_for(LOCK_AFTER - 1), None);
        assert_eq!(lock_for(LOCK_AFTER), Some(FIRST_LOCK_MS));
        assert_eq!(lock_for(LOCK_AFTER + 3), Some(8 * FIRST_LOCK_MS));
        assert_eq!(lock_for(LOCK_AFTER + 6), Some(MAX_LOCK_MS));
        assert_eq!(lock_for(u32::MAX), Some(MAX_LOCK_MS));
    }

    #[test]
    fn a_username_belongs_to_one_account() {
        let db = MemSql::new();
        let l = Login::new(&db).unwrap();
        assert!(set(&l, "gacc", &KEY));
        assert!(!set(&l, "gother", &[1; 32]), "taken");
        assert!(set(&l, "gacc", &[2; 32]), "the owner changes the password");
        assert!(matches!(l.check(&[2; 32], NOW).unwrap(), Checked::Ok { .. }));
        assert_eq!(l.check(&KEY, NOW).unwrap(), Checked::Wrong, "the old password is gone");
        assert!(!l.clear("gother").unwrap());
        assert!(l.clear("gacc").unwrap());
        assert_eq!(l.check(&[2; 32], NOW).unwrap(), Checked::Wrong);
        assert!(set(&l, "gother", &[1; 32]), "free again");
    }

    #[test]
    fn a_new_password_lifts_a_lock() {
        let db = MemSql::new();
        let l = Login::new(&db).unwrap();
        set(&l, "gacc", &KEY);
        for _ in 0..LOCK_AFTER {
            l.check(&[9; 32], NOW).unwrap();
        }
        assert!(matches!(l.check(&KEY, NOW).unwrap(), Checked::Locked { .. }));
        set(&l, "gacc", &[3; 32]);
        assert!(matches!(l.check(&[3; 32], NOW).unwrap(), Checked::Ok { .. }));
    }

    #[test]
    fn keys_must_be_32_bytes() {
        assert_eq!(parse_key(&URL_SAFE_NO_PAD.encode(KEY)), Some(KEY));
        assert_eq!(parse_key(&URL_SAFE_NO_PAD.encode([1u8; 16])), None);
        assert_eq!(parse_key("not base64!"), None);
    }

    #[test]
    fn the_account_remembers_its_username() {
        let db = MemSql::new();
        crate::account::migrate(&db).unwrap();
        let a = AccountLogin::open(&db);
        assert_eq!(a.username().unwrap(), None);
        a.set_username(Some("mo.r")).unwrap();
        a.set_username(Some("mo.refaat")).unwrap();
        assert_eq!(a.username().unwrap().as_deref(), Some("mo.refaat"));
        a.set_username(None).unwrap();
        assert_eq!(a.username().unwrap(), None);
    }
}
