use std::collections::{HashMap, HashSet};

use sea_query::{Alias, Expr, ExprTrait, Query};
use sqlx::Row;

use crate::backend::QueryBackend;
use crate::bind::{mysql_arguments, postgres_arguments, sqlite_arguments};
use crate::error::OrmpiError;
use crate::fields::{FieldType, OnDelete};
use crate::model::def::ModelDef;
use crate::model::registry::ModelRegistry;
use crate::pool::OrmpiPool;
use crate::value::OrmpiValue;
use crate::Result;

// ── save_instance ─────────────────────────────────────────────────────────────

/// Insert or update a model instance.
///
/// Returns the primary-key value as `OrmpiValue` after the operation so the
/// Python layer can set `instance.pk`.
pub async fn save_instance(
    pool: &OrmpiPool,
    model_def: &ModelDef,
    values: HashMap<String, OrmpiValue>,
    dirty_fields: HashSet<String>,
    is_adding: bool,
    force_insert: bool,
    force_update: bool,
    update_fields: Option<Vec<String>>,
) -> Result<OrmpiValue> {
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
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
    values: &HashMap<String, OrmpiValue>,
) -> Result<OrmpiValue> {
    // Fields to insert: skip auto-pk unless the user explicitly set it.
    let fields_to_insert: Vec<_> = model_def
        .fields
        .values()
        .filter(|f| !matches!(f.field_type, FieldType::ManyToMany { .. }))
        .filter(|f| {
            if f.primary_key && f.auto_increment {
                // Include explicit pk value only when non-null
                matches!(values.get(&f.attname), Some(v) if !matches!(v, OrmpiValue::Null))
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
    use crate::backend::DatabaseBackend;
    let pk_col = Alias::new(&model_def.pk_field);

    match pool {
        OrmpiPool::Sqlite(p) => {
            let mut stmt = Query::insert();
            stmt.into_table(Alias::new(&model_def.table_name))
                .columns(columns)
                .values_panic(row_values);
            let (sql, sea_vals) = backend.build_insert(&stmt);
            let args = sqlite_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(OrmpiValue::Int(result.last_insert_rowid()))
        }
        OrmpiPool::Mysql(p) => {
            let mut stmt = Query::insert();
            stmt.into_table(Alias::new(&model_def.table_name))
                .columns(columns)
                .values_panic(row_values);
            let (sql, sea_vals) = backend.build_insert(&stmt);
            let args = mysql_arguments(sea_vals);
            let result = sqlx::query_with(&sql, args).execute(p).await?;
            Ok(OrmpiValue::Int(result.last_insert_id() as i64))
        }
        OrmpiPool::Postgres(p) => {
            let mut stmt = Query::insert();
            stmt.into_table(Alias::new(&model_def.table_name))
                .columns(columns)
                .values_panic(row_values)
                .returning_col(pk_col);
            let (sql, sea_vals) = backend.build_insert(&stmt);
            let args = postgres_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args).fetch_one(p).await?;
            let id: i64 = row
                .try_get(model_def.pk_field.as_str())
                .unwrap_or_default();
            Ok(OrmpiValue::Int(id))
        }
    }
}

async fn update_instance(
    pool: &OrmpiPool,
    backend: &QueryBackend,
    model_def: &ModelDef,
    values: &HashMap<String, OrmpiValue>,
    dirty_fields: &HashSet<String>,
    update_fields: Option<Vec<String>>,
) -> Result<OrmpiValue> {
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
            .unwrap_or(OrmpiValue::Null));
    }

    let pk_val = values.get(pk_attname).ok_or_else(|| OrmpiError::FieldError {
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

    Ok(pk_val.clone())
}

// ── delete_instance ───────────────────────────────────────────────────────────

/// Find all (child_model_def, fk_column_attname) pairs that CASCADE-delete
/// when the given parent model is deleted.
pub fn find_cascade_children(parent_model_name: &str) -> Vec<(ModelDef, String)> {
    let reg = ModelRegistry::global();
    let guard = reg.read().unwrap();
    let mut children = Vec::new();
    for child_def in guard.all() {
        for field in child_def.fields.values() {
            let (to, on_delete) = match &field.field_type {
                FieldType::ForeignKey { to, on_delete, .. } => (to.as_str(), on_delete),
                FieldType::OneToOne { to, on_delete } => (to.as_str(), on_delete),
                _ => continue,
            };
            if to == parent_model_name && matches!(on_delete, OnDelete::Cascade) {
                children.push((child_def.clone(), field.attname.clone()));
            }
        }
    }
    children
}

/// Find all (child_model_def, fk_column_name) pairs with SET_NULL behaviour
/// that reference the given parent model.
pub fn find_set_null_children(parent_model_name: &str) -> Vec<(ModelDef, String)> {
    let reg = ModelRegistry::global();
    let guard = reg.read().unwrap();
    let mut children = Vec::new();
    for child_def in guard.all() {
        for field in child_def.fields.values() {
            let (to, on_delete) = match &field.field_type {
                FieldType::ForeignKey { to, on_delete, .. } => (to.as_str(), on_delete),
                FieldType::OneToOne { to, on_delete } => (to.as_str(), on_delete),
                _ => continue,
            };
            if to == parent_model_name && matches!(on_delete, OnDelete::SetNull) {
                children.push((child_def.clone(), field.column_name()));
            }
        }
    }
    children
}

/// SET fk_col = NULL on child rows where fk_col = pk_value.
async fn set_null_children_by_pk(
    pool: &OrmpiPool,
    backend: &QueryBackend,
    child_table: &str,
    fk_col: &str,
    pk_value: &OrmpiValue,
) -> Result<usize> {
    use sea_query::{Alias, Expr, Query};
    let stmt = Query::update()
        .table(Alias::new(child_table))
        .value(Alias::new(fk_col), sea_query::Value::Int(None))
        .and_where(Expr::col(Alias::new(fk_col)).eq(pk_value.to_sea_value()))
        .to_owned();
    let (sql, sea_vals) = backend.build_update(&stmt);
    let rows = match pool {
        OrmpiPool::Sqlite(p) => {
            sqlx::query_with(&sql, sqlite_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
        OrmpiPool::Mysql(p) => {
            sqlx::query_with(&sql, mysql_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
        OrmpiPool::Postgres(p) => {
            sqlx::query_with(&sql, postgres_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
    };
    Ok(rows)
}

/// Delete child rows where `fk_col = pk_value` for a single parent PK.
async fn delete_children_by_pk(
    pool: &OrmpiPool,
    backend: &QueryBackend,
    child_table: &str,
    fk_col: &str,
    pk_value: &OrmpiValue,
) -> Result<usize> {
    let stmt = Query::delete()
        .from_table(Alias::new(child_table))
        .and_where(Expr::col(Alias::new(fk_col)).eq(pk_value.to_sea_value()))
        .to_owned();
    let (sql, sea_vals) = backend.build_delete(&stmt);
    let rows = match pool {
        OrmpiPool::Sqlite(p) => {
            sqlx::query_with(&sql, sqlite_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
        OrmpiPool::Mysql(p) => {
            sqlx::query_with(&sql, mysql_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
        OrmpiPool::Postgres(p) => {
            sqlx::query_with(&sql, postgres_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
    };
    Ok(rows)
}

/// Delete child rows where `fk_col IN (pk_values)` for a batch of parent PKs.
pub async fn delete_children_by_pk_list(
    pool: &OrmpiPool,
    backend: &QueryBackend,
    child_table: &str,
    fk_col: &str,
    pk_values: &[OrmpiValue],
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
    let rows = match pool {
        OrmpiPool::Sqlite(p) => {
            sqlx::query_with(&sql, sqlite_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
        OrmpiPool::Mysql(p) => {
            sqlx::query_with(&sql, mysql_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
        OrmpiPool::Postgres(p) => {
            sqlx::query_with(&sql, postgres_arguments(sea_vals))
                .execute(p).await?.rows_affected() as usize
        }
    };
    Ok(rows)
}

/// Delete a single model row by its primary key, cascading to children.
///
/// Returns `(rows_affected, {ModelName: rows_affected})`.
pub async fn delete_instance(
    pool: &OrmpiPool,
    model_def: &ModelDef,
    pk_value: OrmpiValue,
) -> Result<(usize, HashMap<String, usize>)> {
    let backend = QueryBackend(pool.backend());
    let mut counts: HashMap<String, usize> = HashMap::new();

    // SET_NULL: nullify FK on referencing children before deleting parent.
    let set_null_children = find_set_null_children(&model_def.model_name);
    for (child_def, fk_col) in &set_null_children {
        set_null_children_by_pk(pool, &backend, &child_def.table_name, fk_col, &pk_value).await?;
        // SET_NULL doesn't delete rows, so we don't add to counts.
        let _ = child_def; // suppress unused warning
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

    let rows_affected = match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            sqlx::query_with(&sql, args)
                .execute(p)
                .await?
                .rows_affected() as usize
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            sqlx::query_with(&sql, args)
                .execute(p)
                .await?
                .rows_affected() as usize
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            sqlx::query_with(&sql, args)
                .execute(p)
                .await?
                .rows_affected() as usize
        }
    };

    *counts.entry(model_def.model_name.clone()).or_insert(0) += rows_affected;
    Ok((rows_affected, counts))
}

// ── fetch_instance ────────────────────────────────────────────────────────────

/// Fetch a single row by pk and return its column values as a map.
///
/// Used by `refresh_from_db`.
pub async fn fetch_instance(
    pool: &OrmpiPool,
    model_def: &ModelDef,
    pk_value: OrmpiValue,
) -> Result<HashMap<String, serde_json::Value>> {
    use crate::executor::{fetch_row_sqlite, fetch_row_mysql, fetch_row_postgres};

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

    let row_map = match pool {
        OrmpiPool::Sqlite(p) => {
            let args = sqlite_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args)
                .fetch_one(p)
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => OrmpiError::DoesNotExist {
                        model: model_def.model_name.clone(),
                    },
                    other => OrmpiError::Database(other),
                })?;
            fetch_row_sqlite(&row)
        }
        OrmpiPool::Mysql(p) => {
            let args = mysql_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args)
                .fetch_one(p)
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => OrmpiError::DoesNotExist {
                        model: model_def.model_name.clone(),
                    },
                    other => OrmpiError::Database(other),
                })?;
            fetch_row_mysql(&row)
        }
        OrmpiPool::Postgres(p) => {
            let args = postgres_arguments(sea_vals);
            let row = sqlx::query_with(&sql, args)
                .fetch_one(p)
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => OrmpiError::DoesNotExist {
                        model: model_def.model_name.clone(),
                    },
                    other => OrmpiError::Database(other),
                })?;
            fetch_row_postgres(&row)
        }
    };

    Ok(row_map.into_iter().collect())
}
