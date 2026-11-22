use std::collections::HashMap;

use sea_query::{Alias, Asterisk, Cond, Expr, ExprTrait, Order, Query};

use crate::backend::QueryBackend;
use crate::bind::{mysql_arguments, postgres_arguments, sqlite_arguments};
use crate::model::def::ModelDef;
use crate::pool::PiormPool;
use crate::query::filter::{FilterNode, Lookup, QNode};
use crate::query::state::{QueryState, SelectFields};
use crate::row::{mysql_row_to_piorm, postgres_row_to_piorm, sqlite_row_to_piorm, PiormRow};
use crate::value::{FExprNode, FExprOp, PiormValue};
use crate::{PiormError, Result};

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
    pool: &PiormPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<Vec<PiormRow>> {
    let mut stmt = build_select(state, model_def);

    // Apply select_related JOINs if any paths were specified.
    if !state.select_related_fields.is_empty() {
        use crate::model::registry::ModelRegistry;
        use crate::query::join::apply_select_related;
        let reg = ModelRegistry::global();
        let guard = reg
            .read()
            .map_err(|_| PiormError::Other("registry lock poisoned".into()))?;
        apply_select_related(&mut stmt, model_def, &state.select_related_fields, &*guard)?;
    }

    let (sql, sea_vals) = backend.build_select(&stmt);

    match pool {
        PiormPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(sqlite_row_to_piorm).collect()
        }
        PiormPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(mysql_row_to_piorm).collect()
        }
        PiormPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(postgres_row_to_piorm).collect()
        }
    }
}

pub async fn evaluate_count(
    state: &QueryState,
    pool: &PiormPool,
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
        PiormPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            Ok(row.try_get::<i64, _>(0)?)
        }
        PiormPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            Ok(row.try_get::<i64, _>(0)?)
        }
        PiormPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            Ok(row.try_get::<i64, _>(0)?)
        }
    }
}

/// INSERT a single row and return the primary-key value of the new row.
pub async fn create_row(
    field_values: &[(String, PiormValue)],
    pool: &PiormPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<i64> {
    let table = Alias::new(&model_def.table_name);
    let mut stmt = Query::insert();
    stmt.into_table(table);

    let mut cols = vec![];
    let mut vals: Vec<Expr> = vec![];
    for (field_name, value) in field_values {
        let col_name = resolve_column(&[field_name.clone()], model_def);
        cols.push(Alias::new(&col_name));
        vals.push(Expr::val(value.to_sea_value()));
    }
    stmt.columns(cols);
    stmt.values(vals).map_err(|e| PiormError::Other(e.to_string()))?;

    let (sql, sea_vals) = backend.build_insert(&stmt);

    match pool {
        PiormPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.last_insert_rowid())
        }
        PiormPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.last_insert_id() as i64)
        }
        PiormPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected() as i64)
        }
    }
}

