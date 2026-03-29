use std::collections::HashMap;

use sea_query::{Alias, Asterisk, Cond, Expr, ExprTrait, Order, Query};

use crate::backend::QueryBackend;
use crate::bind::{mysql_arguments, postgres_arguments, sqlite_arguments};
use crate::model::def::ModelDef;
use crate::pool::OrmpiPool;
use crate::query::filter::{FilterNode, Lookup, QNode};
use crate::query::state::{QueryState, SelectFields};
use crate::row::{mysql_row_to_ormpi, postgres_row_to_ormpi, sqlite_row_to_ormpi, OrmpiRow};
use crate::transaction::{OrmpiTransaction, TransactionHandle};
use crate::value::OrmpiValue;
use crate::{OrmpiError, Result};

// Re-export for convenience in callers
pub use crate::query::aggregate::AggregateExpr;

pub fn build_select(state: &QueryState, model_def: &ModelDef) -> sea_query::SelectStatement {
    let mut stmt = Query::select();
    let table = Alias::new(&model_def.table_name);
    let has_annotations = !state.annotations.is_empty();

    match &state.select_fields {
        SelectFields::All => {
            stmt.from(table).column(Asterisk);
        }
        SelectFields::Only(fields) => {
            stmt.from(table);
            for f in fields {
                stmt.column(Alias::new(f));
            }
        }
        SelectFields::Defer(deferred) => {
            stmt.from(table);
            for field in model_def.fields.values() {
                if !deferred.contains(&field.attname) {
                    stmt.column(Alias::new(&field.column_name()));
                }
            }
        }
    }

    // Add annotation expressions as named columns (e.g. COUNT(*) AS comment_count).
    if has_annotations {
        for (alias, agg_expr) in &state.annotations {
            stmt.expr_as(agg_expr.to_sea_expr(), Alias::new(alias));
        }
        // GROUP BY pk so each row gets its own annotation.
        let pk_col = Alias::new(&model_def.pk_field);
        stmt.group_by_col(pk_col);
    }

    for node in &state.filters.filters {
        let cond = filter_node_to_cond(node, model_def, false);
        stmt.cond_where(cond);
    }
    for node in &state.filters.excludes {
        let cond = filter_node_to_cond(node, model_def, true);
        stmt.cond_where(cond);
    }

    for order in &state.order_by {
        let field = if order.field == "pk" { model_def.pk_field.as_str() } else { order.field.as_str() };
        let col = Alias::new(field);
        let dir = if order.descending { Order::Desc } else { Order::Asc };
        stmt.order_by(col, dir);
    }
    if state.order_by.is_empty() {
        for order in &model_def.ordering {
            let field = if order.field == "pk" { model_def.pk_field.as_str() } else { order.field.as_str() };
            let col = Alias::new(field);
            let dir = if order.descending { Order::Desc } else { Order::Asc };
            stmt.order_by(col, dir);
        }
    }

    if state.distinct {
        stmt.distinct();
    }
    if let Some(limit) = state.limit {
        stmt.limit(limit);
    }
    if let Some(offset) = state.offset {
        stmt.offset(offset);
    }

    stmt.to_owned()
}

pub async fn evaluate(
    state: &QueryState,
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<Vec<OrmpiRow>> {
    let mut stmt = build_select(state, model_def);

    // Apply select_related JOINs if any paths were specified.
    if !state.select_related_fields.is_empty() {
        use crate::model::registry::ModelRegistry;
        use crate::query::join::apply_select_related;
        let reg = ModelRegistry::global();
        let guard = reg
            .read()
            .map_err(|_| OrmpiError::Other("registry lock poisoned".into()))?;
        apply_select_related(&mut stmt, model_def, &state.select_related_fields, &*guard)?;
    }

    let (sql, sea_vals) = backend.build_select(&stmt);

    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(sqlite_row_to_ormpi).collect()
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(mysql_row_to_ormpi).collect()
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(postgres_row_to_ormpi).collect()
        }
    }
}

