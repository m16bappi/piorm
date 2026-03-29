use crate::error::{OrmpiError, Result};
use crate::model::def::ModelDef;
use crate::model::registry::ModelRegistry;
use crate::value::OrmpiValue;

/// Django-style field lookup operators.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup {
    Exact,
    IExact,
    Contains,
    IContains,
    StartsWith,
    IStartsWith,
    EndsWith,
    IEndsWith,
    In,
    Gt,
    Gte,
    Lt,
    Lte,
    Range,
    IsNull,
    Regex,
    IRegex,
    Date,
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Week,
    WeekDay,
    Quarter,
}

impl Lookup {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "exact" => Some(Self::Exact),
            "iexact" => Some(Self::IExact),
            "contains" => Some(Self::Contains),
            "icontains" => Some(Self::IContains),
            "startswith" => Some(Self::StartsWith),
            "istartswith" => Some(Self::IStartsWith),
            "endswith" => Some(Self::EndsWith),
            "iendswith" => Some(Self::IEndsWith),
            "in" => Some(Self::In),
            "gt" => Some(Self::Gt),
            "gte" => Some(Self::Gte),
            "lt" => Some(Self::Lt),
            "lte" => Some(Self::Lte),
            "range" => Some(Self::Range),
            "isnull" => Some(Self::IsNull),
            "regex" => Some(Self::Regex),
            "iregex" => Some(Self::IRegex),
            "date" => Some(Self::Date),
            "year" => Some(Self::Year),
            "month" => Some(Self::Month),
            "day" => Some(Self::Day),
            "hour" => Some(Self::Hour),
            "minute" => Some(Self::Minute),
            "second" => Some(Self::Second),
            "week" => Some(Self::Week),
            "weekday" => Some(Self::WeekDay),
            "quarter" => Some(Self::Quarter),
            _ => None,
        }
    }
}

/// A Q-object node supporting AND, OR, NOT composition.
#[derive(Debug, Clone)]
pub enum QNode {
    Leaf {
        field_path: Vec<String>,
        lookup: Lookup,
        value: OrmpiValue,
    },
    And(Vec<QNode>),
    Or(Vec<QNode>),
    Not(Box<QNode>),
}

/// A single node in a filter condition — either a simple lookup or a Q object.
#[derive(Debug, Clone)]
pub enum FilterNode {
    Lookup {
        field_path: Vec<String>,
        lookup: Lookup,
        value: OrmpiValue,
    },
    Q(QNode),
}

/// Parse a Django-style filter kwarg key (e.g. "title__icontains") into a FilterNode.
///
/// Validates that the first field segment exists in `model_def`.
/// Returns `OrmpiError::UnknownField` if the field doesn't exist.
pub fn parse_filter_kwarg(
    key: &str,
    value: OrmpiValue,
    model_def: &ModelDef,
    _registry: &ModelRegistry,
) -> Result<FilterNode> {
    let parts: Vec<&str> = key.split("__").collect();

    // Split off a trailing lookup suffix if recognised.
    let (field_parts, lookup) = match parts.last() {
        Some(last) if Lookup::from_str(last).is_some() => {
            (&parts[..parts.len() - 1], Lookup::from_str(last).unwrap())
        }
        _ => (parts.as_slice(), Lookup::Exact),
    };

    let first_field = field_parts.first().ok_or_else(|| OrmpiError::UnknownField {
        keyword: key.to_string(),
        choices: model_def
            .fields
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", "),
    })?;

    // "pk" is always valid; otherwise check attname, column_name, or FK attr name.
    if *first_field != "pk" {
        let fk_attname = format!("{}_id", first_field);
        let known = model_def.fields.contains_key(*first_field)
            || model_def.fields.contains_key(fk_attname.as_str())
            || model_def
                .fields
                .values()
                .any(|f| f.column_name() == *first_field);
        if !known {
            return Err(OrmpiError::UnknownField {
                keyword: key.to_string(),
                choices: model_def
                    .fields
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
    }

    Ok(FilterNode::Lookup {
        field_path: field_parts.iter().map(|s| s.to_string()).collect(),
        lookup,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_from_str() {
        assert_eq!(Lookup::from_str("exact"), Some(Lookup::Exact));
        assert_eq!(Lookup::from_str("icontains"), Some(Lookup::IContains));
        assert_eq!(Lookup::from_str("gt"), Some(Lookup::Gt));
        assert_eq!(Lookup::from_str("isnull"), Some(Lookup::IsNull));
        assert_eq!(Lookup::from_str("unknown"), None);
    }

    #[test]
    fn test_lookup_unknown_defaults_to_exact() {
        // "notafield" is not a lookup so defaults to Exact
        assert!(Lookup::from_str("notafield").is_none());
    }
}
