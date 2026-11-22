use std::collections::{HashMap, HashSet};

use sea_query::{Alias, Expr, ExprTrait, Query};

use crate::backend::{DatabaseBackend, QueryBackend};
use crate::error::PiormError;
use crate::fields::{FieldType, OnDelete};
use crate::model::def::ModelDef;
use crate::model::registry::ModelRegistry;
use crate::pool::PiormPool;
use crate::value::PiormValue;
use crate::Result;

// ── save_instance ─────────────────────────────────────────────────────────────

/// Insert or update a model instance.
///
/// Returns the primary-key value as `PiormValue` after the operation so the
/// Python layer can set `instance.pk`.
pub async fn save_instance(
    pool: &PiormPool,
    model_def: &ModelDef,
    values: HashMap<String, PiormValue>,
    dirty_fields: HashSet<String>,
    is_adding: bool,
    force_insert: bool,
    force_update: bool,
    update_fields: Option<Vec<String>>,
) -> Result<PiormValue> {
    let backend = QueryBackend(pool.backend());

    if is_adding || force_insert {
        insert_instance(pool, &backend, model_def, &values).await
    } else {
        update_instance(
            pool,
            &backend,
            model_def,
            &values,
            &dirty_fields,
            update_fields,
        )
        .await
    }
}

async fn insert_instance(
    pool: &PiormPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
    values: &HashMap<String, PiormValue>,
) -> Result<PiormValue> {
    // Fields to insert: skip auto-pk unless the user explicitly set it.
    let fields_to_insert: Vec<_> = model_def
        .fields
        .values()
        .filter(|f| !matches!(f.field_type, FieldType::ManyToMany { .. }))
        .filter(|f| {
            if f.primary_key && f.auto_increment {
                // Include explicit pk value only when non-null
                matches!(values.get(&f.attname), Some(v) if !matches!(v, PiormValue::Null))
            } else {
                true
            }
        })
        .collect();

    let columns: Vec<Alias> = fields_to_insert
        .iter()
        .map(|f| Alias::new(&f.column_name()))
        .collect();

    let row_values: Vec<Expr> = fields_to_insert
        .iter()
        .map(|f| {
            let val = if let Some(v) = values.get(&f.attname) {
                v.to_sea_value()
            } else {
                // Use the field's declared default when the value isn't set.
                use crate::fields::DefaultValue;
                match &f.default {
                    DefaultValue::Value(v) => v.to_sea_value(),
                    DefaultValue::EmptyString => {
                        sea_query::Value::String(Some(String::new()))
                    }
                    _ => sea_query::Value::Int(None),
                }
            };
            Expr::val(val)
        })
        .collect();

    // Postgres uses RETURNING to get the generated pk; others use last_insert_id.
    let pk_col = Alias::new(&model_def.pk_field);

    if matches!(pool.backend(), DatabaseBackend::Postgres) {
        let mut stmt = Query::insert();
        stmt.into_table(Alias::new(&model_def.table_name))
            .columns(columns)
            .values_panic(row_values)
            .returning_col(pk_col);
        let (sql, sea_vals) = backend.build_insert(&stmt);
        let row = pool.fetch_one(&sql, sea_vals).await?;
        let id: i64 = row
            .get(model_def.pk_field.as_str())
            .and_then(|v| if let PiormValue::Int(i) = v { Some(*i) } else { None })
            .ok_or_else(|| PiormError::FieldError {
                model: model_def.model_name.clone(),
                field: model_def.pk_field.clone(),
                message: format!(
                    "failed to read generated primary key from RETURNING clause"
                ),
            })?;
        Ok(PiormValue::Int(id))
    } else {
        let mut stmt = Query::insert();
        stmt.into_table(Alias::new(&model_def.table_name))
            .columns(columns)
            .values_panic(row_values);
        let (sql, sea_vals) = backend.build_insert(&stmt);
        let id = pool.insert_last_id(&sql, sea_vals).await?;
        Ok(PiormValue::Int(id))
    }
}

