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

/// Execute a raw SELECT with optional positional params and return all rows as `OrmpiRow`.
pub async fn fetch_all_raw(
    pool: &OrmpiPool,
    sql: &str,
    params: &[crate::value::OrmpiValue],
) -> crate::Result<Vec<crate::row::OrmpiRow>> {
    use sea_query::Values;
    use crate::bind::{mysql_arguments, postgres_arguments, sqlite_arguments};
    use crate::row::{mysql_row_to_ormpi, postgres_row_to_ormpi, sqlite_row_to_ormpi};

    let sea_vals = Values(params.iter().map(|v| v.to_sea_value()).collect());
    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let rows = sqlx::query_with(sql, args).fetch_all(p).await?;
            rows.iter().map(sqlite_row_to_ormpi).collect()
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let rows = sqlx::query_with(sql, args).fetch_all(p).await?;
            rows.iter().map(mysql_row_to_ormpi).collect()
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let rows = sqlx::query_with(sql, args).fetch_all(p).await?;
            rows.iter().map(postgres_row_to_ormpi).collect()
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
