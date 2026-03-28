use sqlx::{Column, Row};

use crate::pool::OrmpiPool;

/// Execute a raw SQL statement that doesn't return rows (DDL / DML).
pub async fn execute_sql(pool: &OrmpiPool, sql: &str) -> crate::Result<u64> {
    match pool {
        OrmpiPool::Sqlite(p) => Ok(sqlx::query(sql).execute(p).await?.rows_affected()),
        OrmpiPool::Mysql(p) => Ok(sqlx::query(sql).execute(p).await?.rows_affected()),
        OrmpiPool::Postgres(p) => Ok(sqlx::query(sql).execute(p).await?.rows_affected()),
    }
}

/// Execute a raw SQL statement and return the first row as (column_name, json_value) pairs.
pub async fn fetch_one_raw(
    pool: &OrmpiPool,
    sql: &str,
) -> crate::Result<Vec<(String, serde_json::Value)>> {
    match pool {
        OrmpiPool::Sqlite(p) => {
            let row = sqlx::query(sql).fetch_one(p).await?;
            Ok(row_to_json_sqlite(&row))
        }
        OrmpiPool::Mysql(p) => {
            let row = sqlx::query(sql).fetch_one(p).await?;
            Ok(row_to_json_mysql(&row))
        }
        OrmpiPool::Postgres(p) => {
            let row = sqlx::query(sql).fetch_one(p).await?;
            Ok(row_to_json_postgres(&row))
        }
    }
}

pub fn fetch_row_sqlite(row: &sqlx::sqlite::SqliteRow) -> Vec<(String, serde_json::Value)> {
    row_to_json_sqlite(row)
}

pub fn fetch_row_mysql(row: &sqlx::mysql::MySqlRow) -> Vec<(String, serde_json::Value)> {
    row_to_json_mysql(row)
}

pub fn fetch_row_postgres(row: &sqlx::postgres::PgRow) -> Vec<(String, serde_json::Value)> {
    row_to_json_postgres(row)
}

fn row_to_json_sqlite(row: &sqlx::sqlite::SqliteRow) -> Vec<(String, serde_json::Value)> {
    row.columns()
        .iter()
        .map(|col| {
            let name = col.name().to_string();
            let val = extract_sqlite_value(row, col.name());
            (name, val)
        })
        .collect()
}

fn extract_sqlite_value(row: &sqlx::sqlite::SqliteRow, col: &str) -> serde_json::Value {
    if let Ok(v) = row.try_get::<i64, _>(col) {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = row.try_get::<f64, _>(col) {
        if let Some(n) = serde_json::Number::from_f64(v) {
            return serde_json::Value::Number(n);
        }
    }
    if let Ok(v) = row.try_get::<String, _>(col) {
        return serde_json::Value::String(v);
    }
    if let Ok(v) = row.try_get::<bool, _>(col) {
        return serde_json::Value::Bool(v);
    }
    serde_json::Value::Null
}

fn row_to_json_mysql(row: &sqlx::mysql::MySqlRow) -> Vec<(String, serde_json::Value)> {
    row.columns()
        .iter()
        .map(|col| {
            let name = col.name().to_string();
            let val = extract_mysql_value(row, col.name());
            (name, val)
        })
        .collect()
}

fn extract_mysql_value(row: &sqlx::mysql::MySqlRow, col: &str) -> serde_json::Value {
    if let Ok(v) = row.try_get::<i64, _>(col) {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = row.try_get::<f64, _>(col) {
        if let Some(n) = serde_json::Number::from_f64(v) {
            return serde_json::Value::Number(n);
        }
    }
    if let Ok(v) = row.try_get::<String, _>(col) {
        return serde_json::Value::String(v);
    }
    if let Ok(v) = row.try_get::<bool, _>(col) {
        return serde_json::Value::Bool(v);
    }
    serde_json::Value::Null
}

fn row_to_json_postgres(row: &sqlx::postgres::PgRow) -> Vec<(String, serde_json::Value)> {
    row.columns()
        .iter()
        .map(|col| {
            let name = col.name().to_string();
            let val = extract_postgres_value(row, col.name());
            (name, val)
        })
        .collect()
}

fn extract_postgres_value(row: &sqlx::postgres::PgRow, col: &str) -> serde_json::Value {
    if let Ok(v) = row.try_get::<i64, _>(col) {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = row.try_get::<i32, _>(col) {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = row.try_get::<f64, _>(col) {
        if let Some(n) = serde_json::Number::from_f64(v) {
            return serde_json::Value::Number(n);
        }
    }
    if let Ok(v) = row.try_get::<String, _>(col) {
        return serde_json::Value::String(v);
    }
    if let Ok(v) = row.try_get::<bool, _>(col) {
        return serde_json::Value::Bool(v);
    }
    serde_json::Value::Null
}