pub async fn evaluate_count(
    state: &QueryState,
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<i64> {
    use sqlx::Row as _;

    let mut count_state = state.clone();
    count_state.order_by.clear();
    count_state.limit = None;
    count_state.offset = None;

    let inner = build_select(&count_state, model_def);

    let mut stmt = Query::select();
    stmt.expr(Expr::cust("COUNT(*)"))
        .from_subquery(inner, Alias::new("_c"));

    let (sql, sea_vals) = backend.build_select(&stmt);

    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            Ok(row.try_get::<i64, _>(0)?)
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            Ok(row.try_get::<i64, _>(0)?)
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            Ok(row.try_get::<i64, _>(0)?)
        }
    }
}

/// INSERT a single row and return the primary-key value of the new row.
pub async fn create_row(
    field_values: &[(String, OrmpiValue)],
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<i64> {
    let table = Alias::new(&model_def.table_name);
    let mut stmt = Query::insert();
    stmt.into_table(table);

    let mut cols = vec![];
    let mut vals: Vec<Expr> = vec![];
    for (field_name, value) in field_values {
        let col_name = if let Some(f) = model_def.fields.get(field_name.as_str()) {
            f.column_name()
        } else {
            field_name.clone()
        };
        cols.push(Alias::new(&col_name));
        vals.push(Expr::val(value.to_sea_value()));
    }
    stmt.columns(cols);
    stmt.values(vals).map_err(|e| OrmpiError::Other(e.to_string()))?;

    let (sql, sea_vals) = backend.build_insert(&stmt);

    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.last_insert_rowid())
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.last_insert_id() as i64)
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected() as i64)
        }
    }
}

/// Run a whole-queryset aggregate (no GROUP BY).
/// Returns `HashMap<alias, OrmpiValue>` — one entry per named aggregate.
pub async fn evaluate_aggregate(
    state: &QueryState,
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<HashMap<String, OrmpiValue>> {
    use sqlx::Row as _;

    if state.annotations.is_empty() {
        return Ok(HashMap::new());
    }

    // Build a SELECT agg1 AS alias1, agg2 AS alias2 ... FROM table [WHERE ...]
    let mut stmt = Query::select();
    let table = Alias::new(&model_def.table_name);
    stmt.from(table);

    for (alias, agg_expr) in &state.annotations {
        stmt.expr_as(agg_expr.to_sea_expr(), Alias::new(alias));
    }

    for node in &state.filters.filters {
        let cond = filter_node_to_cond(node, model_def, false);
        stmt.cond_where(cond);
    }
    for node in &state.filters.excludes {
        let cond = filter_node_to_cond(node, model_def, true);
        stmt.cond_where(cond);
    }

    let (sql, sea_vals) = backend.build_select(&stmt);
    let aliases: Vec<String> = state.annotations.keys().cloned().collect();

    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            let mut result = HashMap::new();
            for (i, alias) in aliases.iter().enumerate() {
                // Try i64 first (COUNT), then f64 (AVG/SUM).
                let val = if let Ok(v) = row.try_get::<i64, _>(i) {
                    OrmpiValue::Int(v)
                } else if let Ok(v) = row.try_get::<f64, _>(i) {
                    OrmpiValue::Float(v)
                } else {
                    OrmpiValue::Null
                };
                result.insert(alias.clone(), val);
            }
            Ok(result)
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            let mut result = HashMap::new();
            for (i, alias) in aliases.iter().enumerate() {
                let val = if let Ok(v) = row.try_get::<i64, _>(i) {
                    OrmpiValue::Int(v)
                } else if let Ok(v) = row.try_get::<f64, _>(i) {
                    OrmpiValue::Float(v)
                } else {
                    OrmpiValue::Null
                };
                result.insert(alias.clone(), val);
            }
            Ok(result)
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            let mut result = HashMap::new();
            for (i, alias) in aliases.iter().enumerate() {
                let val = if let Ok(v) = row.try_get::<i64, _>(i) {
                    OrmpiValue::Int(v)
                } else if let Ok(v) = row.try_get::<f64, _>(i) {
                    OrmpiValue::Float(v)
                } else {
                    OrmpiValue::Null
                };
                result.insert(alias.clone(), val);
            }
            Ok(result)
        }
    }
}

