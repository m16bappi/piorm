use indexmap::IndexMap;
use sqlx::{Column, Row, TypeInfo};

use crate::error::OrmpiError;
use crate::value::OrmpiValue;

/// A single database row as an ordered map of column name → value.
#[derive(Debug, Clone)]
pub struct OrmpiRow {
    pub columns: Vec<String>,
    pub values: IndexMap<String, OrmpiValue>,
}

impl OrmpiRow {
    pub fn get(&self, column: &str) -> Option<&OrmpiValue> {
        self.values.get(column)
    }

    pub fn get_required(&self, column: &str) -> crate::Result<&OrmpiValue> {
        self.values.get(column).ok_or_else(|| OrmpiError::FieldError {
            model: "?".to_string(),
            field: column.to_string(),
            message: format!("Column '{column}' not found in result row"),
        })
    }
}

// ── SQLite ────────────────────────────────────────────────────────────────────

pub fn sqlite_row_to_ormpi(row: &sqlx::sqlite::SqliteRow) -> crate::Result<OrmpiRow> {
    let mut values = IndexMap::new();
    let mut columns = Vec::new();

    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let ord = col.ordinal();

        let value = match type_name {
            "INTEGER" | "INT" => row
                .try_get::<Option<i64>, _>(ord)
                .map(|v| v.map(OrmpiValue::Int).unwrap_or(OrmpiValue::Null))?,
            "REAL" => row
                .try_get::<Option<f64>, _>(ord)
                .map(|v| v.map(OrmpiValue::Float).unwrap_or(OrmpiValue::Null))?,
            "TEXT" => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(OrmpiValue::Text).unwrap_or(OrmpiValue::Null))?,
            "BLOB" => row
                .try_get::<Option<Vec<u8>>, _>(ord)
                .map(|v| v.map(OrmpiValue::Bytes).unwrap_or(OrmpiValue::Null))?,
            "BOOLEAN" => row
                .try_get::<Option<bool>, _>(ord)
                .map(|v| v.map(OrmpiValue::Bool).unwrap_or(OrmpiValue::Null))?,
            "NULL" => OrmpiValue::Null,
            _ => {
                // Fallback: try integer → float → text
                if let Ok(Some(i)) = row.try_get::<Option<i64>, _>(ord) {
                    OrmpiValue::Int(i)
                } else if let Ok(Some(f)) = row.try_get::<Option<f64>, _>(ord) {
                    OrmpiValue::Float(f)
                } else {
                    row.try_get::<Option<String>, _>(ord)
                        .map(|v| v.map(OrmpiValue::Text).unwrap_or(OrmpiValue::Null))
                        .unwrap_or(OrmpiValue::Null)
                }
            }
        };

        columns.push(name.clone());
        values.insert(name, value);
    }

    Ok(OrmpiRow { columns, values })
}

// ── MySQL ─────────────────────────────────────────────────────────────────────

pub fn mysql_row_to_ormpi(row: &sqlx::mysql::MySqlRow) -> crate::Result<OrmpiRow> {
    let mut values = IndexMap::new();
    let mut columns = Vec::new();

    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let ord = col.ordinal();

        let value = match type_name {
            "TINYINT(1)" | "BOOLEAN" => row
                .try_get::<Option<bool>, _>(ord)
                .map(|v| v.map(OrmpiValue::Bool).unwrap_or(OrmpiValue::Null))?,
            "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => row
                .try_get::<Option<i64>, _>(ord)
                .map(|v| v.map(OrmpiValue::Int).unwrap_or(OrmpiValue::Null))?,
            "FLOAT" | "DOUBLE" => row
                .try_get::<Option<f64>, _>(ord)
                .map(|v| v.map(OrmpiValue::Float).unwrap_or(OrmpiValue::Null))?,
            "VARCHAR" | "TEXT" | "CHAR" | "MEDIUMTEXT" | "LONGTEXT" => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(OrmpiValue::Text).unwrap_or(OrmpiValue::Null))?,
            "BLOB" | "BINARY" | "VARBINARY" => row
                .try_get::<Option<Vec<u8>>, _>(ord)
                .map(|v| v.map(OrmpiValue::Bytes).unwrap_or(OrmpiValue::Null))?,
            "JSON" => row
                .try_get::<Option<serde_json::Value>, _>(ord)
                .map(|v| v.map(OrmpiValue::Json).unwrap_or(OrmpiValue::Null))?,
            "NULL" => OrmpiValue::Null,
            _ => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(OrmpiValue::Text).unwrap_or(OrmpiValue::Null))
                .unwrap_or(OrmpiValue::Null),
        };

        columns.push(name.clone());
        values.insert(name, value);
    }

    Ok(OrmpiRow { columns, values })
}

