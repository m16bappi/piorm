use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// A database column value, backend-agnostic.
#[derive(Debug, Clone, PartialEq)]
pub enum OrmpiValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime(NaiveDateTime),
    DateTimeTz(DateTime<Utc>),
    Uuid(Uuid),
    Json(JsonValue),
    Decimal(Decimal),
    List(Vec<OrmpiValue>),
}

impl OrmpiValue {
    /// Convert to `sea_query::Value` for use in query parameters.
    pub fn to_sea_value(&self) -> sea_query::Value {
        match self {
            OrmpiValue::Null => sea_query::Value::Int(None),
            OrmpiValue::Bool(b) => sea_query::Value::Bool(Some(*b)),
            OrmpiValue::Int(i) => sea_query::Value::BigInt(Some(*i)),
            OrmpiValue::Float(f) => sea_query::Value::Double(Some(*f)),
            OrmpiValue::Text(s) => sea_query::Value::String(Some(s.clone())),
            OrmpiValue::Bytes(b) => sea_query::Value::Bytes(Some(b.clone())),
            OrmpiValue::Date(d) => sea_query::Value::ChronoDate(Some(*d)),
            OrmpiValue::Time(t) => sea_query::Value::ChronoTime(Some(*t)),
            OrmpiValue::DateTime(dt) => sea_query::Value::ChronoDateTime(Some(*dt)),
            OrmpiValue::DateTimeTz(dt) => {
                sea_query::Value::ChronoDateTimeUtc(Some(*dt))
            }
            OrmpiValue::Uuid(u) => sea_query::Value::Uuid(Some(*u)),
            OrmpiValue::Json(j) => sea_query::Value::Json(Some(Box::new(j.clone()))),
            OrmpiValue::Decimal(d) => sea_query::Value::Double(Some(
                d.to_f64().unwrap_or(f64::NAN),
            )),
            OrmpiValue::List(_) => sea_query::Value::Bytes(None), // handled specially in lookup_to_expr
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_roundtrip() {
        let v = OrmpiValue::Null;
        assert!(matches!(v.to_sea_value(), sea_query::Value::Int(None)));
    }

    #[test]
    fn test_int_roundtrip() {
        let v = OrmpiValue::Int(42);
        assert!(matches!(v.to_sea_value(), sea_query::Value::BigInt(Some(42))));
    }

    #[test]
    fn test_text_roundtrip() {
        let v = OrmpiValue::Text("hello".into());
        assert!(matches!(v.to_sea_value(), sea_query::Value::String(Some(s)) if s == "hello"));
    }

    #[test]
    fn test_bool_roundtrip() {
        let v = OrmpiValue::Bool(true);
        assert!(matches!(v.to_sea_value(), sea_query::Value::Bool(Some(true))));
    }
}