/// Execute a bulk UPDATE from QueryState filters.
/// Returns the number of rows affected.
pub async fn bulk_update(
    state: &QueryState,
    updates: &HashMap<String, OrmpiValue>,
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<u64> {
    let table = Alias::new(&model_def.table_name);
    let mut stmt = Query::update();
    stmt.table(table);

    for (field_name, value) in updates {
        let col_name = resolve_column(&[field_name.clone()], model_def);
        stmt.value(Alias::new(&col_name), value.to_sea_value());
    }

    for node in &state.filters.filters {
        let cond = filter_node_to_cond(node, model_def, false);
        stmt.cond_where(cond);
    }
    for node in &state.filters.excludes {
        let cond = filter_node_to_cond(node, model_def, true);
        stmt.cond_where(cond);
    }

    let (sql, sea_vals) = backend.build_update(&stmt);

    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
    }
}

/// Execute a bulk DELETE from QueryState filters.
/// Returns the number of rows deleted.
pub async fn bulk_delete(
    state: &QueryState,
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<u64> {
    let table = Alias::new(&model_def.table_name);
    let mut stmt = Query::delete();
    stmt.from_table(table);

    for node in &state.filters.filters {
        let cond = filter_node_to_cond(node, model_def, false);
        stmt.cond_where(cond);
    }
    for node in &state.filters.excludes {
        let cond = filter_node_to_cond(node, model_def, true);
        stmt.cond_where(cond);
    }

    let (sql, sea_vals) = backend.build_delete(&stmt);

    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
    }
}

/// Batch-INSERT rows in chunks of `batch_size`.
/// Returns the total number of rows inserted.
pub async fn bulk_create_rows(
    rows: Vec<Vec<(String, OrmpiValue)>>,
    batch_size: usize,
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<usize> {
    if rows.is_empty() {
        return Ok(0);
    }
    let table = Alias::new(&model_def.table_name);
    let mut total = 0usize;

    for chunk in rows.chunks(batch_size) {
        if chunk.is_empty() {
            continue;
        }
        // Determine columns from the first row.
        let first = &chunk[0];
        let col_names: Vec<String> = first
            .iter()
            .map(|(k, _)| {
                if let Some(f) = model_def.fields.get(k.as_str()) {
                    f.column_name()
                } else {
                    k.clone()
                }
            })
            .collect();

        let mut stmt = Query::insert();
        stmt.into_table(table.clone());
        stmt.columns(col_names.iter().map(|c| Alias::new(c)));

        for row in chunk {
            let vals: Vec<Expr> = row.iter().map(|(_, v)| Expr::val(v.to_sea_value())).collect();
            stmt.values(vals)
                .map_err(|e| OrmpiError::Other(e.to_string()))?;
        }

        let (sql, sea_vals) = backend.build_insert(&stmt);

        match pool {
            OrmpiPool::Sqlite(p) => {
                let args = sqlite_arguments(sea_vals);
                sqlx::query_with(&sql, args).execute(p).await?;
            }
            OrmpiPool::Mysql(p) => {
                let args = mysql_arguments(sea_vals);
                sqlx::query_with(&sql, args).execute(p).await?;
            }
            OrmpiPool::Postgres(p) => {
                let args = postgres_arguments(sea_vals);
                sqlx::query_with(&sql, args).execute(p).await?;
            }
        }
        total += chunk.len();
    }
    Ok(total)
}

/// UPDATE a single row identified by its primary key.
pub async fn update_row_by_pk(
    pk_val: &OrmpiValue,
    updates: &[(String, OrmpiValue)],
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<u64> {
    if updates.is_empty() {
        return Ok(0);
    }
    let table = Alias::new(&model_def.table_name);
    let mut stmt = Query::update();
    stmt.table(table);

    for (field_name, value) in updates {
        let col_name = if let Some(f) = model_def.fields.get(field_name.as_str()) {
            f.column_name()
        } else {
            field_name.clone()
        };
        stmt.value(Alias::new(&col_name), value.to_sea_value());
    }

    stmt.cond_where(
        Expr::col(Alias::new(&model_def.pk_field)).eq(pk_val.to_sea_value()),
    );

    let (sql, sea_vals) = backend.build_update(&stmt);

    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
    }
}

/// SELECT * FROM `table_name` WHERE `column` IN (`values`).
/// Used for `prefetch_related` batch fetches.
pub async fn fetch_by_column_in(
    table_name: &str,
    column: &str,
    values: &[OrmpiValue],
    pool: &OrmpiPool,
    backend: &QueryBackend,
) -> Result<Vec<OrmpiRow>> {
    if values.is_empty() {
        return Ok(vec![]);
    }

    let sea_vals: Vec<sea_query::Value> = values.iter().map(|v| v.to_sea_value()).collect();
    let mut stmt = Query::select();
    stmt.from(Alias::new(table_name))
        .column(sea_query::Asterisk)
        .and_where(Expr::col(Alias::new(column)).is_in(sea_vals));

    let (sql, binds) = backend.build_select(&stmt);

    match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(binds);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(sqlite_row_to_ormpi).collect()
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(binds);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(mysql_row_to_ormpi).collect()
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(binds);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(postgres_row_to_ormpi).collect()
        }
    }
}

