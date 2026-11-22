use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Arithmetic operator for F() expression trees.
#[derive(Debug, Clone, PartialEq)]
pub enum FExprOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

/// A recursive F() expression tree node.
#[derive(Debug, Clone, PartialEq)]
pub enum FExprNode {
    Field(String),
    BinOp {
        lhs: Box<FExprNode>,
        op: FExprOp,
        rhs: Box<FExprNode>,
    },
    Literal(Box<PiormValue>),
}

/// A database column value, backend-agnostic.
#[derive(Debug, Clone, PartialEq)]
pub enum PiormValue {
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
    List(Vec<PiormValue>),
    FExpr(Box<FExprNode>),
}

impl PiormValue {
    /// Convert to `sea_query::Value` for use in query parameters.
    pub fn to_sea_value(&self) -> sea_query::Value {
        match self {
            PiormValue::Null => sea_query::Value::Int(None),
            PiormValue::Bool(b) => sea_query::Value::Bool(Some(*b)),
            PiormValue::Int(i) => sea_query::Value::BigInt(Some(*i)),
            PiormValue::Float(f) => sea_query::Value::Double(Some(*f)),
            PiormValue::Text(s) => sea_query::Value::String(Some(s.clone())),
            PiormValue::Bytes(b) => sea_query::Value::Bytes(Some(b.clone())),
            PiormValue::Date(d) => sea_query::Value::ChronoDate(Some(*d)),
            PiormValue::Time(t) => sea_query::Value::ChronoTime(Some(*t)),
            PiormValue::DateTime(dt) => sea_query::Value::ChronoDateTime(Some(*dt)),
            PiormValue::DateTimeTz(dt) => {
                sea_query::Value::ChronoDateTimeUtc(Some(*dt))
            }
            PiormValue::Uuid(u) => sea_query::Value::Uuid(Some(*u)),
            PiormValue::Json(j) => sea_query::Value::Json(Some(Box::new(j.clone()))),
            PiormValue::Decimal(d) => sea_query::Value::Double(Some(
                d.to_f64().unwrap_or(f64::NAN),
            )),
            PiormValue::List(_) => sea_query::Value::Bytes(None), // handled specially in lookup_to_expr
            PiormValue::FExpr(_) => sea_query::Value::Bytes(None), // never called; handled via fexpr_to_sea_expr
        }
    }

    /// Convert this value into an `FExprNode` suitable for use in expression trees.
    pub fn into_fexpr_node(self) -> FExprNode {
        match self {
            PiormValue::FExpr(node) => *node,
            other => FExprNode::Literal(Box::new(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_roundtrip() {
        let v = PiormValue::Null;
        assert!(matches!(v.to_sea_value(), sea_query::Value::Int(None)));
    }

    #[test]
    fn test_int_roundtrip() {
        let v = PiormValue::Int(42);
        assert!(matches!(v.to_sea_value(), sea_query::Value::BigInt(Some(42))));
    }

    #[test]
    fn test_text_roundtrip() {
        let v = PiormValue::Text("hello".into());
        assert!(matches!(v.to_sea_value(), sea_query::Value::String(Some(s)) if s == "hello"));
    }

    #[test]
    fn test_bool_roundtrip() {
        let v = PiormValue::Bool(true);
        assert!(matches!(v.to_sea_value(), sea_query::Value::Bool(Some(true))));
    }
}
