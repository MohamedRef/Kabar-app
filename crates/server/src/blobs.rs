//! Images for the vision tasks (Gambar!, handwriting, stroke checks,
//! screenshots). A device uploads a PNG or JPEG once, named by its SHA-256,
//! then refers to it from `EngineCall::image_sha256`. Blobs live in the
//! account's Durable Object (no R2, which needs a payment method on file)
//! and are dropped after a week: they are only needed for the calls that
//! follow the upload.

use base64::Engine as _;

use crate::sql::{Sql, SqlError, Value};
use crate::tokens::sha256_hex;

pub const MAX_BLOB_BYTES: usize = 1024 * 1024;
pub const KEEP_MS: u64 = 7 * 24 * 60 * 60 * 1000;

pub const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS blobs (
    sha256     TEXT PRIMARY KEY,
    mime       TEXT NOT NULL,
    data_b64   TEXT NOT NULL,
    created_ms INTEGER NOT NULL
)";

/// An image as the engines take it.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub mime: String,
    pub base64: String,
}

impl Image {
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime, self.base64)
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Invalid {
    #[error("the name must be the lowercase SHA-256 hex of the body")]
    BadName,
    #[error("the body's SHA-256 doesn't match the name")]
    Mismatch,
    #[error("only PNG and JPEG images, up to 1 MB")]
    NotAnImage,
}

/// Checks an upload and returns its media type (from the bytes, not the
/// client's header).
pub fn validate(sha256: &str, bytes: &[u8]) -> Result<&'static str, Invalid> {
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(Invalid::BadName);
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else {
        return Err(Invalid::NotAnImage);
    };
    if bytes.len() > MAX_BLOB_BYTES {
        return Err(Invalid::NotAnImage);
    }
    if sha256_hex(bytes) != sha256 {
        return Err(Invalid::Mismatch);
    }
    Ok(mime)
}

pub fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Stores a validated blob (re-uploading refreshes its week) and drops
/// expired ones.
pub fn put(db: &impl Sql, sha256: &str, image: &Image, now: u64) -> Result<(), SqlError> {
    db.exec(
        "INSERT INTO blobs (sha256, mime, data_b64, created_ms) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(sha256) DO UPDATE SET created_ms = excluded.created_ms",
        &[sha256.into(), image.mime.as_str().into(), image.base64.as_str().into(), now.into()],
    )?;
    db.exec("DELETE FROM blobs WHERE created_ms < ?1", &[now.saturating_sub(KEEP_MS).into()])?;
    Ok(())
}

pub fn get(db: &impl Sql, sha256: &str, now: u64) -> Result<Option<Image>, SqlError> {
    let rows = db.exec(
        "SELECT mime, data_b64 FROM blobs WHERE sha256 = ?1 AND created_ms >= ?2",
        &[sha256.into(), now.saturating_sub(KEEP_MS).into()],
    )?;
    Ok(rows.into_iter().next().and_then(|row| {
        let mut row = row.into_iter();
        let mime = row.next().and_then(Value::into_string)?;
        let base64 = row.next().and_then(Value::into_string)?;
        Some(Image { mime, base64 })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::testing::MemSql;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn uploads_are_checked_against_their_name() {
        let sha = sha256_hex(PNG);
        assert_eq!(validate(&sha, PNG), Ok("image/png"));
        assert_eq!(validate(&sha.to_uppercase(), PNG), Err(Invalid::BadName));
        assert_eq!(validate(&sha256_hex(b"other"), PNG), Err(Invalid::Mismatch));
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2];
        assert_eq!(validate(&sha256_hex(&jpeg), &jpeg), Ok("image/jpeg"));
        let svg = b"<svg onload=alert(1)>";
        assert_eq!(validate(&sha256_hex(svg), svg), Err(Invalid::NotAnImage));
        let mut big = PNG.to_vec();
        big.resize(MAX_BLOB_BYTES + 1, 0);
        assert_eq!(validate(&sha256_hex(&big), &big), Err(Invalid::NotAnImage));
    }

    #[test]
    fn blobs_last_a_week() {
        let db = MemSql::new();
        db.exec_batch(SCHEMA).unwrap();
        let sha = sha256_hex(PNG);
        let image = Image { mime: "image/png".into(), base64: encode(PNG) };
        put(&db, &sha, &image, 1_000).unwrap();
        assert_eq!(get(&db, &sha, 1_000 + KEEP_MS).unwrap(), Some(image.clone()));
        assert_eq!(get(&db, &sha, 1_001 + KEEP_MS).unwrap(), None);
        assert!(image.data_url().starts_with("data:image/png;base64,iVBOR"));
        // Another upload later sweeps the old one away.
        let other = sha256_hex(b"x");
        put(&db, &other, &image, 2_000 + KEEP_MS).unwrap();
        assert_eq!(db.exec("SELECT COUNT(*) FROM blobs", &[]).unwrap()[0][0], Value::Int(1));
    }
}
