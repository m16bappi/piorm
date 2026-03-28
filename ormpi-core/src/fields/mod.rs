pub mod types;

pub use types::{FieldType, OnDelete};

use crate::value::OrmpiValue;

/// Metadata describing a single model field / database column.
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub attname: String,           // column name — usually same as name, or "{name}_id" for FK
    pub db_column: Option<String>, // explicit override
    pub field_type: FieldType,
    pub null: bool,
    pub blank: bool,
    pub default: DefaultValue,
    pub unique: bool,
    pub db_index: bool,
    pub primary_key: bool,
    pub editable: bool,
    pub choices: Vec<(OrmpiValue, String)>, // (db_value, display_label)
    pub auto_now: bool,
    pub auto_now_add: bool,
    pub max_length: Option<u32>,
    pub validators: Vec<BuiltinValidator>,
    pub help_text: String,
    pub verbose_name: Option<String>,
    pub related_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DefaultValue {
    None,
    Null,
    Value(OrmpiValue),
    CurrentTimestamp,
    EmptyString,
}

#[derive(Debug, Clone)]
pub enum BuiltinValidator {
    MinValue(f64),
    MaxValue(f64),
    MinLength(usize),
    MaxLength(usize),
    Regex(String),
    EmailValidator,
    UrlValidator,
    IpValidator,
}

impl FieldDef {
    /// The actual database column name (db_column override or attname).
    pub fn column_name(&self) -> String {
        if let Some(ref explicit) = self.db_column {
            explicit.clone()
        } else {
            self.attname.clone()
        }
    }

