use indexmap::IndexMap;

use crate::model::def::OrderSpec;
use crate::query::aggregate::AggregateExpr;
use crate::query::filter::FilterNode;

/// Accumulated query state — built up by chaining QuerySet methods.
/// No DB hits happen until the QuerySet is evaluated.
#[derive(Debug, Clone)]
pub struct QueryState {
    pub app_label: String,
    pub model_name: String,
    /// Override the database alias (e.g. "replica").
    pub database: Option<String>,
    /// WHERE conditions accumulated by `.filter()`.
    pub filters: ConditionTree,
    /// ORDER BY specs.
    pub order_by: Vec<OrderSpec>,
    /// Whether SELECT DISTINCT is active.
    pub distinct: bool,
    /// Fields to DISTINCT ON (Postgres).
    pub distinct_fields: Vec<String>,
    /// Column projection.
    pub select_fields: SelectFields,
    /// LIMIT.
    pub limit: Option<u64>,
    /// OFFSET.
    pub offset: Option<u64>,
    /// What the queryset should return when evaluated.
    pub result_type: ResultType,
    /// Named aggregate expressions added via `.annotate()`.
    pub annotations: IndexMap<String, AggregateExpr>,
    /// FK paths to JOIN in a single query via `select_related()`.
    pub select_related_fields: Vec<String>,
    /// Reverse FK / M2M paths to batch-fetch via `prefetch_related()`.
    pub prefetch_fields: Vec<String>,
}

impl QueryState {
    pub fn new(app_label: String, model_name: String) -> Self {
        Self {
            app_label,
            model_name,
            database: None,
            filters: ConditionTree::default(),
            order_by: vec![],
            distinct: false,
            distinct_fields: vec![],
            select_fields: SelectFields::All,
            limit: None,
            offset: None,
            result_type: ResultType::Model,
            annotations: IndexMap::new(),
            select_related_fields: vec![],
            prefetch_fields: vec![],
        }
    }
}

/// Which columns to return.
#[derive(Debug, Clone, Default)]
pub enum SelectFields {
    #[default]
    All,
    /// `.only(fields)` — fetch only these columns.
    Only(Vec<String>),
    /// `.defer(fields)` — fetch all except these columns.
    Defer(Vec<String>),
}

/// What shape the evaluated queryset returns.
#[derive(Debug, Clone, Default)]
pub enum ResultType {
    /// Returns model instances.
    #[default]
    Model,
    /// Returns `Vec<HashMap<String, Value>>`.
    Values,
    /// Returns `Vec<Vec<Value>>`.
    ValuesList(Vec<String>),
    /// Returns a flat `Vec<Value>` for a single field.
    FlatList(String),
    Count,
    Exists,
}

/// Filter / exclude condition buckets.
#[derive(Debug, Clone, Default)]
pub struct ConditionTree {
    /// ANDed together and applied as WHERE.
    pub filters: Vec<FilterNode>,
    /// ANDed together, then negated (NOT ... AND NOT ...).
    pub excludes: Vec<FilterNode>,
}
