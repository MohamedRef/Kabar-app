//! [`Sql`] over Durable Object SQLite storage.

use worker::{SqlStorage, SqlStorageValue};

use crate::sql::{Row, Sql, SqlError, Value};

pub struct DoSql(pub SqlStorage);

impl Sql for DoSql {
    fn exec(&self, sql: &str, params: &[Value]) -> Result<Vec<Row>, SqlError> {
        let bindings: Vec<SqlStorageValue> = params
            .iter()
            .map(|p| match p {
                Value::Null => SqlStorageValue::Null,
                Value::Int(i) => SqlStorageValue::Integer(*i),
                Value::Real(f) => SqlStorageValue::Float(*f),
                Value::Text(s) => SqlStorageValue::String(s.clone()),
            })
            .collect();
        let cursor = self.0.exec(sql, bindings).map_err(|e| SqlError(e.to_string()))?;
        cursor
            .raw()
            .map(|row| {
                let row = row.map_err(|e| SqlError(e.to_string()))?;
                row.into_iter()
                    .map(|v| match v {
                        SqlStorageValue::Null => Ok(Value::Null),
                        SqlStorageValue::Boolean(b) => Ok(Value::Int(b.into())),
                        SqlStorageValue::Integer(i) => Ok(Value::Int(i)),
                        SqlStorageValue::Float(f) => Ok(Value::Real(f)),
                        SqlStorageValue::String(s) => Ok(Value::Text(s)),
                        SqlStorageValue::Blob(_) => Err(SqlError("blobs are not used".into())),
                    })
                    .collect()
            })
            .collect()
    }
}
