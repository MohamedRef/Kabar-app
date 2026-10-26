//! One-click full export (spec §7): every table of the local store as JSON in
//! one document. Tables are found through `sqlite_master`, so tables added by
//! later migrations are included without touching this file.
//!
//! Secrets never leave: the refresh and access tokens live in the macOS
//! Keychain / Android Keystore, not in SQLite, and as a second line of defence
//! any table, column or key/id that names a secret is skipped, and any value
//! that carries a Kabar token is redacted.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use rusqlite::types::ValueRef;
use serde_json::{Map, Value, json};

use super::{Result, Store};

/// Bumped when the export layout changes.
pub const EXPORT_FORMAT_VERSION: u32 = 1;

/// Words that mark a name as secret (matched case-insensitively, as a substring).
const SECRET_WORDS: &[&str] = &[
    "token",
    "secret",
    "password",
    "passwd",
    "api_key",
    "apikey",
    "credential",
    "private_key",
    "cookie",
    "bearer",
];

/// Columns whose value names the row (a settings or meta key, a sync row id).
const KEY_COLUMNS: &[&str] = &["key", "id", "row_id"];

const REDACTED: &str = "[redacted]";

#[derive(Clone, Debug, PartialEq)]
pub struct Export {
    pub document: Value,
    /// Rows written, over all tables.
    pub rows: u64,
    pub tables: usize,
}

pub fn is_secret_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SECRET_WORDS.iter().any(|w| lower.contains(w))
}

/// Holds a Kabar access token (an EdDSA JWT: `{"alg"` is `eyJhbGci`) or a
/// refresh token (`kr1.<account>.<device>.<43 chars>`).
pub fn carries_token(text: &str) -> bool {
    if text.contains("eyJhbGci") {
        return true;
    }
    text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))).any(|word| {
        let parts: Vec<&str> = word.split('.').collect();
        parts.len() == 4 && parts[0] == "kr1" && parts[3].len() >= 40
    })
}

fn cell(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) => {
            let s = String::from_utf8_lossy(t);
            if carries_token(&s) { json!(REDACTED) } else { json!(s) }
        }
        ValueRef::Blob(b) => json!({ "base64": STANDARD.encode(b) }),
    }
}

impl Store {
    /// The whole local store as one JSON document.
    pub fn export(&self, app_version: &str, now_ms: u64) -> Result<Export> {
        let schema_version: u32 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let names: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )?;
            stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };

        let mut tables = Map::new();
        let mut skipped = Vec::new();
        let mut total = 0u64;
        for name in names.into_iter().filter(|n| !is_secret_name(n)) {
            // `name` comes from sqlite_master; quote it as an identifier anyway.
            let mut stmt = self.conn.prepare(&format!("SELECT * FROM \"{}\"", name.replace('"', "\"\"")))?;
            let columns: Vec<String> = stmt.column_names().map(String::from).collect();
            let keep: Vec<bool> = columns.iter().map(|c| !is_secret_name(c)).collect();
            let key_cols: Vec<usize> = columns
                .iter()
                .enumerate()
                .filter(|(_, c)| KEY_COLUMNS.contains(&c.as_str()))
                .map(|(i, _)| i)
                .collect();

            let mut rows = Vec::new();
            let mut cursor = stmt.query([])?;
            while let Some(row) = cursor.next()? {
                let secret_key = key_cols.iter().find_map(|&i| match row.get_ref(i) {
                    Ok(ValueRef::Text(t)) if is_secret_name(&String::from_utf8_lossy(t)) => {
                        Some(String::from_utf8_lossy(t).into_owned())
                    }
                    _ => None,
                });
                if let Some(key) = secret_key {
                    skipped.push(json!({ "table": name, "key": key }));
                    continue;
                }
                let mut obj = Map::new();
                for (i, col) in columns.iter().enumerate() {
                    if keep[i] {
                        obj.insert(col.clone(), cell(row.get_ref(i)?));
                    }
                }
                rows.push(Value::Object(obj));
            }
            total += rows.len() as u64;
            let kept: Vec<&String> = columns.iter().zip(&keep).filter(|(_, k)| **k).map(|(c, _)| c).collect();
            tables.insert(name, json!({ "columns": kept, "rows": rows }));
        }

        let count = tables.len();
        let document = json!({
            "format": "kabar-export",
            "format_version": EXPORT_FORMAT_VERSION,
            "app_version": app_version,
            "schema_version": schema_version,
            "device_id": self.device.0,
            "exported_at_ms": now_ms,
            "row_count": total,
            "note": "Everything Kabar keeps on this device. Sign-in tokens live in the system keychain and are never exported.",
            "skipped_secret_rows": skipped,
            "tables": tables,
        });
        Ok(Export { document, rows: total, tables: count })
    }
}

