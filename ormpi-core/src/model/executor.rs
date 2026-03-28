use std::collections::{HashMap, HashSet};

use sea_query::{Alias, Expr, ExprTrait, Query};
use sqlx::Row;

use crate::backend::QueryBackend;
use crate::bind::{mysql_arguments, postgres_arguments, sqlite_arguments};
use crate::error::OrmpiError;
use crate::fields::FieldType;
use crate::model::def::ModelDef;
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

/// Delete a single model row by its primary key.
///
/// Returns `(rows_affected, {ModelName: rows_affected})`.
pub async fn delete_instance(
    pool: &OrmpiPool,
    model_def: &ModelDef,
    pk_value: OrmpiValue,
) -> Result<(usize, HashMap<String, usize>)> {
    let backend = QueryBackend(pool.backend());
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

    let mut counts = HashMap::new();
    counts.insert(model_def.model_name.clone(), rows_affected);
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
