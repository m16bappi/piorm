//! Helpers that convert sea-query `Values` into sqlx `Arguments` for each backend.
//!
//! sea-query is compiled with `with-chrono`, `with-uuid`, and `with-json` enabled in this
//! workspace, so all Value variants are always present — no feature-gated `#[cfg]` needed here.

use sea_query::{Value, Values};
use sqlx::Arguments;

// ── Shared generation macro ───────────────────────────────────────────────────
//
// Each backend differs only in how it handles unsigned integers, tz-aware
// datetimes, and JSON. Everything else is identical.
//
// Parameters:
//   $fn_name          — name of the private `bind_*` helper
//   $args_ty          — sqlx arguments type, e.g. `sqlx::sqlite::SqliteArguments<'static>`
//   $tiny_unsigned    — expression converting Option<u8>  → bindable value
//   $small_unsigned   — expression converting Option<u16> → bindable value
//   $unsigned         — expression converting Option<u32> → bindable value
//   $big_unsigned     — expression converting Option<u64> → bindable value
//   $tz_datetime      — expression converting Option<Box<DateTime<FixedOffset>>> → bindable value
//   $json             — expression converting Option<Box<serde_json::Value>> → bindable value
macro_rules! impl_bind_fn {
    (
        $fn_name:ident,
        $args_ty:ty,
        tiny_unsigned:  $tiny_unsigned:expr,
        small_unsigned: $small_unsigned:expr,
        unsigned:       $unsigned:expr,
        big_unsigned:   $big_unsigned:expr,
        tz_datetime:    $tz_datetime:expr,
        json:           $json:expr $(,)?
    ) => {
        fn $fn_name(args: &mut $args_ty, value: Value) {
            match value {
                Value::Bool(v)                       => { let _ = args.add(v); }
                Value::TinyInt(v)                    => { let _ = args.add(v); }
                Value::SmallInt(v)                   => { let _ = args.add(v); }
                Value::Int(v)                        => { let _ = args.add(v); }
                Value::BigInt(v)                     => { let _ = args.add(v); }
                Value::TinyUnsigned(v)               => { let _ = args.add($tiny_unsigned(v)); }
                Value::SmallUnsigned(v)              => { let _ = args.add($small_unsigned(v)); }
                Value::Unsigned(v)                   => { let _ = args.add($unsigned(v)); }
                Value::BigUnsigned(v)                => { let _ = args.add($big_unsigned(v)); }
                Value::Float(v)                      => { let _ = args.add(v); }
                Value::Double(v)                     => { let _ = args.add(v); }
                Value::String(v)                     => { let _ = args.add(v); }
                Value::Char(v)                       => { let _ = args.add(v.map(|c| c.to_string())); }
                Value::Bytes(v)                      => { let _ = args.add(v); }
                Value::ChronoDate(v)                 => { let _ = args.add(v); }
                Value::ChronoTime(v)                 => { let _ = args.add(v); }
                Value::ChronoDateTime(v)             => { let _ = args.add(v); }
                Value::ChronoDateTimeUtc(v)          => { let _ = args.add(v); }
                Value::ChronoDateTimeLocal(v)        => { let _ = args.add(v); }
                Value::ChronoDateTimeWithTimeZone(v) => { let _ = args.add($tz_datetime(v)); }
                Value::Uuid(v)                       => { let _ = args.add(v); }
                Value::Json(v)                       => { let _ = args.add($json(v)); }
            }
        }
    };
}

// ── SQLite ────────────────────────────────────────────────────────────────────

impl_bind_fn!(
    bind_sqlite,
    sqlx::sqlite::SqliteArguments<'static>,
    tiny_unsigned:  |v| v,
    small_unsigned: |v| v,
    unsigned:       |v| v,
    big_unsigned:   |v: Option<u64>| v.map(|u| i64::try_from(u).unwrap_or(i64::MAX)),
    tz_datetime:    |v| v,
    json:           |v: Option<Box<serde_json::Value>>| v.map(|j| *j),
);

pub fn sqlite_arguments(values: Values) -> sqlx::sqlite::SqliteArguments<'static> {
    let mut args = sqlx::sqlite::SqliteArguments::default();
    for v in values {
        bind_sqlite(&mut args, v);
    }
    args
}

// ── MySQL ─────────────────────────────────────────────────────────────────────

impl_bind_fn!(
    bind_mysql,
    sqlx::mysql::MySqlArguments,
    tiny_unsigned:  |v| v,
    small_unsigned: |v| v,
    unsigned:       |v| v,
    big_unsigned:   |v| v,      // MySQL natively supports u64
    // MySQL has no native tz-aware datetime type; store as UTC string.
    tz_datetime:    |v: Option<chrono::DateTime<chrono::FixedOffset>>|
                        v.map(|dt| dt.to_utc().to_string()),
    json:           |v: Option<Box<serde_json::Value>>| v.map(|j| *j),
);

pub fn mysql_arguments(values: Values) -> sqlx::mysql::MySqlArguments {
    let mut args = sqlx::mysql::MySqlArguments::default();
    for v in values {
        bind_mysql(&mut args, v);
    }
    args
}

// ── Postgres ──────────────────────────────────────────────────────────────────

impl_bind_fn!(
    bind_postgres,
    sqlx::postgres::PgArguments,
    // Postgres has no unsigned integer types; upcast to the next signed size.
    tiny_unsigned:  |v: Option<u8>|  v.map(|u| u as i16),
    small_unsigned: |v: Option<u16>| v.map(|u| u as i32),
    unsigned:       |v: Option<u32>| v.map(|u| u as i64),
    big_unsigned:   |v: Option<u64>| v.map(|u| i64::try_from(u).unwrap_or(i64::MAX)),
    tz_datetime:    |v| v,
    json:           |v: Option<Box<serde_json::Value>>| v.map(|j| sqlx::types::Json(*j)),
);

pub fn postgres_arguments(values: Values) -> sqlx::postgres::PgArguments {
    let mut args = sqlx::postgres::PgArguments::default();
    for v in values {
        bind_postgres(&mut args, v);
    }
    args
}
