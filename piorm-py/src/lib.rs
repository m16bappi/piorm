pub(crate) mod convert;
pub mod db;
pub mod fields;
pub mod model;
pub mod query;
pub mod routing;
mod runtime;

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

// ── PiormError → PyErr (function, avoids orphan rule) ────────────────────────

pub(crate) fn piorm_err(err: piorm_core::PiormError) -> PyErr {
    use piorm_core::PiormError::*;
    use pyo3::exceptions::*;
    match &err {
        DoesNotExist { .. } => PyLookupError::new_err(err.to_string()),
        MultipleObjectsReturned { .. } => PyValueError::new_err(err.to_string()),
        ValidationError(_) | FieldError { .. } | UnknownField { .. } | InvalidLookup(_) => {
            PyValueError::new_err(err.to_string())
        }
        _ => PyRuntimeError::new_err(err.to_string()),
    }
}

// ── Pool lookup helper ────────────────────────────────────────────────────────

/// Look up a pool by alias from the global registry.
/// Converts `PiormError` into `PyErr` so it can be used with `?` in `PyResult` contexts.
pub(crate) fn get_pool_for_alias(alias: &str) -> PyResult<piorm_core::pool::PiormPool> {
    let registry = piorm_core::registry::DatabaseRegistry::global();
    let guard = registry.read().unwrap();
    guard.get_pool(alias).map_err(piorm_err).map(|p| p.clone())
}

// ── Newtype for returning rows to Python ──────────────────────────────────────

struct Row(Vec<(String, serde_json::Value)>);

impl IntoPy<PyObject> for Row {
    fn into_py(self, py: Python<'_>) -> PyObject {
        let list = PyList::empty_bound(py);
        for (name, val) in self.0 {
            let val_py = json_to_py(py, val);
            let tup = PyTuple::new_bound(py, [name.into_py(py), val_py]);
            list.append(&tup).unwrap();
        }
        list.into()
    }
}

fn json_to_py(py: Python<'_>, val: serde_json::Value) -> PyObject {
    match val {
        serde_json::Value::Null => py.None(),
        serde_json::Value::Bool(b) => b.into_py(py),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py(py)
            } else if let Some(f) = n.as_f64() {
                f.into_py(py)
            } else {
                n.to_string().into_py(py)
            }
        }
        serde_json::Value::String(s) => s.into_py(py),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty_bound(py);
            for item in arr {
                list.append(json_to_py(py, item)).unwrap();
            }
            list.into()
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new_bound(py);
            for (k, v) in map {
                dict.set_item(k, json_to_py(py, v)).unwrap();
            }
            dict.into()
        }
    }
}

// ── Python functions ──────────────────────────────────────────────────────────

#[pyfunction]
fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// configure(databases: dict, database_routers=None, auto_sync=False) -> Awaitable[None]
#[pyfunction]
#[pyo3(signature = (databases, database_routers = None, auto_sync = false))]
fn configure(
    py: Python<'_>,
    databases: &Bound<'_, PyDict>,
    database_routers: Option<Vec<String>>,
    auto_sync: bool,
) -> PyResult<PyObject> {
    let configs = parse_databases_dict(py, databases)?;

    // Validate that a "default" alias is present.
    if !configs.iter().any(|c| c.alias == "default") {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "DATABASES must contain a 'default' entry",
        ));
    }

    // Load router instances while we still hold the GIL.
    let router_instances = match database_routers.as_deref() {
        Some(paths) if !paths.is_empty() => routing::load_routers(py, paths)?,
        _ => vec![],
    };
    routing::set_routers(router_instances);

    // Snapshot all registered model defs now (while GIL is held).
    // Models imported after configure() is called will NOT be synced.
    let all_defs: Vec<_> = if auto_sync {
        let reg = piorm_core::model::registry::ModelRegistry::global();
        let guard = reg.read().unwrap();
        guard.all().cloned().collect()
    } else {
        vec![]
    };

    let future = async move {
        // 1. Initialize connection pools.
        piorm_core::registry::initialize(configs)
            .await
            .map_err(piorm_err)?;

        // 2. Optionally create all registered model tables (CREATE TABLE IF NOT EXISTS).
        if auto_sync {
            if all_defs.is_empty() {
                Python::with_gil(|py| {
                    let _ = py.run_bound(
                        "import warnings; warnings.warn(\
                            'auto_sync=True was set but no models are registered. \
                            Import your models before calling configure().', \
                            stacklevel=3\
                        )",
                        None,
                        None,
                    );
                });
            } else {
                let pool = get_pool_for_alias("default")?;
                let backend = piorm_core::backend::QueryBackend(pool.backend());

                for model_def in &all_defs {
                    let ddl = piorm_core::model::ddl::model_to_ddl(model_def, backend);
                    apply_ddl_for_model(&pool, &ddl).await?;
                }
            }
        }

        Python::with_gil(|py| Ok(py.None()))
    };
    pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
}

