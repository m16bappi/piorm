use sea_query::Values;
use sqlx::{Column, Row};

use crate::pool::OrmpiPool;
use crate::value::OrmpiValue;

// ── Shared waterfall macro ────────────────────────────────────────────────────

/// Try i64 → f64 → String → bool → Null.
/// Postgres callers prepend an i32 check before invoking this.
macro_rules! extract_common_value {
    ($row:expr, $col:expr) => {{
        if let Ok(v) = $row.try_get::<i64, _>($col) {
            serde_json::Value::Number(v.into())
        } else if let Ok(v) = $row.try_get::<f64, _>($col) {
            serde_json::Number::from_f64(v)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = $row.try_get::<String, _>($col) {
            serde_json::Value::String(v)
        } else if let Ok(v) = $row.try_get::<bool, _>($col) {
            serde_json::Value::Bool(v)
        } else {
            serde_json::Value::Null
        }
    }};
}

// ── ToJsonPairs trait ─────────────────────────────────────────────────────────

/// Converts a database row into an ordered list of (column_name, JSON value) pairs.
trait ToJsonPairs {
    fn to_json_pairs(&self) -> Vec<(String, serde_json::Value)>;
}

impl ToJsonPairs for sqlx::sqlite::SqliteRow {
    fn to_json_pairs(&self) -> Vec<(String, serde_json::Value)> {
        self.columns()
            .iter()
            .map(|col| {
                let name = col.name().to_string();
                let val = extract_common_value!(self, col.name());
                (name, val)
            })
            .collect()
    }
}

impl ToJsonPairs for sqlx::mysql::MySqlRow {
    fn to_json_pairs(&self) -> Vec<(String, serde_json::Value)> {
        self.columns()
            .iter()
            .map(|col| {
                let name = col.name().to_string();
                let val = extract_common_value!(self, col.name());
                (name, val)
            })
            .collect()
    }
}

impl ToJsonPairs for sqlx::postgres::PgRow {
    fn to_json_pairs(&self) -> Vec<(String, serde_json::Value)> {
        self.columns()
            .iter()
            .map(|col| {
                let name = col.name().to_string();
                // Postgres-specific: try i32 first (INT4 columns map to i32, not i64).
                let val = if let Ok(v) = self.try_get::<i32, _>(col.name()) {
                    serde_json::Value::Number(v.into())
                } else {
                    extract_common_value!(self, col.name())
                };
                (name, val)
            })
            .collect()
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Execute a raw SQL statement that doesn't return rows (DDL / DML).
pub async fn execute_sql(pool: &OrmpiPool, sql: &str) -> crate::Result<u64> {
    pool.execute(sql, Values(vec![])).await
}

/// Execute a raw SELECT with optional positional params and return all rows as `OrmpiRow`.
pub async fn fetch_all_raw(
    pool: &OrmpiPool,
    sql: &str,
    params: &[OrmpiValue],
) -> crate::Result<Vec<crate::row::OrmpiRow>> {
    let values = Values(params.iter().map(|v| v.to_sea_value()).collect());
    pool.fetch_all(sql, values).await
}

/// Execute a raw SQL statement and return the first row as (column_name, json_value) pairs.
pub async fn fetch_one_raw(
    pool: &OrmpiPool,
    sql: &str,
) -> crate::Result<Vec<(String, serde_json::Value)>> {
    match pool {
        OrmpiPool::Sqlite(p)   => Ok(sqlx::query(sql).fetch_one(p).await?.to_json_pairs()),
        OrmpiPool::Mysql(p)    => Ok(sqlx::query(sql).fetch_one(p).await?.to_json_pairs()),
        OrmpiPool::Postgres(p) => Ok(sqlx::query(sql).fetch_one(p).await?.to_json_pairs()),
    }
}