    /// Convert this field definition to a sea-query `ColumnDef` for DDL.
    /// Returns `None` for M2M fields (they don't produce a column in this table).
    pub fn to_column_def(&self) -> Option<sea_query::ColumnDef> {
        let col_name = self.column_name();
        let mut col = sea_query::ColumnDef::new(sea_query::Alias::new(&col_name));

        match &self.field_type {
            FieldType::AutoInt => {
                col.integer().auto_increment().primary_key();
                return Some(col);
            }
            FieldType::AutoBigInt => {
                col.big_integer().auto_increment().primary_key();
                return Some(col);
            }
            FieldType::AutoSmallInt => {
                col.small_integer().auto_increment().primary_key();
                return Some(col);
            }
            FieldType::SmallInt | FieldType::PositiveSmallInt => {
                col.small_integer();
            }
            FieldType::Int | FieldType::PositiveInt => {
                col.integer();
            }
            FieldType::BigInt | FieldType::PositiveBigInt => {
                col.big_integer();
            }
            FieldType::Float => {
                col.float();
            }
            FieldType::Double => {
                col.double();
            }
            FieldType::Decimal { max_digits, decimal_places } => {
                col.decimal_len(*max_digits, *decimal_places);
            }
            FieldType::Char { max_length }
            | FieldType::Slug { max_length }
            | FieldType::Email { max_length }
            | FieldType::Url { max_length } => {
                col.string_len(*max_length);
            }
            FieldType::Text | FieldType::IpAddress | FieldType::GenericIpAddress => {
                col.text();
            }
            FieldType::Boolean => {
                col.boolean();
            }
            FieldType::Date => {
                col.date();
            }
            FieldType::Time => {
                col.time();
            }
            FieldType::DateTime => {
                col.timestamp_with_time_zone();
            }
            FieldType::Duration => {
                col.big_integer(); // stored as microseconds
            }
            FieldType::Binary => {
                col.blob();
            }
            FieldType::Uuid => {
                col.uuid();
            }
            FieldType::Json => {
                col.json();
            }
            FieldType::JsonB => {
                col.json_binary();
            }
            FieldType::ForeignKey { .. } | FieldType::OneToOne { .. } => {
                col.big_integer(); // FK column stores the related PK
            }
            FieldType::ManyToMany { .. } => return None,
        }

        if self.primary_key {
            col.primary_key();
        }
        if !self.null {
            col.not_null();
        }
        if self.unique {
            col.unique_key();
        }

        match &self.default {
            DefaultValue::None => {}
            DefaultValue::Null => {
                if self.null {
                    col.default(sea_query::Value::Int(None));
                }
            }
            DefaultValue::Value(v) => {
                col.default(v.to_sea_value());
            }
            DefaultValue::CurrentTimestamp => {
                col.default(sea_query::Keyword::CurrentTimestamp);
            }
            DefaultValue::EmptyString => {
                col.default("");
            }
        }

        Some(col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_query::{SqliteQueryBuilder, Table};

    fn make_field(ft: FieldType) -> FieldDef {
        FieldDef {
            name: "test".into(),
            attname: "test".into(),
            db_column: None,
            field_type: ft,
            null: false,
            blank: false,
            default: DefaultValue::None,
            unique: false,
            db_index: false,
            primary_key: false,
            editable: true,
            choices: vec![],
            auto_now: false,
            auto_now_add: false,
            max_length: None,
            validators: vec![],
            help_text: String::new(),
            verbose_name: None,
            related_name: None,
        }
    }

    #[test]
    fn test_integer_not_null() {
        let f = make_field(FieldType::Int);
        let col = f.to_column_def().unwrap();
        let sql = Table::create()
            .table(sea_query::Alias::new("t"))
            .col(col)
            .to_string(SqliteQueryBuilder);
        assert!(sql.contains("NOT NULL"), "expected NOT NULL in: {sql}");
        assert!(sql.contains("integer"), "expected integer in: {sql}");
    }

    #[test]
    fn test_varchar_with_length() {
        let f = make_field(FieldType::Char { max_length: 100 });
        let col = f.to_column_def().unwrap();
        let sql = Table::create()
            .table(sea_query::Alias::new("t"))
            .col(col)
            .to_string(sea_query::MysqlQueryBuilder);
        assert!(sql.contains("100"), "expected length 100 in: {sql}");
    }

    #[test]
    fn test_m2m_returns_none() {
        let f = make_field(FieldType::ManyToMany {
            to: "Other".into(),
            through: None,
            db_table: None,
        });
        assert!(f.to_column_def().is_none());
    }

    #[test]
    fn test_auto_int_primary_key() {
        let f = make_field(FieldType::AutoInt);
        let col = f.to_column_def().unwrap();
        let sql = Table::create()
            .table(sea_query::Alias::new("t"))
            .col(col)
            .to_string(SqliteQueryBuilder);
        assert!(sql.contains("PRIMARY KEY"), "expected PRIMARY KEY in: {sql}");
        assert!(sql.contains("AUTO"), "expected AUTO in: {sql}");
    }

    #[test]
    fn test_unique_constraint() {
        let mut f = make_field(FieldType::Text);
        f.unique = true;
        let col = f.to_column_def().unwrap();
        let sql = Table::create()
            .table(sea_query::Alias::new("t"))
            .col(col)
            .to_string(SqliteQueryBuilder);
        assert!(sql.contains("UNIQUE"), "expected UNIQUE in: {sql}");
    }

    #[test]
    fn test_nullable_column() {
        let mut f = make_field(FieldType::Text);
        f.null = true;
        let col = f.to_column_def().unwrap();
        let sql = Table::create()
            .table(sea_query::Alias::new("t"))
            .col(col)
            .to_string(SqliteQueryBuilder);
        assert!(!sql.contains("NOT NULL"), "expected no NOT NULL in: {sql}");
    }

    #[test]
    fn test_default_value() {
        let mut f = make_field(FieldType::Int);
        f.null = true;
        f.default = DefaultValue::Value(OrmpiValue::Int(0));
        let col = f.to_column_def().unwrap();
        let sql = Table::create()
            .table(sea_query::Alias::new("t"))
            .col(col)
            .to_string(SqliteQueryBuilder);
        assert!(sql.contains("DEFAULT"), "expected DEFAULT in: {sql}");
    }
}
