use indexmap::IndexMap;
use sqlx::{Column, Row, TypeInfo};

use crate::error::PiormError;
use crate::value::PiormValue;

/// A single database row as an ordered map of column name → value.
#[derive(Debug, Clone)]
pub struct PiormRow {
    pub columns: Vec<String>,
    pub values: IndexMap<String, PiormValue>,
}

impl PiormRow {
    pub fn get(&self, column: &str) -> Option<&PiormValue> {
        self.values.get(column)
    }

    pub fn get_required(&self, column: &str) -> crate::Result<&PiormValue> {
        self.values.get(column).ok_or_else(|| PiormError::FieldError {
            model: "?".to_string(),
            field: column.to_string(),
            message: format!("Column '{column}' not found in result row"),
        })
    }
}

// ── SQLite ────────────────────────────────────────────────────────────────────

pub fn sqlite_row_to_piorm(row: &sqlx::sqlite::SqliteRow) -> crate::Result<PiormRow> {
    let mut values = IndexMap::new();
    let mut columns = Vec::new();

    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let ord = col.ordinal();

        let value = match type_name {
            "INTEGER" | "INT" => row
                .try_get::<Option<i64>, _>(ord)
                .map(|v| v.map(PiormValue::Int).unwrap_or(PiormValue::Null))?,
            "REAL" => row
                .try_get::<Option<f64>, _>(ord)
                .map(|v| v.map(PiormValue::Float).unwrap_or(PiormValue::Null))?,
            "TEXT" => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(PiormValue::Text).unwrap_or(PiormValue::Null))?,
            "BLOB" => row
                .try_get::<Option<Vec<u8>>, _>(ord)
                .map(|v| v.map(PiormValue::Bytes).unwrap_or(PiormValue::Null))?,
            "BOOLEAN" => row
                .try_get::<Option<bool>, _>(ord)
                .map(|v| v.map(PiormValue::Bool).unwrap_or(PiormValue::Null))?,
            "NULL" => PiormValue::Null,
            _ => {
                // Fallback: try integer → float → text
                if let Ok(Some(i)) = row.try_get::<Option<i64>, _>(ord) {
                    PiormValue::Int(i)
                } else if let Ok(Some(f)) = row.try_get::<Option<f64>, _>(ord) {
                    PiormValue::Float(f)
                } else {
                    row.try_get::<Option<String>, _>(ord)
                        .map(|v| v.map(PiormValue::Text).unwrap_or(PiormValue::Null))
                        .unwrap_or(PiormValue::Null)
                }
            }
        };

        columns.push(name.clone());
        values.insert(name, value);
    }

    Ok(PiormRow { columns, values })
}

// ── MySQL ─────────────────────────────────────────────────────────────────────

pub fn mysql_row_to_piorm(row: &sqlx::mysql::MySqlRow) -> crate::Result<PiormRow> {
    let mut values = IndexMap::new();
    let mut columns = Vec::new();

    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let ord = col.ordinal();

        let value = match type_name {
            "TINYINT(1)" | "BOOLEAN" => row
                .try_get::<Option<bool>, _>(ord)
                .map(|v| v.map(PiormValue::Bool).unwrap_or(PiormValue::Null))?,
            "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => row
                .try_get::<Option<i64>, _>(ord)
                .map(|v| v.map(PiormValue::Int).unwrap_or(PiormValue::Null))?,
            "FLOAT" | "DOUBLE" => row
                .try_get::<Option<f64>, _>(ord)
                .map(|v| v.map(PiormValue::Float).unwrap_or(PiormValue::Null))?,
            "VARCHAR" | "TEXT" | "CHAR" | "MEDIUMTEXT" | "LONGTEXT" => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(PiormValue::Text).unwrap_or(PiormValue::Null))?,
            "BLOB" | "BINARY" | "VARBINARY" => row
                .try_get::<Option<Vec<u8>>, _>(ord)
                .map(|v| v.map(PiormValue::Bytes).unwrap_or(PiormValue::Null))?,
            "JSON" => row
                .try_get::<Option<serde_json::Value>, _>(ord)
                .map(|v| v.map(PiormValue::Json).unwrap_or(PiormValue::Null))?,
            "NULL" => PiormValue::Null,
            _ => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(PiormValue::Text).unwrap_or(PiormValue::Null))
                .unwrap_or(PiormValue::Null),
        };

        columns.push(name.clone());
        values.insert(name, value);
    }

    Ok(PiormRow { columns, values })
}

// ── Postgres ──────────────────────────────────────────────────────────────────

pub fn postgres_row_to_piorm(row: &sqlx::postgres::PgRow) -> crate::Result<PiormRow> {
    let mut values = IndexMap::new();
    let mut columns = Vec::new();

    for col in row.columns() {
        let name = col.name().to_string();
        let type_name = col.type_info().name();
        let ord = col.ordinal();

        let value = match type_name {
            "BOOL" => row
                .try_get::<Option<bool>, _>(ord)
                .map(|v| v.map(PiormValue::Bool).unwrap_or(PiormValue::Null))?,
            "INT2" | "INT4" => row
                .try_get::<Option<i32>, _>(ord)
                .map(|v| v.map(|i| PiormValue::Int(i as i64)).unwrap_or(PiormValue::Null))?,
            "INT8" => row
                .try_get::<Option<i64>, _>(ord)
                .map(|v| v.map(PiormValue::Int).unwrap_or(PiormValue::Null))?,
            "FLOAT4" | "FLOAT8" => row
                .try_get::<Option<f64>, _>(ord)
                .map(|v| v.map(PiormValue::Float).unwrap_or(PiormValue::Null))?,
            "TEXT" | "VARCHAR" | "NAME" | "BPCHAR" => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(PiormValue::Text).unwrap_or(PiormValue::Null))?,
            "BYTEA" => row
                .try_get::<Option<Vec<u8>>, _>(ord)
                .map(|v| v.map(PiormValue::Bytes).unwrap_or(PiormValue::Null))?,
            "JSON" | "JSONB" => row
                .try_get::<Option<serde_json::Value>, _>(ord)
                .map(|v| v.map(PiormValue::Json).unwrap_or(PiormValue::Null))?,
            "UUID" => row
                .try_get::<Option<uuid::Uuid>, _>(ord)
                .map(|v| v.map(PiormValue::Uuid).unwrap_or(PiormValue::Null))?,
            "DATE" => row
                .try_get::<Option<chrono::NaiveDate>, _>(ord)
                .map(|v| v.map(PiormValue::Date).unwrap_or(PiormValue::Null))?,
            "TIME" => row
                .try_get::<Option<chrono::NaiveTime>, _>(ord)
                .map(|v| v.map(PiormValue::Time).unwrap_or(PiormValue::Null))?,
            "TIMESTAMP" => row
                .try_get::<Option<chrono::NaiveDateTime>, _>(ord)
                .map(|v| v.map(PiormValue::DateTime).unwrap_or(PiormValue::Null))?,
            "TIMESTAMPTZ" => row
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(ord)
                .map(|v| v.map(PiormValue::DateTimeTz).unwrap_or(PiormValue::Null))?,
            _ => row
                .try_get::<Option<String>, _>(ord)
                .map(|v| v.map(PiormValue::Text).unwrap_or(PiormValue::Null))
                .unwrap_or(PiormValue::Null),
        };

        columns.push(name.clone());
        values.insert(name, value);
    }

    Ok(PiormRow { columns, values })
}