/// Run a whole-queryset aggregate (no GROUP BY).
/// Returns `HashMap<alias, PiormValue>` — one entry per named aggregate.
pub async fn evaluate_aggregate(
    state: &QueryState,
    pool: &PiormPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<HashMap<String, PiormValue>> {
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
        PiormPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            let mut result = HashMap::new();
            for (i, alias) in aliases.iter().enumerate() {
                // Try i64 first (COUNT), then f64 (AVG/SUM).
                let val = if let Ok(v) = row.try_get::<i64, _>(i) {
                    PiormValue::Int(v)
                } else if let Ok(v) = row.try_get::<f64, _>(i) {
                    PiormValue::Float(v)
                } else {
                    PiormValue::Null
                };
                result.insert(alias.clone(), val);
            }
            Ok(result)
        }
        PiormPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            let mut result = HashMap::new();
            for (i, alias) in aliases.iter().enumerate() {
                let val = if let Ok(v) = row.try_get::<i64, _>(i) {
                    PiormValue::Int(v)
                } else if let Ok(v) = row.try_get::<f64, _>(i) {
                    PiormValue::Float(v)
                } else {
                    PiormValue::Null
                };
                result.insert(alias.clone(), val);
            }
            Ok(result)
        }
        PiormPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            let mut result = HashMap::new();
            for (i, alias) in aliases.iter().enumerate() {
                let val = if let Ok(v) = row.try_get::<i64, _>(i) {
                    PiormValue::Int(v)
                } else if let Ok(v) = row.try_get::<f64, _>(i) {
                    PiormValue::Float(v)
                } else {
                    PiormValue::Null
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
    updates: &HashMap<String, PiormValue>,
    pool: &PiormPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<u64> {
    let table = Alias::new(&model_def.table_name);
    let mut stmt = Query::update();
    stmt.table(table);

    for (field_name, value) in updates {
        let col_name = resolve_column(&[field_name.clone()], model_def);
        if let PiormValue::FExpr(node) = value {
            stmt.value(Alias::new(&col_name), fexpr_to_sea_expr(node, &model_def.table_name));
        } else {
            stmt.value(Alias::new(&col_name), value.to_sea_value());
        }
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
        PiormPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        PiormPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        PiormPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
    }
}

/// Execute a bulk DELETE from QueryState filters, cascading to children.
/// Returns the number of rows deleted.
pub async fn bulk_delete(
    state: &QueryState,
    pool: &PiormPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
) -> Result<u64> {
    use crate::model::executor::{delete_children_by_pk_list, find_cascade_children};
    use sqlx::Row as _;

    // If there are CASCADE children, fetch parent PKs first so we can delete children.
    let cascade_children = find_cascade_children(&model_def.model_name);
    if !cascade_children.is_empty() {
        // SELECT pk FROM parent_table WHERE filters...
        let mut pk_state = state.clone();
        pk_state.select_fields = crate::query::state::SelectFields::Only(vec![model_def.pk_field.clone()]);
        pk_state.order_by.clear();
        pk_state.limit = None;
        pk_state.offset = None;
        let pk_stmt = build_select(&pk_state, model_def);
        let (pk_sql, pk_sea_vals) = backend.build_select(&pk_stmt);

        let pk_values: Vec<PiormValue> = match pool {
            PiormPool::Sqlite(p) => {
                let args = sqlite_arguments(pk_sea_vals);
                let rows = sqlx::query_with(&pk_sql, args).fetch_all(p).await?;
                rows.iter().map(|r| {
                    r.try_get::<i64, _>(0).map(PiormValue::Int)
                        .unwrap_or(PiormValue::Null)
                }).collect()
            }
            PiormPool::Mysql(p) => {
                let args = mysql_arguments(pk_sea_vals);
                let rows = sqlx::query_with(&pk_sql, args).fetch_all(p).await?;
                rows.iter().map(|r| {
                    r.try_get::<i64, _>(0).map(PiormValue::Int)
                        .unwrap_or(PiormValue::Null)
                }).collect()
            }
            PiormPool::Postgres(p) => {
                let args = postgres_arguments(pk_sea_vals);
                let rows = sqlx::query_with(&pk_sql, args).fetch_all(p).await?;
                rows.iter().map(|r| {
                    r.try_get::<i64, _>(0).map(PiormValue::Int)
                        .unwrap_or(PiormValue::Null)
                }).collect()
            }
        };

        // Delete cascade children for all matched parent PKs.
        for (child_def, fk_attname) in &cascade_children {
            delete_children_by_pk_list(
                pool, backend, &child_def.table_name, fk_attname, &pk_values,
            ).await?;
        }
    }

    // Now delete the parent rows.
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
        PiormPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        PiormPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        PiormPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
    }
}

/// Batch-INSERT rows in chunks of `batch_size`.
/// Returns a `Vec<i64>` of the assigned primary keys (sequential from last_insert_rowid for SQLite).
pub async fn bulk_create_rows(
    rows: Vec<Vec<(String, PiormValue)>>,
    batch_size: usize,
    pool: &PiormPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
    ignore_conflicts: bool,
) -> Result<Vec<i64>> {
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let table = Alias::new(&model_def.table_name);
    let mut all_pks: Vec<i64> = Vec::with_capacity(rows.len());

    for chunk in rows.chunks(batch_size) {
        if chunk.is_empty() {
            continue;
        }
        let first = &chunk[0];
        let col_names: Vec<String> = first
            .iter()
            .map(|(k, _)| resolve_column(&[k.clone()], model_def))
            .collect();

        let mut stmt = Query::insert();
        stmt.into_table(table.clone());
        stmt.columns(col_names.iter().map(|c| Alias::new(c)));

        for row in chunk {
            let vals: Vec<Expr> = row.iter().map(|(_, v)| Expr::val(v.to_sea_value())).collect();
            stmt.values(vals)
                .map_err(|e| PiormError::Other(e.to_string()))?;
        }

        if ignore_conflicts {
            stmt.on_conflict(sea_query::OnConflict::new().do_nothing().to_owned());
        }

        let (sql, sea_vals) = backend.build_insert(&stmt);
        let chunk_len = chunk.len() as i64;

        match pool {
            PiormPool::Sqlite(p) => {
                let args = sqlite_arguments(sea_vals);
                let result = sqlx::query_with(&sql, args).execute(p).await?;
                let last_id = result.last_insert_rowid();
                for pk in (last_id - chunk_len + 1)..=last_id {
                    all_pks.push(pk);
                }
            }
            PiormPool::Mysql(p) => {
                let args = mysql_arguments(sea_vals);
                let result = sqlx::query_with(&sql, args).execute(p).await?;
                let last_id = result.last_insert_id() as i64;
                for pk in (last_id - chunk_len + 1)..=last_id {
                    all_pks.push(pk);
                }
            }
            PiormPool::Postgres(p) => {
                let args = postgres_arguments(sea_vals);
                sqlx::query_with(&sql, args).execute(p).await?;
                // PKs not reliably available without RETURNING; fill with 0.
                for _ in 0..chunk_len {
                    all_pks.push(0);
                }
            }
        }
    }
    Ok(all_pks)
}

/// UPDATE a single row identified by its primary key.
pub async fn update_row_by_pk(
    pk_val: &PiormValue,
    updates: &[(String, PiormValue)],
    pool: &PiormPool,
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
        let col_name = resolve_column(&[field_name.clone()], model_def);
        if let PiormValue::FExpr(node) = value {
            stmt.value(Alias::new(&col_name), fexpr_to_sea_expr(node, &model_def.table_name));
        } else {
            stmt.value(Alias::new(&col_name), value.to_sea_value());
        }
    }

    stmt.cond_where(
        Expr::col(Alias::new(&model_def.pk_field)).eq(pk_val.to_sea_value()),
    );

    let (sql, sea_vals) = backend.build_update(&stmt);

    match pool {
        PiormPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        PiormPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(result.rows_affected())
        }
        PiormPool::Postgres(p) => {
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
    values: &[PiormValue],
    pool: &PiormPool,
    backend: &QueryBackend,
) -> Result<Vec<PiormRow>> {
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
        PiormPool::Sqlite(p) => {
            let args = sqlite_arguments(binds);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(sqlite_row_to_piorm).collect()
        }
        PiormPool::Mysql(p) => {
            let args = mysql_arguments(binds);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(mysql_row_to_piorm).collect()
        }
        PiormPool::Postgres(p) => {
            let args = postgres_arguments(binds);
            let rows = sqlx::query_with(&sql, args).fetch_all(p).await?;
            rows.iter().map(postgres_row_to_piorm).collect()
        }
    }
}