/// close() -> Awaitable[None]  — gracefully close all database pools
#[pyfunction]
fn close(py: Python<'_>) -> PyResult<PyObject> {
    let future = async move {
        let pools: Vec<_> = {
            let registry = piorm_core::registry::DatabaseRegistry::global();
            let guard = registry.read().unwrap();
            guard.pools.values().cloned().collect()
        };
        let close_futs: Vec<_> = pools.iter().map(|p| p.close()).collect();
        futures::future::join_all(close_futs).await;
        Python::with_gil(|py| Ok(py.None()))
    };
    pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
}

/// execute_sql(sql, alias="default") -> Awaitable[int]  — for DDL/DML without rows
#[pyfunction]
#[pyo3(signature = (sql, alias = "default"))]
fn execute_sql(py: Python<'_>, sql: String, alias: &str) -> PyResult<PyObject> {
    let alias = alias.to_string();
    let pool = get_pool_for_alias(&alias)?;
    let future = async move {
        piorm_core::executor::execute_sql(&pool, &sql)
            .await
            .map_err(piorm_err)
    };
    pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
}

/// execute_raw(sql, params=None, alias="default") -> Awaitable[list[list[Any]]]
///
/// Executes raw SQL with optional positional parameters.
/// Returns a list of rows (each row is a list of column values) for SELECT statements,
/// or an empty list for DDL/DML statements.
#[pyfunction]
#[pyo3(signature = (sql, params = None, alias = "default"))]
fn execute_raw(
    py: Python<'_>,
    sql: String,
    params: Option<Vec<PyObject>>,
    alias: &str,
) -> PyResult<PyObject> {
    let alias = alias.to_string();
    let pool = get_pool_for_alias(&alias)?;

    // Convert Python params to PiormValue while we hold the GIL.
    let piorm_params: Vec<piorm_core::value::PiormValue> = params
        .unwrap_or_default()
        .iter()
        .map(|p| crate::model::instance::py_to_piorm_value(p.bind(py)))
        .collect::<PyResult<_>>()?;

    let future = async move {
        let rows = piorm_core::executor::fetch_all_raw(&pool, &sql, &piorm_params)
            .await
            .map_err(piorm_err)?;

        Python::with_gil(|py| {
            let outer = PyList::empty_bound(py);
            for row in &rows {
                let inner = PyList::empty_bound(py);
                for col in &row.columns {
                    let val = row
                        .values
                        .get(col)
                        .unwrap_or(&piorm_core::value::PiormValue::Null);
                    inner.append(crate::convert::value_to_py(py, val)).unwrap();
                }
                outer.append(&inner).unwrap();
            }
            Ok::<PyObject, PyErr>(outer.into())
        })
    };
    pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
}