fn filter_node_to_cond(node: &FilterNode, model_def: &ModelDef, negate: bool) -> Cond {
    match node {
        FilterNode::Lookup { field_path, lookup, value } => {
            let col_name = resolve_column(field_path, model_def);
            let col = Alias::new(&col_name);
            let expr = lookup_to_expr(col, lookup, value);
            if negate {
                Cond::all().not().add(Cond::all().add(expr))
            } else {
                Cond::all().add(expr)
            }
        }
        FilterNode::Q(q_node) => {
            let cond = qnode_to_cond(q_node, model_def);
            if negate {
                cond.not()
            } else {
                cond
            }
        }
    }
}

fn qnode_to_cond(node: &QNode, model_def: &ModelDef) -> Cond {
    match node {
        QNode::Leaf { field_path, lookup, value } => {
            let col_name = resolve_column(field_path, model_def);
            let col = Alias::new(&col_name);
            let expr = lookup_to_expr(col, lookup, value);
            Cond::all().add(expr)
        }
        QNode::And(nodes) => {
            let mut cond = Cond::all();
            for n in nodes {
                cond = cond.add(qnode_to_cond(n, model_def));
            }
            cond
        }
        QNode::Or(nodes) => {
            let mut cond = Cond::any();
            for n in nodes {
                cond = cond.add(qnode_to_cond(n, model_def));
            }
            cond
        }
        QNode::Not(inner) => qnode_to_cond(inner, model_def).not(),
    }
}

fn lookup_to_expr(col: Alias, lookup: &Lookup, value: &OrmpiValue) -> sea_query::SimpleExpr {
    let sea_val = value.to_sea_value();
    match lookup {
        Lookup::Exact | Lookup::IExact => Expr::col(col).eq(sea_val),
        Lookup::Contains => Expr::col(col).like(format!("%{}%", value_as_str(value))),
        // ilike is Postgres-only; use like for cross-backend compatibility (case-insensitive on most DBs)
        Lookup::IContains => Expr::col(col).like(format!("%{}%", value_as_str(value))),
        Lookup::StartsWith => Expr::col(col).like(format!("{}%", value_as_str(value))),
        Lookup::IStartsWith => Expr::col(col).like(format!("{}%", value_as_str(value))),
        Lookup::EndsWith => Expr::col(col).like(format!("%{}", value_as_str(value))),
        Lookup::IEndsWith => Expr::col(col).like(format!("%{}", value_as_str(value))),
        Lookup::Gt => Expr::col(col).gt(sea_val),
        Lookup::Gte => Expr::col(col).gte(sea_val),
        Lookup::Lt => Expr::col(col).lt(sea_val),
        Lookup::Lte => Expr::col(col).lte(sea_val),
        Lookup::IsNull => {
            if matches!(value, OrmpiValue::Bool(true)) {
                Expr::col(col).is_null()
            } else {
                Expr::col(col).is_not_null()
            }
        }
        Lookup::In => match value {
            OrmpiValue::List(items) => {
                let sea_vals: Vec<sea_query::Value> =
                    items.iter().map(|v| v.to_sea_value()).collect();
                Expr::col(col).is_in(sea_vals)
            }
            single => Expr::col(col).eq(single.to_sea_value()),
        },
        Lookup::Range => match value {
            OrmpiValue::List(bounds) if bounds.len() == 2 => {
                let lo = bounds[0].to_sea_value();
                let hi = bounds[1].to_sea_value();
                Expr::col(col.clone()).gte(lo).and(Expr::col(col).lte(hi))
            }
            _ => Expr::col(col).eq(sea_val),
        },
        _ => Expr::col(col).eq(sea_val),
    }
}