#[cfg(test)]
mod tests {
    use kabar_core::settings::{Settings, Theme};
    use kabar_protocol::sync::OpKind;

    use super::*;

    const JWT: &str = "eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhIn0.c2ln";
    const REFRESH: &str = "kr1.acct_1.dev-1.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    #[test]
    fn exports_every_table_and_no_secrets() {
        let mut store = Store::open_in_memory().unwrap();
        let old = store.settings().unwrap();
        store.save_settings(&old, &Settings { theme: Theme::Batik, welcomed: true, ..old.clone() }).unwrap();
        // "Saya mau pesan nasi goreng" (SPEC §24) as a saved word.
        store
            .write_synced(
                "word_status",
                "id:pesan",
                OpKind::Upsert,
                Some(json!({ "lexeme": "pesan", "lang": "id", "status": "learning" })),
            )
            .unwrap();
        store.set_meta("last_sync_ms", Some("1793011856000")).unwrap();
        // Regressions the exporter must survive: a token stored where it never should be.
        store.set_meta("refresh_token", Some(REFRESH)).unwrap();
        store.set_meta("note", Some(&format!("Bearer {JWT}"))).unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TABLE future_feature (id TEXT PRIMARY KEY, body TEXT, api_key TEXT);
             INSERT INTO future_feature VALUES ('a', 'Arek-arek iku wis mangan', 'sk-ant-xyz');
             CREATE TABLE oauth_tokens (id TEXT, value TEXT);
             INSERT INTO oauth_tokens VALUES ('x', 'ya29.abc');",
            )
            .unwrap();

        let export = store.export("0.1.0", 1_793_011_856_000).unwrap();
        let doc = &export.document;
        let text = serde_json::to_string(doc).unwrap();

        assert_eq!(doc["format"], "kabar-export");
        assert_eq!(doc["device_id"], store.device_id().0);
        assert_eq!(doc["app_version"], "0.1.0");
        assert!(doc["schema_version"].as_u64().unwrap() >= 6);
        let tables = doc["tables"].as_object().unwrap();
        for t in ["meta", "settings", "word_status", "sync_rows", "outbox", "cards", "future_feature"] {
            assert!(tables.contains_key(t), "{t} missing");
        }
        assert!(!tables.contains_key("oauth_tokens"));
        assert!(!tables.keys().any(|k| k.starts_with("sqlite_")));
        let settings_rows = tables["settings"]["rows"].as_array().unwrap();
        assert!(settings_rows.iter().any(|r| r["key"] == "theme" && r["value"] == "\"batik\""));
        assert!(text.contains("Arek-arek iku wis mangan"));
        assert!(tables["word_status"]["rows"].as_array().unwrap().len() == 1);

        // Secrets: none of them, in any form.
        for secret in [REFRESH, JWT, "sk-ant-xyz", "ya29.abc", "api_key"] {
            assert!(!text.contains(secret), "{secret} leaked");
        }
        let in_tables = serde_json::to_string(&doc["tables"]).unwrap();
        assert!(!in_tables.contains("refresh_token"));
        assert!(doc["skipped_secret_rows"].as_array().unwrap().iter().any(|s| s["key"] == "refresh_token"));
        assert!(text.contains(REDACTED));

        let counted: u64 = tables.values().map(|t| t["rows"].as_array().unwrap().len() as u64).sum();
        assert_eq!(export.rows, counted);
        assert_eq!(doc["row_count"], counted);
        assert_eq!(export.tables, tables.len());
    }

    #[test]
    fn token_detection() {
        assert!(carries_token(JWT));
        assert!(carries_token(&format!("{{\"t\":\"{REFRESH}\"}}")));
        assert!(!carries_token("Saya mau pesan nasi goreng"));
        assert!(!carries_token("kr1.short"));
        assert!(is_secret_name("access_token") && is_secret_name("GEMINI_API_KEY"));
        assert!(!is_secret_name("sessions") && !is_secret_name("word_status") && !is_secret_name("key"));
    }
}