/// run_cli(args) -> int
///
/// Run the piorm CLI with the given argument list (excluding the program name).
/// Returns the exit code. Raises RuntimeError on failure.
///
/// Example:
///   from piorm._core import run_cli
///   run_cli(["migrate", "--database-url", "sqlite://./db.sqlite3"])
#[pyfunction]
fn run_cli(args: Vec<String>) -> PyResult<i32> {
    use clap::Parser;

    // Prepend a fake program name so clap can parse the args list correctly.
    let mut argv = vec!["piorm".to_string()];
    argv.extend(args);

    let cli = piorm_cli::Cli::try_parse_from(&argv)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    rt.block_on(piorm_cli::run_cli(cli))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Execute all DDL statements for a single model against the given pool.
/// Skips FK constraint statements that start with `--` (SQLite comments).
async fn apply_ddl_for_model(
    pool: &piorm_core::pool::PiormPool,
    ddl: &piorm_core::model::ddl::DdlStatements,
) -> PyResult<()> {
    piorm_core::executor::execute_sql(pool, &ddl.create_table)
        .await
        .map_err(piorm_err)?;

    for sql in &ddl.create_indexes {
        piorm_core::executor::execute_sql(pool, sql)
            .await
            .map_err(piorm_err)?;
    }

    for sql in &ddl.create_m2m_tables {
        piorm_core::executor::execute_sql(pool, sql)
            .await
            .map_err(piorm_err)?;
    }

    for sql in &ddl.create_fk_constraints {
        if !sql.starts_with("--") {
            piorm_core::executor::execute_sql(pool, sql)
                .await
                .map_err(piorm_err)?;
        }
    }

    Ok(())
}

/// create_table(model_key, alias="default") -> Awaitable[None]
///
/// Generate DDL for the named model and execute it against the configured
/// database.  `model_key` is `"app_label.ModelName"` or just `"ModelName"`.
#[pyfunction]
#[pyo3(signature = (model_key, alias = "default"))]
fn create_table(py: Python<'_>, model_key: String, alias: &str) -> PyResult<PyObject> {
    let alias = alias.to_string();

    // Resolve the model def while we still hold the GIL.
    let model_def = {
        let reg = piorm_core::model::registry::ModelRegistry::global();
        let guard = reg.read().unwrap();
        let maybe = if model_key.contains('.') {
            let mut parts = model_key.splitn(2, '.');
            let app = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            guard.get(app, name).cloned()
        } else {
            guard.get_by_name(&model_key).cloned()
        };
        maybe.ok_or_else(|| {
            pyo3::exceptions::PyLookupError::new_err(format!(
                "No model registered for key {:?}",
                model_key
            ))
        })?
    };

    let future = async move {
        let pool = Python::with_gil(|_| get_pool_for_alias(&alias))?;
        let backend = piorm_core::backend::QueryBackend(pool.backend());
        let ddl = piorm_core::model::ddl::model_to_ddl(&model_def, backend);
        apply_ddl_for_model(&pool, &ddl).await?;

        Python::with_gil(|py| Ok(py.None()))
    };

    pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
}

/// create_all_tables(alias="default") -> Awaitable[None]
///
/// Generate and execute DDL for every registered model.
#[pyfunction]
#[pyo3(signature = (alias = "default"))]
fn create_all_tables(py: Python<'_>, alias: &str) -> PyResult<PyObject> {
    let alias = alias.to_string();

    let all_defs: Vec<_> = {
        let reg = piorm_core::model::registry::ModelRegistry::global();
        let guard = reg.read().unwrap();
        guard.all().cloned().collect()
    };

    let future = async move {
        let pool = Python::with_gil(|_| get_pool_for_alias(&alias))?;
        let backend = piorm_core::backend::QueryBackend(pool.backend());

        for model_def in &all_defs {
            let ddl = piorm_core::model::ddl::model_to_ddl(model_def, backend);
            apply_ddl_for_model(&pool, &ddl).await?;
        }

        Python::with_gil(|py| Ok(py.None()))
    };

    pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(get_version, m)?)?;
    m.add_function(wrap_pyfunction!(configure, m)?)?;
    m.add_function(wrap_pyfunction!(close, m)?)?;
    m.add_function(wrap_pyfunction!(execute_raw, m)?)?;
    m.add_function(wrap_pyfunction!(execute_sql, m)?)?;
    m.add_function(wrap_pyfunction!(create_table, m)?)?;
    m.add_function(wrap_pyfunction!(create_all_tables, m)?)?;
    m.add_function(wrap_pyfunction!(run_cli, m)?)?;
    fields::register_fields(m)?;
    model::register_model(m)?;
    query::register_query(m)?;
    db::register_db(m)?;
    Ok(())
}

// ── Python dict parsing helpers ───────────────────────────────────────────────

fn parse_databases_dict(
    py: Python<'_>,
    databases: &Bound<'_, PyDict>,
) -> PyResult<Vec<piorm_core::config::DatabaseConfig>> {
    let mut configs = Vec::new();
    for (alias_obj, db_obj) in databases.iter() {
        let alias: String = alias_obj.extract()?;
        let db_dict = db_obj.downcast::<PyDict>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "databases[{alias:?}] must be a dict"
            ))
        })?;
        let map = pydict_to_json_map(py, db_dict)?;
        let cfg = piorm_core::config::DatabaseConfig::from_map(&alias, &map)
            .map_err(piorm_err)?;
        configs.push(cfg);
    }
    Ok(configs)
}

fn pydict_to_json_map(
    _py: Python<'_>,
    d: &Bound<'_, PyDict>,
) -> PyResult<HashMap<String, serde_json::Value>> {
    let mut map = HashMap::new();
    for (k, v) in d.iter() {
        let key: String = k.extract()?;
        let val = pyobject_to_json(&v)?;
        map.insert(key, val);
    }
    Ok(map)
}

fn pyobject_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(serde_json::Value::Bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(serde_json::Value::Number(i.into()));
    }
    if let Ok(f) = obj.extract::<f64>() {
        let n = serde_json::Number::from_f64(f).unwrap_or_else(|| serde_json::Number::from(0));
        return Ok(serde_json::Value::Number(n));
    }
    if let Ok(d) = obj.downcast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in d.iter() {
            let key: String = k.extract()?;
            let val = pyobject_to_json(&v)?;
            map.insert(key, val);
        }
        return Ok(serde_json::Value::Object(map));
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        return Ok(serde_json::Value::String(s.extract::<String>()?));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(serde_json::Value::String(s));
    }
    Ok(serde_json::Value::String(obj.str()?.extract::<String>()?))
}
