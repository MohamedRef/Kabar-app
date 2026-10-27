//! The account Durable Object's storage setup, outside `wasm` so its failure
//! path is tested natively.

use kabar_protocol::cloud::{ApiError, ErrorCode};

use crate::sql::{Sql, SqlError};
use crate::{blobs, devices, hub};

/// Creates or upgrades every table the account keeps. Safe on every start.
pub fn migrate(db: &impl Sql) -> Result<(), SqlError> {
    devices::migrate(db)?;
    db.exec_batch(hub::SCHEMA)?;
    db.exec_batch(blobs::SCHEMA)
}

/// Sets up storage, then sweeps the op log once. A failed sweep only costs
/// space, so it is logged through `warn` and the account still serves; a
/// failed migration is returned, and the object then refuses every request
/// with [`unavailable`] instead of half-working on a broken schema.
pub fn open(db: &impl Sql, warn: impl Fn(&str)) -> Result<(), String> {
    migrate(db).map_err(|e| e.to_string())?;
    if let Err(e) = hub::Hub::open(db).compact_once() {
        warn(&format!("op log sweep failed: {e}"));
    }
    Ok(())
}

/// The answer to every request while the account's storage couldn't be set
/// up. The object retries on its next start.
pub fn unavailable(why: &str) -> ApiError {
    ApiError {
        code: ErrorCode::Internal,
        message: format!("this account's storage couldn't be set up ({why}); Kabar Cloud retries on restart"),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::sql::testing::MemSql;

    #[test]
    fn a_fresh_account_opens() {
        let db = MemSql::new();
        let warned = RefCell::new(Vec::new());
        assert_eq!(open(&db, |w| warned.borrow_mut().push(w.to_owned())), Ok(()));
        assert!(warned.borrow().is_empty());
        assert_eq!(open(&db, |_| panic!("no warning on a restart")), Ok(()));
        assert_eq!(hub::Hub::open(&db).head_seq().unwrap(), 0);
    }

    #[test]
    fn a_failed_migration_is_reported_not_swallowed() {
        let db = MemSql::new();
        // A column added by hand, so the recorded step can't run.
        db.exec_batch(devices::SCHEMA).unwrap();
        db.exec_batch("ALTER TABLE devices ADD COLUMN push_token TEXT").unwrap();
        let why = open(&db, |_| {}).unwrap_err();
        assert!(why.contains("push_token"), "{why}");
        let answer = unavailable(&why);
        assert_eq!(answer.code, ErrorCode::Internal);
        assert!(answer.message.contains(&why));
    }
}