// ── Postgres ──────────────────────────────────────────────────────────────────

pub fn postgres_row_to_ormpi(row: &sqlx::postgres::PgRow) -> crate::Result<OrmpiRow> {
    let mut values = IndexMap::new();
    let mut columns = Vec::new();

    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let ord = col.ordinal();

        let value = match type_name {
            "BOOL" => row
                .try_get::<Option<bool>, _>(ord)
                .map(|v| v.map(OrmpiValue::Bool).unwrap_or(OrmpiValue::Null))?,
            "INT2" | "INT4" => row
                .try_get::<Option<i32>, _>(ord)
                .map(|v| v.map(|i| OrmpiValue::Int(i as i64)).unwrap_or(OrmpiValue::Null))?,
            "INT8" => row
                .try_get::<Option<i64>, _>(ord)
                .map(|v| v.map(OrmpiValue::Int).unwrap_or(OrmpiValue::Null))?,
            "FLOAT4" | "FLOAT8" => row
                .try_get::<Option<f64>, _>(ord)
                .map(|v| v.map(OrmpiValue::Float).unwrap_or(OrmpiValue::Null))?,
            "TEXT" | "VARCHAR" | "NAME" | "BPCHAR" => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(OrmpiValue::Text).unwrap_or(OrmpiValue::Null))?,
            "BYTEA" => row
                .try_get::<Option<Vec<u8>>, _>(ord)
                .map(|v| v.map(OrmpiValue::Bytes).unwrap_or(OrmpiValue::Null))?,
            "JSON" | "JSONB" => row
                .try_get::<Option<serde_json::Value>, _>(ord)
                .map(|v| v.map(OrmpiValue::Json).unwrap_or(OrmpiValue::Null))?,
            "UUID" => row
                .try_get::<Option<uuid::Uuid>, _>(ord)
                .map(|v| v.map(OrmpiValue::Uuid).unwrap_or(OrmpiValue::Null))?,
            "DATE" => row
                .try_get::<Option<chrono::NaiveDate>, _>(ord)
                .map(|v| v.map(OrmpiValue::Date).unwrap_or(OrmpiValue::Null))?,
            "TIME" => row
                .try_get::<Option<chrono::NaiveTime>, _>(ord)
                .map(|v| v.map(OrmpiValue::Time).unwrap_or(OrmpiValue::Null))?,
            "TIMESTAMP" => row
                .try_get::<Option<chrono::NaiveDateTime>, _>(ord)
                .map(|v| v.map(OrmpiValue::DateTime).unwrap_or(OrmpiValue::Null))?,
            "TIMESTAMPTZ" => row
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(ord)
                .map(|v| v.map(OrmpiValue::DateTimeTz).unwrap_or(OrmpiValue::Null))?,
            _ => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(OrmpiValue::Text).unwrap_or(OrmpiValue::Null))
                .unwrap_or(OrmpiValue::Null),
        };

        columns.push(name.clone());
        values.insert(name, value);
    }

    Ok(OrmpiRow { columns, values })
}
