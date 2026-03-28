//! Helpers that convert sea-query `Values` into sqlx `Arguments` for each backend.
//!
//! sea-query is compiled with `with-chrono`, `with-uuid`, and `with-json` enabled in this
//! workspace, so all Value variants are always present — no feature-gated `#[cfg]` needed here.

use sea_query::{Value, Values};
use sqlx::Arguments;

// ── SQLite ────────────────────────────────────────────────────────────────────

pub fn sqlite_arguments(values: Values) -> sqlx::sqlite::SqliteArguments<'static> {
    let mut args = sqlx::sqlite::SqliteArguments::default();
    for v in values {
        bind_sqlite(&mut args, v);
    }
    args
}

fn bind_sqlite(args: &mut sqlx::sqlite::SqliteArguments<'static>, value: Value) {
    match value {
        Value::Bool(v) => { let _ = args.add(v); }
        Value::TinyInt(v) => { let _ = args.add(v); }
        Value::SmallInt(v) => { let _ = args.add(v); }
        Value::Int(v) => { let _ = args.add(v); }
        Value::BigInt(v) => { let _ = args.add(v); }
        Value::TinyUnsigned(v) => { let _ = args.add(v); }
        Value::SmallUnsigned(v) => { let _ = args.add(v); }
        Value::Unsigned(v) => { let _ = args.add(v); }
        Value::BigUnsigned(v) => {
            let _ = args.add(v.map(|u| i64::try_from(u).unwrap_or(i64::MAX)));
        }
        Value::Float(v) => { let _ = args.add(v); }
        Value::Double(v) => { let _ = args.add(v); }
        Value::String(v) => { let _ = args.add(v); }
        Value::Char(v) => { let _ = args.add(v.map(|c| c.to_string())); }
        Value::Bytes(v) => { let _ = args.add(v); }
        Value::ChronoDate(v) => { let _ = args.add(v); }
        Value::ChronoTime(v) => { let _ = args.add(v); }
        Value::ChronoDateTime(v) => { let _ = args.add(v); }
        Value::ChronoDateTimeUtc(v) => { let _ = args.add(v); }
        Value::ChronoDateTimeLocal(v) => { let _ = args.add(v); }
        Value::ChronoDateTimeWithTimeZone(v) => { let _ = args.add(v); }
        Value::Uuid(v) => { let _ = args.add(v); }
        Value::Json(v) => { let _ = args.add(v.map(|j| *j)); }
    }
}

// ── MySQL ─────────────────────────────────────────────────────────────────────

pub fn mysql_arguments(values: Values) -> sqlx::mysql::MySqlArguments {
    let mut args = sqlx::mysql::MySqlArguments::default();
    for v in values {
        bind_mysql(&mut args, v);
    }
    args
}

fn bind_mysql(args: &mut sqlx::mysql::MySqlArguments, value: Value) {
    match value {
        Value::Bool(v) => { let _ = args.add(v); }
        Value::TinyInt(v) => { let _ = args.add(v); }
        Value::SmallInt(v) => { let _ = args.add(v); }
        Value::Int(v) => { let _ = args.add(v); }
        Value::BigInt(v) => { let _ = args.add(v); }
        Value::TinyUnsigned(v) => { let _ = args.add(v); }
        Value::SmallUnsigned(v) => { let _ = args.add(v); }
        Value::Unsigned(v) => { let _ = args.add(v); }
        Value::BigUnsigned(v) => { let _ = args.add(v); }
        Value::Float(v) => { let _ = args.add(v); }
        Value::Double(v) => { let _ = args.add(v); }
        Value::String(v) => { let _ = args.add(v); }
        Value::Char(v) => { let _ = args.add(v.map(|c| c.to_string())); }
        Value::Bytes(v) => { let _ = args.add(v); }
        Value::ChronoDate(v) => { let _ = args.add(v); }
        Value::ChronoTime(v) => { let _ = args.add(v); }
        Value::ChronoDateTime(v) => { let _ = args.add(v); }
        Value::ChronoDateTimeUtc(v) => { let _ = args.add(v); }
        Value::ChronoDateTimeLocal(v) => { let _ = args.add(v); }
        // MySQL has no native tz-aware datetime type; store as UTC string
        Value::ChronoDateTimeWithTimeZone(v) => {
            let _ = args.add(v.map(|dt| dt.to_utc().to_string()));
        }
        Value::Uuid(v) => { let _ = args.add(v); }
        Value::Json(v) => { let _ = args.add(v.map(|j| *j)); }
    }
}

// ── Postgres ──────────────────────────────────────────────────────────────────

pub fn postgres_arguments(values: Values) -> sqlx::postgres::PgArguments {
    let mut args = sqlx::postgres::PgArguments::default();
    for v in values {
        bind_postgres(&mut args, v);
    }
    args
}

fn bind_postgres(args: &mut sqlx::postgres::PgArguments, value: Value) {
    match value {
        Value::Bool(v) => { let _ = args.add(v); }
        Value::TinyInt(v) => { let _ = args.add(v); }
        Value::SmallInt(v) => { let _ = args.add(v); }
        Value::Int(v) => { let _ = args.add(v); }
        Value::BigInt(v) => { let _ = args.add(v); }
        Value::TinyUnsigned(v) => { let _ = args.add(v.map(|u| u as i16)); }
        Value::SmallUnsigned(v) => { let _ = args.add(v.map(|u| u as i32)); }
        Value::Unsigned(v) => { let _ = args.add(v.map(|u| u as i64)); }
        Value::BigUnsigned(v) => {
            let _ = args.add(v.map(|u| i64::try_from(u).unwrap_or(i64::MAX)));
        }
        Value::Float(v) => { let _ = args.add(v); }
        Value::Double(v) => { let _ = args.add(v); }
        Value::String(v) => { let _ = args.add(v); }
        Value::Char(v) => { let _ = args.add(v.map(|c| c.to_string())); }
        Value::Bytes(v) => { let _ = args.add(v); }
        Value::ChronoDate(v) => { let _ = args.add(v); }
        Value::ChronoTime(v) => { let _ = args.add(v); }
        Value::ChronoDateTime(v) => { let _ = args.add(v); }
        Value::ChronoDateTimeUtc(v) => { let _ = args.add(v); }
        Value::ChronoDateTimeLocal(v) => { let _ = args.add(v); }
        Value::ChronoDateTimeWithTimeZone(v) => { let _ = args.add(v); }
        Value::Uuid(v) => { let _ = args.add(v); }
        Value::Json(v) => { let _ = args.add(v.map(|j| sqlx::types::Json(*j))); }
    }
}
