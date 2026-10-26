//! The slice of SQLite that the Durable Object exposes (`ctx.storage.sql`),
//! as a trait so the hub and device registry run against real SQLite
//! (rusqlite) in tests.
//!
//! Durable Object SQL runs each event's statements atomically as long as no
//! `await` happens in between, so callers keep their SQL synchronous.

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            // JS numbers come back as floats when they are not safe integers.
            Value::Real(f) if f.fract() == 0.0 => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.as_i64().and_then(|i| u64::try_from(i).ok())
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn into_string(self) -> Option<String> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Self {
        // Timestamps and sequence numbers stay far below 2^53.
        Value::Int(v as i64)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Int(v.into())
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        v.map_or(Value::Null, Into::into)
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("sql: {0}")]
pub struct SqlError(pub String);

pub type Row = Vec<Value>;

pub trait Sql {
    /// Runs one statement with positional `?` parameters and returns all rows.
    fn exec(&self, sql: &str, params: &[Value]) -> Result<Vec<Row>, SqlError>;

    /// Runs a script of statements without parameters (schema setup).
    fn exec_batch(&self, sql: &str) -> Result<(), SqlError> {
        for stmt in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            self.exec(stmt, &[])?;
        }
        Ok(())
    }
}

/// Reads column `i` of a row, or fails with a message naming the column.
pub(crate) fn col<'a>(row: &'a Row, i: usize, name: &str) -> Result<&'a Value, SqlError> {
    row.get(i).ok_or_else(|| SqlError(format!("missing column {name}")))
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// In-memory SQLite standing in for Durable Object storage.
    pub struct MemSql(pub rusqlite::Connection);

    impl MemSql {
        pub fn new() -> Self {
            Self(rusqlite::Connection::open_in_memory().unwrap())
        }
    }

    impl Sql for MemSql {
        fn exec(&self, sql: &str, params: &[Value]) -> Result<Vec<Row>, SqlError> {
            use rusqlite::types::{Value as V, ValueRef};
            let err = |e: rusqlite::Error| SqlError(e.to_string());
            let mut stmt = self.0.prepare(sql).map_err(err)?;
            let params: Vec<V> = params
                .iter()
                .map(|p| match p {
                    Value::Null => V::Null,
                    Value::Int(i) => V::Integer(*i),
                    Value::Real(f) => V::Real(*f),
                    Value::Text(s) => V::Text(s.clone()),
                })
                .collect();
            let n = stmt.column_count();
            let mut rows = stmt.query(rusqlite::params_from_iter(params)).map_err(err)?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().map_err(err)? {
                let mut r = Vec::with_capacity(n);
                for i in 0..n {
                    r.push(match row.get_ref(i).map_err(err)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(i) => Value::Int(i),
                        ValueRef::Real(f) => Value::Real(f),
                        ValueRef::Text(t) => Value::Text(String::from_utf8_lossy(t).into_owned()),
                        ValueRef::Blob(_) => return Err(SqlError("blobs are not used".into())),
                    });
                }
                out.push(r);
            }
            Ok(out)
        }
    }
}