async fn update_instance(
    pool: &PiormPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
    values: &HashMap<String, PiormValue>,
    dirty_fields: &HashSet<String>,
    update_fields: Option<Vec<String>>,
) -> Result<PiormValue> {
    let pk_attname = &model_def.pk_field;

    let fields_to_update: Vec<_> = model_def
        .fields
        .values()
        .filter(|f| !f.primary_key)
        .filter(|f| !matches!(f.field_type, FieldType::ManyToMany { .. }))
        .filter(|f| {
            if let Some(ref uf) = update_fields {
                uf.contains(&f.attname)
            } else {
                dirty_fields.contains(&f.attname)
            }
        })
        .collect();

    // Nothing dirty — return the current pk without hitting the database.
    if fields_to_update.is_empty() {
        return Ok(values
            .get(pk_attname)
            .cloned()
            .unwrap_or(PiormValue::Null));
    }

    let pk_val = values.get(pk_attname).ok_or_else(|| PiormError::FieldError {
        model: model_def.model_name.clone(),
        field: pk_attname.clone(),
        message: "pk is None — cannot UPDATE a model without a primary key".to_string(),
    })?;

    let mut stmt = Query::update();
    stmt.table(Alias::new(&model_def.table_name));
    for f in &fields_to_update {
        let val = values
            .get(&f.attname)
            .map(|v| v.to_sea_value())
            .unwrap_or(sea_query::Value::Int(None));
        stmt.value(Alias::new(&f.column_name()), val);
    }
    stmt.and_where(
        Expr::col(Alias::new(pk_attname)).eq(pk_val.to_sea_value()),
    );

    let (sql, sea_vals) = backend.build_update(&stmt);
    pool.execute(&sql, sea_vals).await?;
    Ok(pk_val.clone())
}

// ── delete_instance ───────────────────────────────────────────────────────────

/// Walk every model in the registry and collect (child_def, field_name) pairs
/// where the field is a FK/O2O referencing `parent_model_name` and the
/// `predicate` returns true for that field's `on_delete` policy.
///
/// `name_fn` extracts the column identifier to store alongside each child def.
fn find_children_matching(
    parent_model_name: &str,
    predicate: impl Fn(&OnDelete) -> bool,
    name_fn: impl Fn(&crate::fields::FieldDef) -> String,
) -> Vec<(ModelDef, String)> {
    let reg = ModelRegistry::global();
    let guard = reg.read().unwrap();
    let mut children = Vec::new();

    for child_def in guard.all() {
        for field in child_def.fields.values() {
            let (to, on_delete) = match &field.field_type {
                FieldType::ForeignKey { to, on_delete, .. } => (to.as_str(), on_delete),
                FieldType::OneToOne { to, on_delete }       => (to.as_str(), on_delete),
                _ => continue,
            };
            if to == parent_model_name && predicate(on_delete) {
                children.push((child_def.clone(), name_fn(field)));
            }
        }
    }
    children
}

/// Find all (child_model_def, fk_column_attname) pairs that CASCADE-delete
/// when the given parent model is deleted.
pub fn find_cascade_children(parent_model_name: &str) -> Vec<(ModelDef, String)> {
    find_children_matching(
        parent_model_name,
        |od| matches!(od, OnDelete::Cascade),
        |f| f.attname.clone(),
    )
}

/// Find all (child_model_def, fk_column_name) pairs with SET_NULL behaviour
/// that reference the given parent model.
pub fn find_set_null_children(parent_model_name: &str) -> Vec<(ModelDef, String)> {
    find_children_matching(
        parent_model_name,
        |od| matches!(od, OnDelete::SetNull),
        |f| f.column_name(),
    )
}

/// SET fk_col = NULL on child rows where fk_col = pk_value.
async fn set_null_children_by_pk(
    pool: &PiormPool,
    backend: &QueryBackend,
    child_table: &str,
    fk_col: &str,
    pk_value: &PiormValue,
) -> Result<usize> {
    use sea_query::{Alias, Expr, Query};
    let stmt = Query::update()
        .table(Alias::new(child_table))
        .value(Alias::new(fk_col), sea_query::Value::Int(None))
        .and_where(Expr::col(Alias::new(fk_col)).eq(pk_value.to_sea_value()))
        .to_owned();
    let (sql, sea_vals) = backend.build_update(&stmt);
    let rows = pool.execute(&sql, sea_vals).await? as usize;
    Ok(rows)
}

/// Delete child rows where `fk_col = pk_value` for a single parent PK.
async fn delete_children_by_pk(
    pool: &PiormPool,
    backend: &QueryBackend,
    child_table: &str,
    fk_col: &str,
    pk_value: &PiormValue,
) -> Result<usize> {
    let stmt = Query::delete()
        .from_table(Alias::new(child_table))
        .and_where(Expr::col(Alias::new(fk_col)).eq(pk_value.to_sea_value()))
        .to_owned();
    let (sql, sea_vals) = backend.build_delete(&stmt);
    let rows = pool.execute(&sql, sea_vals).await? as usize;
    Ok(rows)
}