/// Convert an F() expression tree into a sea-query `SimpleExpr` for use in UPDATE SET.
fn fexpr_to_sea_expr(node: &FExprNode, table: &str) -> sea_query::SimpleExpr {
    match node {
        FExprNode::Field(name) => {
            Expr::col((Alias::new(table), Alias::new(name.as_str()))).into()
        }
        FExprNode::Literal(val) => Expr::val(val.to_sea_value()).into(),
        FExprNode::BinOp { lhs, op, rhs } => {
            let l = fexpr_to_sea_expr(lhs, table);
            let r = fexpr_to_sea_expr(rhs, table);
            match op {
                FExprOp::Add => l.add(r),
                FExprOp::Sub => l.sub(r),
                FExprOp::Mul => l.mul(r),
                FExprOp::Div => l.div(r),
                FExprOp::Mod => l.modulo(r),
            }
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

fn lookup_to_expr(col: Alias, lookup: &Lookup, value: &PiormValue) -> sea_query::SimpleExpr {
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
            if matches!(value, PiormValue::Bool(true)) {
                Expr::col(col).is_null()
            } else {
                Expr::col(col).is_not_null()
            }
        }
        Lookup::In => match value {
            PiormValue::List(items) => {
                let sea_vals: Vec<sea_query::Value> =
                    items.iter().map(|v| v.to_sea_value()).collect();
                Expr::col(col).is_in(sea_vals)
            }
            single => Expr::col(col).eq(single.to_sea_value()),
        },
        Lookup::Range => match value {
            PiormValue::List(bounds) if bounds.len() == 2 => {
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
    // FK attr name: user wrote "author", field is stored as "author_id".
    let fk_attname = format!("{}_id", first);
    if let Some(field) = model_def.fields.get(fk_attname.as_str()) {
        return field.column_name();
    }
    for field in model_def.fields.values() {
        if field.column_name() == first {
            return first.to_string();
        }
    }
    first.to_string()
}

fn value_as_str(value: &PiormValue) -> String {
    match value {
        PiormValue::Text(s) => s.clone(),
        PiormValue::Int(i) => i.to_string(),
        PiormValue::Float(f) => f.to_string(),
        _ => String::new(),
    }
}