fn resolve_column(field_path: &[String], model_def: &ModelDef) -> String {
    let first = match field_path.first() {
        Some(s) => s.as_str(),
        None => return model_def.pk_field.clone(),
    };
    if first == "pk" {
        return model_def.pk_field.clone();
    }
    if let Some(field) = model_def.fields.get(first) {
        return field.column_name();
    }
    for field in model_def.fields.values() {
        if field.column_name() == first {
            return first.to_string();
        }
    }
    first.to_string()
}

fn value_as_str(value: &OrmpiValue) -> String {
    match value {
        OrmpiValue::Text(s) => s.clone(),
        OrmpiValue::Int(i) => i.to_string(),
        OrmpiValue::Float(f) => f.to_string(),
        _ => String::new(),
    }
}

/// INSERT a single row within an active transaction and return the pk value.
pub async fn create_row_in_txn(
    field_values: &[(String, OrmpiValue)],
    txn: &mut OrmpiTransaction,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<i64> {
    use std::ops::DerefMut;

    let table = Alias::new(&model_def.table_name);
    let mut stmt = Query::insert();
    stmt.into_table(table);

    let mut cols = vec![];
    let mut vals: Vec<Expr> = vec![];
    for (field_name, value) in field_values {
        let col_name = if let Some(f) = model_def.fields.get(field_name.as_str()) {
            f.column_name()
        } else {
            field_name.clone()
        };
        cols.push(Alias::new(&col_name));
        vals.push(Expr::val(value.to_sea_value()));
    }
    stmt.columns(cols);
    stmt.values(vals).map_err(|e| OrmpiError::Other(e.to_string()))?;

    let (sql, sea_vals) = backend.build_insert(&stmt);

    match &mut txn.handle {
        TransactionHandle::Sqlite(t) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(t.as_mut().deref_mut()).await?;
            Ok(result.last_insert_rowid())
        }
        TransactionHandle::Mysql(t) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(t.as_mut().deref_mut()).await?;
            Ok(result.last_insert_id() as i64)
        }
        TransactionHandle::Postgres(t) => {
            let args = postgres_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(t.as_mut().deref_mut()).await?;
            Ok(result.rows_affected() as i64)
        }
    }
}

/// SELECT rows within an active transaction using QueryState filters.
pub async fn evaluate_in_txn(
    state: &QueryState,
    txn: &mut OrmpiTransaction,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<Vec<OrmpiRow>> {
    use std::ops::DerefMut;

    let stmt = build_select(state, model_def);
    let (sql, sea_vals) = backend.build_select(&stmt);

    match &mut txn.handle {
        TransactionHandle::Sqlite(t) => {
            let args = sqlite_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(t.as_mut().deref_mut()).await?;
            rows.iter().map(sqlite_row_to_ormpi).collect()
        }
        TransactionHandle::Mysql(t) => {
            let args = mysql_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(t.as_mut().deref_mut()).await?;
            rows.iter().map(mysql_row_to_ormpi).collect()
        }
        TransactionHandle::Postgres(t) => {
            let args = postgres_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(t.as_mut().deref_mut()).await?;
            rows.iter().map(postgres_row_to_ormpi).collect()
        }
    }
}