/// Delete child rows where `fk_col IN (pk_values)` for a batch of parent PKs.
pub async fn delete_children_by_pk_list(
    pool: &PiormPool,
    backend: &QueryBackend,
    child_table: &str,
    fk_col: &str,
    pk_values: &[PiormValue],
) -> Result<usize> {
    if pk_values.is_empty() {
        return Ok(0);
    }
    let sea_vals: Vec<sea_query::Value> = pk_values.iter().map(|v| v.to_sea_value()).collect();
    let stmt = Query::delete()
        .from_table(Alias::new(child_table))
        .and_where(Expr::col(Alias::new(fk_col)).is_in(sea_vals))
        .to_owned();
    let (sql, sea_vals) = backend.build_delete(&stmt);
    let rows = pool.execute(&sql, sea_vals).await? as usize;
    Ok(rows)
}

/// Delete a single model row by its primary key, cascading to children.
///
/// Returns `(rows_affected, {ModelName: rows_affected})`.
pub async fn delete_instance(
    pool: &PiormPool,
    model_def: &ModelDef,
    pk_value: PiormValue,
) -> Result<(usize, HashMap<String, usize>)> {
    let backend = QueryBackend(pool.backend());
    let mut counts: HashMap<String, usize> = HashMap::new();

    // SET_NULL: nullify FK on referencing children before deleting parent.
    let set_null_children = find_set_null_children(&model_def.model_name);
    for (child_def, fk_col) in &set_null_children {
        set_null_children_by_pk(pool, &backend, &child_def.table_name, fk_col, &pk_value).await?;
        // SET_NULL doesn't delete rows, so we don't add to counts.
    }

    // Cascade: delete children first.
    let children = find_cascade_children(&model_def.model_name);
    for (child_def, fk_attname) in &children {
        let deleted = delete_children_by_pk(
            pool, &backend, &child_def.table_name, fk_attname, &pk_value,
        ).await?;
        if deleted > 0 {
            *counts.entry(child_def.model_name.clone()).or_insert(0) += deleted;
        }
    }

    // Delete the parent row.
    let pk_col = Alias::new(&model_def.pk_field);
    let stmt = Query::delete()
        .from_table(Alias::new(&model_def.table_name))
        .and_where(Expr::col(pk_col).eq(pk_value.to_sea_value()))
        .to_owned();

    let (sql, sea_vals) = backend.build_delete(&stmt);
    let rows_affected = pool.execute(&sql, sea_vals).await? as usize;

    *counts.entry(model_def.model_name.clone()).or_insert(0) += rows_affected;
    Ok((rows_affected, counts))
}

// ── fetch_instance ────────────────────────────────────────────────────────────

/// Fetch a single row by pk and return its column values as a map.
///
/// Used by `refresh_from_db`.
pub async fn fetch_instance(
    pool: &PiormPool,
    model_def: &ModelDef,
    pk_value: PiormValue,
) -> Result<HashMap<String, serde_json::Value>> {
    let backend = QueryBackend(pool.backend());
    let pk_col = Alias::new(&model_def.pk_field);
    let table = Alias::new(&model_def.table_name);

    let stmt = Query::select()
        .column(sea_query::Asterisk)
        .from(table)
        .and_where(Expr::col(pk_col).eq(pk_value.to_sea_value()))
        .limit(1)
        .to_owned();

    let (sql, sea_vals) = backend.build_select(&stmt);

    let row = pool.fetch_one(&sql, sea_vals).await.map_err(|e| match e {
        PiormError::Database(sqlx::Error::RowNotFound) => PiormError::DoesNotExist {
            model: model_def.model_name.clone(),
        },
        other => other,
    })?;

    Ok(row.values.into_iter().map(|(k, v)| (k, piorm_value_to_json(v))).collect())
}

fn piorm_value_to_json(v: PiormValue) -> serde_json::Value {
    match v {
        PiormValue::Null => serde_json::Value::Null,
        PiormValue::Bool(b) => serde_json::Value::Bool(b),
        PiormValue::Int(i) => serde_json::Value::Number(i.into()),
        PiormValue::Float(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        PiormValue::Text(s) => serde_json::Value::String(s),
        PiormValue::Bytes(b) => serde_json::Value::Array(
            b.into_iter().map(|byte| serde_json::Value::Number(byte.into())).collect(),
        ),
        PiormValue::Date(d) => serde_json::Value::String(d.to_string()),
        PiormValue::Time(t) => serde_json::Value::String(t.to_string()),
        PiormValue::DateTime(dt) => serde_json::Value::String(dt.to_string()),
        PiormValue::DateTimeTz(dt) => serde_json::Value::String(dt.to_rfc3339()),
        PiormValue::Uuid(u) => serde_json::Value::String(u.to_string()),
        PiormValue::Json(j) => j,
        PiormValue::Decimal(d) => serde_json::Value::String(d.to_string()),
        PiormValue::List(items) => {
            serde_json::Value::Array(items.into_iter().map(piorm_value_to_json).collect())
        }
        PiormValue::FExpr(_) => serde_json::Value::Null,
    }
}
