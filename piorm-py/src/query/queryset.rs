use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use piorm_core::model::def::OrderSpec;
use piorm_core::model::registry::ModelRegistry;
use piorm_core::query::filter::{FilterNode, parse_filter_kwarg};
use piorm_core::query::state::{QueryState, ResultType, SelectFields};
use piorm_core::row::PiormRow;
use piorm_core::value::PiormValue;

use crate::model::instance::py_to_piorm_value;
use crate::piorm_err;
use crate::query::q::PyQ;

// ── suppress unused-import warnings from helpers ──────────────────────────────
#[allow(unused_imports)]
use piorm_core::query::evaluate::fetch_by_column_in;
#[allow(unused_imports)]
use piorm_core::query::join::find_reverse_relation;

// ── Helper: clone an Option<PyObject> while holding the GIL ──────────────────

fn clone_model_class(py: Python<'_>, mc: &Option<PyObject>) -> Option<PyObject> {
    mc.as_ref().map(|obj| obj.clone_ref(py))
}

/// Async iterable returned by `QuerySet.raw()` / `Manager.raw()`.
#[pyclass(name = "RawQuerySet")]
pub struct PyRawQuerySet {
    sql: String,
    params: Vec<PiormValue>,
    db_alias: String,
    model_class: Option<PyObject>,
    rows: Arc<Mutex<Option<Vec<PyObject>>>>,
    index: Arc<AtomicUsize>,
}

#[pymethods]
impl PyRawQuerySet {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__(&self, py: Python<'_>) -> PyResult<PyObject> {
        let sql = self.sql.clone();
        let params = self.params.clone();
        let db_alias = self.db_alias.clone();
        let model_class = self.model_class.as_ref().map(|m| m.clone_ref(py));
        let rows_arc = self.rows.clone();
        let index_arc = self.index.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Check if rows need fetching (release lock before await).
            let needs_fetch = rows_arc.lock().unwrap().is_none();

            if needs_fetch {
                let pool = {
                    let reg = piorm_core::registry::DatabaseRegistry::global();
                    let guard = reg.read().unwrap();
                    guard.get_pool(&db_alias)
                        .map_err(|e| Python::with_gil(|_| crate::piorm_err(e)))?
                        .clone()
                };
                let fetched = piorm_core::executor::fetch_all_raw(&pool, &sql, &params)
                    .await
                    .map_err(|e| Python::with_gil(|_| crate::piorm_err(e)))?;
                let py_rows: Vec<PyObject> = Python::with_gil(|py| {
                    fetched.iter().map(|row| {
                        match &model_class {
                            Some(cls) => row_to_instance(py, row, cls).unwrap_or_else(|_| py.None()),
                            None => py.None(),
                        }
                    }).collect()
                });
                let mut guard = rows_arc.lock().unwrap();
                if guard.is_none() {
                    *guard = Some(py_rows);
                }
            }

            let idx = index_arc.fetch_add(1, Ordering::SeqCst);
            let guard = rows_arc.lock().unwrap();
            let rows = guard.as_ref().unwrap();
            if idx < rows.len() {
                Ok(Python::with_gil(|py| rows[idx].clone_ref(py)))
            } else {
                Err(pyo3::exceptions::PyStopAsyncIteration::new_err(()))
            }
        })
        .map(|b| b.into())
    }
}

#[pyclass(name = "QuerySet")]
pub struct PyQuerySet {
    pub state: QueryState,
    pub model_class: Option<PyObject>,
}

impl PyQuerySet {
    pub fn new(app_label: String, model_name: String) -> Self {
        Self {
            state: QueryState::new(app_label, model_name),
            model_class: None,
        }
    }

    pub fn new_with_class(app_label: String, model_name: String, model_class: PyObject) -> Self {
        Self {
            state: QueryState::new(app_label, model_name),
            model_class: Some(model_class),
        }
    }
}

// ── Public helpers used by iter.rs ────────────────────────────────────────────

pub(crate) async fn fetch_rows_for_state(state: &QueryState) -> piorm_core::Result<Vec<PiormRow>> {
    use piorm_core::backend::QueryBackend;
    use piorm_core::query::evaluate::evaluate;
    use piorm_core::registry::DatabaseRegistry;

    let alias = crate::routing::get_read_alias(&state.model_name, state.database.as_deref());
    let pool = {
        let reg = DatabaseRegistry::global();
        let guard = reg.read().unwrap();
        guard.get_pool(&alias)?.clone()
    };
    let backend = QueryBackend(pool.backend());
    let model_def = {
        let reg = ModelRegistry::global();
        let guard = reg.read().unwrap();
        guard
            .get(&state.app_label, &state.model_name)
            .ok_or_else(|| {
                piorm_core::PiormError::Other(format!(
                    "Model {}.{} not in registry",
                    state.app_label, state.model_name
                ))
            })?
            .clone()
    };
    evaluate(state, &pool, &backend, &model_def).await
}

pub(crate) fn row_to_instance(
    py: Python<'_>,
    row: &PiormRow,
    model_class: &PyObject,
) -> PyResult<PyObject> {
    // call0() invokes Model.__init__(**{}) which sets up the slots:
    //   _values = {}, _dirty = 0, _adding = True, _db = None
    let instance = model_class.bind(py).call0()?;

    // ONLY allocation: build the populated _values dict and assign to slot.
    let values = PyDict::new_bound(py);
    for (col_name, piorm_val) in &row.values {
        let py_val = crate::convert::value_to_py(py,piorm_val);
        values.set_item(col_name, py_val)?;
    }
    instance.setattr("_values", &values)?;

    // Flat slot writes — no extra allocations.
    instance.setattr("_dirty", 0u64)?;
    instance.setattr("_adding", false)?;
    instance.setattr("_db", py.None())?;

    Ok(instance.into())
}

// ── Helper: build result list from rows ──────────────────────────────────────

fn rows_to_py(
    py: Python<'_>,
    rows: Vec<PiormRow>,
    result_type: &ResultType,
    model_class: &Option<PyObject>,
) -> PyResult<PyObject> {
    match result_type {
        ResultType::Values => {
            let dicts: PyResult<Vec<PyObject>> = rows
                .iter()
                .map(|row| {
                    let d = PyDict::new_bound(py);
                    for (k, v) in &row.values {
                        d.set_item(k, crate::convert::value_to_py(py,v))?;
                    }
                    Ok(d.into())
                })
                .collect();
            Ok(PyList::new_bound(py, dicts?).into())
        }
        ResultType::ValuesList(fields) => {
            let tuples: Vec<PyObject> = rows
                .iter()
                .map(|row| {
                    let vals: Vec<PyObject> = if fields.is_empty() {
                        row.values.values().map(|v| crate::convert::value_to_py(py,v)).collect()
                    } else {
                        fields
                            .iter()
                            .map(|f| crate::convert::value_to_py(py,row.values.get(f).unwrap_or(&PiormValue::Null)))
                            .collect()
                    };
                    pyo3::types::PyTuple::new_bound(py, vals).into()
                })
                .collect();
            Ok(PyList::new_bound(py, tuples).into())
        }
        ResultType::FlatList(field) => {
            let items: Vec<PyObject> = rows
                .iter()
                .map(|row| crate::convert::value_to_py(py,row.values.get(field).unwrap_or(&PiormValue::Null)))
                .collect();
            Ok(PyList::new_bound(py, items).into())
        }
        _ => match model_class {
            Some(cls) => {
                let instances: PyResult<Vec<PyObject>> =
                    rows.iter().map(|row| row_to_instance(py, row, cls)).collect();
                Ok(PyList::new_bound(py, instances?).into())
            }
            None => Ok(PyList::empty_bound(py).into()),
        },
    }
}

// ── select_related / prefetch_related helpers ─────────────────────────────────

/// Split each joined row into a base instance + per-path related value dicts.
/// Columns prefixed with `{path}__` belong to the related model.
fn hydrate_with_select_related(
    py: Python<'_>,
    rows: &[PiormRow],
    model_class: &Option<PyObject>,
    select_related_fields: &[String],
) -> PyResult<Vec<PyObject>> {
    rows.iter()
        .map(|row| {
            // Partition columns: base vs. related.
            let mut base_values = PiormRow {
                columns: vec![],
                values: indexmap::IndexMap::new(),
            };
            // related_col_map: path → list of (stripped_col, value)
            let mut related_cols: std::collections::HashMap<
                String,
                Vec<(String, PiormValue)>,
            > = std::collections::HashMap::new();

            for (col_name, value) in &row.values {
                let mut matched = false;
                for path in select_related_fields {
                    let prefix = format!("{}__", path);
                    if let Some(stripped) = col_name.strip_prefix(&prefix) {
                        related_cols
                            .entry(path.clone())
                            .or_default()
                            .push((stripped.to_string(), value.clone()));
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    base_values.columns.push(col_name.clone());
                    base_values.values.insert(col_name.clone(), value.clone());
                }
            }

            // Build the base instance.
            let instance = match model_class {
                Some(cls) => row_to_instance(py, &base_values, cls)?,
                None => return Ok(py.None()),
            };

            // Attach related value dicts as `_piorm_cache_{path}`.
            for (path, cols) in &related_cols {
                let cache_key = format!("_piorm_cache_{}", path);
                let rel_dict = PyDict::new_bound(py);
                let mut all_null = true;
                for (k, v) in cols {
                    let py_val = crate::convert::value_to_py(py,v);
                    if !py_val.is_none(py) {
                        all_null = false;
                    }
                    rel_dict.set_item(k, py_val)?;
                }
                // If all related columns are NULL the FK was NULL (LEFT JOIN miss).
                if all_null {
                    instance.bind(py).setattr(cache_key.as_str(), py.None())?;
                } else {
                    instance.bind(py).setattr(cache_key.as_str(), &rel_dict)?;
                }
            }

            Ok(instance)
        })
        .collect()
}

/// For each `prefetch_fields` entry, resolve the reverse FK and batch-fetch
/// related rows.  Returns `(lookup_name, related_rows, fk_column)` tuples.
async fn fetch_prefetch_data(
    state: &piorm_core::query::state::QueryState,
    base_rows: &[PiormRow],
) -> PyResult<Vec<(String, Vec<PiormRow>, String)>> {
    use piorm_core::backend::QueryBackend;
    use piorm_core::model::registry::ModelRegistry;
    use piorm_core::query::evaluate::fetch_by_column_in;
    use piorm_core::query::join::find_reverse_relation;
    use piorm_core::registry::DatabaseRegistry;

    let mut result = vec![];

    let db_alias = crate::routing::get_read_alias(&state.model_name, state.database.as_deref());
    let pool = {
        let reg = DatabaseRegistry::global();
        let guard = reg.read().unwrap();
        guard
            .get_pool(&db_alias)
            .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
            .clone()
    };
    let backend = QueryBackend(pool.backend());

    for lookup in &state.prefetch_fields {
        // Resolve reverse relation.
        let (rel_table, fk_col, base_pk) = {
            let reg = ModelRegistry::global();
            let guard = reg.read().unwrap();
            let base_model = guard
                .get(&state.app_label, &state.model_name)
                .ok_or_else(|| {
                    Python::with_gil(|_py| {
                        pyo3::exceptions::PyRuntimeError::new_err("base model not found")
                    })
                })?;

            match find_reverse_relation(&*guard, base_model, lookup) {
                Some((rel_model, fk_col, base_pk)) => {
                    (rel_model.table_name.clone(), fk_col, base_pk)
                }
                None => {
                    return Err(Python::with_gil(|_py| {
                        pyo3::exceptions::PyRuntimeError::new_err(format!(
                            "Cannot resolve prefetch_related '{}' for model '{}'",
                            lookup, state.model_name
                        ))
                    }))
                }
            }
        };

        // Collect base PKs.
        let pk_vals: Vec<piorm_core::value::PiormValue> = base_rows
            .iter()
            .filter_map(|row| row.get(&base_pk).cloned())
            .collect();

        let rel_rows = fetch_by_column_in(&rel_table, &fk_col, &pk_vals, &pool, &backend)
            .await
            .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

        result.push((lookup.clone(), rel_rows, fk_col));
    }

    Ok(result)
}

/// Attach prefetched rows to base instances under `_prefetch_cache_{lookup}`.
/// Groups `rel_rows` by `fk_col` value and matches them to instances by pk.
fn attach_prefetch_cache(
    py: Python<'_>,
    instances: &[PyObject],
    rel_rows: &[PiormRow],
    lookup: &str,
    fk_col: &str,
) -> PyResult<()> {
    use std::collections::HashMap;

    // Serialize an PiormValue to a stable string key for grouping.
    fn ov_key(v: &PiormValue) -> String {
        match v {
            PiormValue::Int(i) => format!("{}", i),
            PiormValue::Float(f) => format!("{}", f),
            PiormValue::Text(s) => s.clone(),
            PiormValue::Bool(b) => format!("{}", b),
            _ => format!("{:?}", v),
        }
    }

    // Group rel_rows by fk value.
    let mut grouped: HashMap<String, Vec<&PiormRow>> = HashMap::new();
    for row in rel_rows {
        if let Some(fk_val) = row.get(fk_col) {
            grouped.entry(ov_key(fk_val)).or_default().push(row);
        }
    }

    let cache_key = format!("_prefetch_cache_{}", lookup);
    for instance in instances {
        let inst_ref = instance.bind(py);

        // Extract pk value as string key.
        let pk_key = inst_ref
            .getattr("_values")
            .ok()
            .and_then(|d| d.get_item("id").ok())
            .and_then(|v| crate::model::instance::py_to_piorm_value(&v).ok())
            .map(|ov| ov_key(&ov))
            .unwrap_or_default();

        let rows_for_inst = grouped.get(&pk_key).cloned().unwrap_or_default();
        let rel_dicts: Vec<PyObject> = rows_for_inst
            .iter()
            .map(|row| {
                let d = PyDict::new_bound(py);
                for (k, v) in &row.values {
                    d.set_item(k, crate::convert::value_to_py(py,v))?;
                }
                Ok(d.into())
            })
            .collect::<PyResult<_>>()?;
        let list = PyList::new_bound(py, rel_dicts);
        inst_ref.setattr(cache_key.as_str(), list)?;
    }

    Ok(())
}

// ── pymethods ─────────────────────────────────────────────────────────────────

#[pymethods]
impl PyQuerySet {
    /// Return a clone of this queryset (no-op at this stage).
    fn all(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        let new_qs = PyQuerySet {
            state: slf.borrow().state.clone(),
            model_class,
        };
        Ok(Py::new(py, new_qs)?.into_py(py))
    }

    /// Return an always-empty queryset by injecting an impossible filter.
    fn none(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        // pk IS NULL → never matches any real row.
        state.filters.filters.push(FilterNode::Lookup {
            field_path: vec!["pk".to_string()],
            lookup: piorm_core::query::filter::Lookup::IsNull,
            value: piorm_core::value::PiormValue::Bool(true),
        });
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Add WHERE conditions — positional Q objects and/or keyword lookups.
    #[pyo3(signature = (*args, **kwargs))]
    fn filter(
        slf: Bound<'_, Self>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);

        // Positional Q objects
        for arg in args.iter() {
            let q = arg.extract::<PyRef<PyQ>>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(
                    "filter() positional arguments must be Q objects",
                )
            })?;
            state
                .filters
                .filters
                .push(FilterNode::Q(q.node.clone()));
        }

        // Keyword filters
        if let Some(kw) = kwargs {
            let reg = ModelRegistry::global();
            let guard = reg.read().unwrap();
            let model_def = guard
                .get(&state.app_label, &state.model_name)
                .ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(format!(
                        "Model {}.{} not found in registry",
                        state.app_label, state.model_name
                    ))
                })?;

            for (k, v) in kw.iter() {
                let key: String = k.extract()?;
                let value = py_to_piorm_value(&v)?;
                let node = parse_filter_kwarg(&key, value, model_def, &guard)
                    .map_err(piorm_err)?;
                state.filters.filters.push(node);
            }
        }

        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Add negated WHERE conditions.
    #[pyo3(signature = (*args, **kwargs))]
    fn exclude(
        slf: Bound<'_, Self>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);

        for arg in args.iter() {
            let q = arg.extract::<PyRef<PyQ>>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(
                    "exclude() positional arguments must be Q objects",
                )
            })?;
            state
                .filters
                .excludes
                .push(FilterNode::Q(q.node.clone()));
        }

        if let Some(kw) = kwargs {
            let reg = ModelRegistry::global();
            let guard = reg.read().unwrap();
            let model_def = guard
                .get(&state.app_label, &state.model_name)
                .ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(format!(
                        "Model {}.{} not found in registry",
                        state.app_label, state.model_name
                    ))
                })?;

            for (k, v) in kw.iter() {
                let key: String = k.extract()?;
                let value = py_to_piorm_value(&v)?;
                let node = parse_filter_kwarg(&key, value, model_def, &guard)
                    .map_err(piorm_err)?;
                state.filters.excludes.push(node);
            }
        }

        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Set ORDER BY.  Prefix with "-" for descending.  Accepts variadic string args.
    #[pyo3(signature = (*fields))]
    fn order_by(slf: Bound<'_, Self>, fields: Vec<String>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.order_by = fields.iter().map(|s| parse_order_spec(s)).collect();
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Enable SELECT DISTINCT.
    fn distinct(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.distinct = true;
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Override the database alias for this query.
    fn using(slf: Bound<'_, Self>, alias: String) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.database = Some(alias);
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Restrict columns to `fields` only.
    #[pyo3(signature = (*fields))]
    fn only(slf: Bound<'_, Self>, fields: Vec<String>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.select_fields = SelectFields::Only(fields);
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Defer loading of the specified columns.
    #[pyo3(signature = (*fields))]
    fn defer(slf: Bound<'_, Self>, fields: Vec<String>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.select_fields = SelectFields::Defer(fields);
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Return the earliest row ordered by `field` ASC.
    fn earliest(slf: Bound<'_, Self>, field: String) -> PyResult<PyObject> {
        let qs = slf.call_method1("order_by", (field,))?;
        qs.call_method0("first").map(|r| r.into())
    }

    /// Return the latest row ordered by `field` DESC.
    fn latest(slf: Bound<'_, Self>, field: String) -> PyResult<PyObject> {
        let desc = format!("-{}", field);
        let qs = slf.call_method1("order_by", (desc,))?;
        qs.call_method0("first").map(|r| r.into())
    }

    /// Execute a raw SQL SELECT and return an async iterable of model instances.
    #[pyo3(signature = (sql, params = None))]
    fn raw(slf: Bound<'_, Self>, sql: String, params: Option<Vec<PyObject>>) -> PyResult<PyObject> {
        let py = slf.py();
        let state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        let db_alias = state.database.clone().unwrap_or_else(|| "default".to_string());

        let piorm_params: Vec<PiormValue> = match params {
            Some(list) => list.iter()
                .map(|obj| crate::model::instance::py_to_piorm_value(obj.bind(py)))
                .collect::<PyResult<Vec<_>>>()?,
            None => vec![],
        };

        let raw_qs = PyRawQuerySet {
            sql,
            params: piorm_params,
            db_alias,
            model_class,
            rows: Arc::new(Mutex::new(None)),
            index: Arc::new(AtomicUsize::new(0)),
        };
        Ok(Py::new(py, raw_qs)?.into_py(py))
    }

    /// Return a QuerySet that evaluates to dicts instead of model instances.
    #[pyo3(signature = (*fields))]
    fn values(slf: Bound<'_, Self>, fields: Vec<String>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.result_type = ResultType::Values;
        if !fields.is_empty() {
            state.select_fields = SelectFields::Only(fields);
        }
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Return a QuerySet that evaluates to a list of tuples (or flat list when flat=True).
    #[pyo3(signature = (*fields, flat = false))]
    fn values_list(slf: Bound<'_, Self>, fields: Vec<String>, flat: bool) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        if flat {
            let field = fields.into_iter().next().unwrap_or_default();
            state.select_fields = SelectFields::Only(vec![field.clone()]);
            state.result_type = ResultType::FlatList(field);
        } else {
            if !fields.is_empty() {
                state.select_fields = SelectFields::Only(fields.clone());
            }
            state.result_type = ResultType::ValuesList(fields);
        }
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Slice or index the queryset.
    fn __getitem__(slf: Bound<'_, Self>, key: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        use pyo3::types::PySlice;
        let py = slf.py();

        if let Ok(idx) = key.extract::<i64>() {
            let mut state = slf.borrow().state.clone();
            let model_class = clone_model_class(py, &slf.borrow().model_class);
            if idx >= 0 {
                state.offset = Some(idx as u64);
            } else {
                // Negative index: reverse ordering and use abs offset
                let abs_idx = (-idx - 1) as u64;
                for o in &mut state.order_by {
                    o.descending = !o.descending;
                }
                state.offset = Some(abs_idx);
            }
            state.limit = Some(1);
            return pyo3_async_runtimes::tokio::future_into_py(py, async move {
                let rows = fetch_rows_for_state(&state)
                    .await
                    .map_err(|e| Python::with_gil(|_| crate::piorm_err(e)))?;
                Python::with_gil(|py| {
                    if rows.is_empty() {
                        return Err(pyo3::exceptions::PyIndexError::new_err("index out of range"));
                    }
                    match &model_class {
                        Some(cls) => row_to_instance(py, &rows[0], cls),
                        None => Ok(py.None()),
                    }
                })
            })
            .map(|b| b.into());
        } else if let Ok(slice) = key.downcast::<PySlice>() {
            let indices = slice.indices(i64::MAX as _)?;
            let mut state = slf.borrow().state.clone();
            let model_class = clone_model_class(py, &slf.borrow().model_class);
            let start = indices.start.max(0) as u64;
            let stop = indices.stop.max(0) as u64;
            if start > 0 {
                state.offset = Some(start);
            }
            if stop > start {
                state.limit = Some(stop - start);
            }
            Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "QuerySet indices must be integers or slices",
            ))
        }
    }

    /// Await the queryset — evaluates and returns a list of model instances (or dicts).
    ///
    /// `__await__` must return an *iterator* (has `__next__`), not just an awaitable.
    /// `future_into_py` may return an asyncio.Future/Task whose `__await__()` returns
    /// a FutureIter (the actual iterator), so we call `.__await__()` on it here.
    fn __await__(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);

        let future = pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rows = fetch_rows_for_state(&state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            // If prefetch_related is requested, batch-fetch each reverse relation.
            let prefetch_data: Vec<(String, Vec<piorm_core::row::PiormRow>, String)> =
                if !state.prefetch_fields.is_empty() {
                    fetch_prefetch_data(&state, &rows).await?
                } else {
                    vec![]
                };

            Python::with_gil(|py| {
                // Hydrate rows: if select_related, split out related columns.
                let instances: Vec<PyObject> = if !state.select_related_fields.is_empty() {
                    hydrate_with_select_related(
                        py,
                        &rows,
                        &model_class,
                        &state.select_related_fields,
                    )?
                } else {
                    // Normal path — reuse existing rows_to_py.
                    let list_obj =
                        rows_to_py(py, rows, &state.result_type, &model_class)?;
                    list_obj.extract::<Vec<PyObject>>(py)?
                };

                // Attach prefetch caches.
                for (lookup, rel_rows, fk_col) in &prefetch_data {
                    attach_prefetch_cache(py, &instances, rel_rows, lookup, fk_col)?;
                }

                let list = PyList::new_bound(py, &instances);
                Ok::<PyObject, PyErr>(list.into())
            })
        })?;

        // Call `future.__await__()` to get the underlying iterator (FutureIter / coroutine).
        future.call_method0("__await__").map(|b| b.into())
    }

    /// Return an async iterator over this queryset.
    fn __aiter__(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        use crate::query::iter::PyAsyncIter;
        let py = slf.py();
        let state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        let iter = PyAsyncIter { state, model_class, cache: None, pos: 0 };
        Ok(Py::new(py, iter)?.into_py(py))
    }

    /// Reverse the current ordering (or default to pk DESC if no ordering is set).
    fn reverse(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        use crate::query::iter::PyAsyncIter;
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        if state.order_by.is_empty() {
            state.order_by = vec![OrderSpec { field: "id".to_string(), descending: true, nulls_last: false, nulls_first: false }];
        } else {
            for o in &mut state.order_by {
                o.descending = !o.descending;
            }
        }
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Return an async iterator, optionally fetching in `chunk_size` batches.
    #[pyo3(signature = (chunk_size = 2000))]
    fn iterator(slf: Bound<'_, Self>, chunk_size: usize) -> PyResult<PyObject> {
        use crate::query::iter::PyAsyncIter;
        let _ = chunk_size; // batch streaming is a future optimisation; fetch all rows
        let py = slf.py();
        let state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        let iter = PyAsyncIter { state, model_class, cache: None, pos: 0 };
        Ok(Py::new(py, iter)?.into_py(py))
    }

    /// Async — return the number of rows matching this queryset.
    fn count(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let state = slf.borrow().state.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            use piorm_core::backend::QueryBackend;
            use piorm_core::query::evaluate::evaluate_count;
            use piorm_core::registry::DatabaseRegistry;

            let alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };
            let count = evaluate_count(&state, &pool, &backend, &model_def)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| Ok(count.into_py(py)))
        })
        .map(|b| b.into())
    }

    /// Async — return True if any rows match this queryset.
    fn exists(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        state.limit = Some(1);
        state.order_by.clear();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rows = fetch_rows_for_state(&state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;
            Python::with_gil(|py| Ok((!rows.is_empty()).into_py(py)))
        })
        .map(|b| b.into())
    }

    /// Async — return the first matching row, or None.
    fn first(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.limit = Some(1);
        if state.order_by.is_empty() {
            state.order_by = vec![OrderSpec {
                field: "id".to_string(),
                descending: false,
                nulls_last: false,
                nulls_first: false,
            }];
        }

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rows = fetch_rows_for_state(&state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                if rows.is_empty() {
                    return Ok(py.None());
                }
                match &model_class {
                    Some(cls) => row_to_instance(py, &rows[0], cls),
                    None => Ok(py.None()),
                }
            })
        })
        .map(|b| b.into())
    }

    /// Async — return the last matching row, or None.
    fn last(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.limit = Some(1);
        if state.order_by.is_empty() {
            state.order_by = vec![OrderSpec {
                field: "id".to_string(),
                descending: true,
                nulls_last: false,
                nulls_first: false,
            }];
        } else {
            // Reverse all orderings.
            for o in &mut state.order_by {
                o.descending = !o.descending;
            }
        }

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rows = fetch_rows_for_state(&state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                if rows.is_empty() {
                    return Ok(py.None());
                }
                match &model_class {
                    Some(cls) => row_to_instance(py, &rows[0], cls),
                    None => Ok(py.None()),
                }
            })
        })
        .map(|b| b.into())
    }

    /// Async — return a single object matching the lookup; raise DoesNotExist or
    /// MultipleObjectsReturned if the result count is != 1.
    #[pyo3(signature = (*args, **kwargs))]
    fn get(
        slf: Bound<'_, Self>,
        args: &Bound<'_, pyo3::types::PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let py = slf.py();
        // Apply filters from args/kwargs, then limit to 2 (to detect MultipleObjectsReturned).
        let filtered_qs = slf.call_method("filter", args, kwargs)?;
        let filtered_qs = filtered_qs.downcast::<PyQuerySet>()?;
        let mut state = filtered_qs.borrow().state.clone();
        let model_class = clone_model_class(py, &filtered_qs.borrow().model_class);
        state.limit = Some(2);
        state.order_by.clear();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rows = fetch_rows_for_state(&state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| match rows.len() {
                0 => {
                    let exc_msg = format!(
                        "{} matching query does not exist.",
                        state.model_name
                    );
                    // Try model-specific DoesNotExist first (e.g. Author.DoesNotExist)
                    if let Some(cls) = &model_class {
                        if let Ok(attr) = cls.bind(py).getattr("DoesNotExist") {
                            if let Ok(exc_type) = attr.downcast_into::<pyo3::types::PyType>() {
                                return Err(PyErr::from_type_bound(exc_type, exc_msg));
                            }
                        }
                    }
                    // Fallback: piorm.exceptions.DoesNotExist
                    let exceptions = py.import_bound("piorm.exceptions")?;
                    let exc_type = exceptions.getattr("DoesNotExist")?
                        .downcast_into::<pyo3::types::PyType>()
                        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("DoesNotExist is not a type"))?;
                    Err(PyErr::from_type_bound(exc_type, exc_msg))
                }
                1 => match &model_class {
                    Some(cls) => row_to_instance(py, &rows[0], cls),
                    None => Ok(py.None()),
                },
                n => {
                    let exc_msg = format!(
                        "get() returned more than one {}: {} objects returned",
                        state.model_name, n
                    );
                    let exceptions = py.import_bound("piorm.exceptions")?;
                    let exc_type = exceptions.getattr("MultipleObjectsReturned")?
                        .downcast_into::<pyo3::types::PyType>()
                        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("MultipleObjectsReturned is not a type"))?;
                    Err(PyErr::from_type_bound(exc_type, exc_msg))
                }
            })
        })
        .map(|b| b.into())
    }

    /// Async — INSERT a new row and return the model instance.
    #[pyo3(signature = (**kwargs))]
    fn create(slf: Bound<'_, Self>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<PyObject> {
        let py = slf.py();
        let state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);

        let field_values: Vec<(String, piorm_core::value::PiormValue)> = match kwargs {
            Some(kw) => kw
                .iter()
                .map(|(k, v)| {
                    let key: String = k.extract()?;
                    let val = crate::model::instance::py_to_piorm_value(&v)?;
                    Ok((key, val))
                })
                .collect::<PyResult<Vec<_>>>()?,
            None => vec![],
        };

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            use piorm_core::backend::QueryBackend;
            use piorm_core::model::registry::ModelRegistry;
            use piorm_core::query::evaluate::create_row;
            use piorm_core::registry::DatabaseRegistry;

            let db_alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&db_alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };

            let pk_val = create_row(&field_values, &pool, &backend, &model_def)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            let mut fetch_state = state.clone();
            fetch_state.filters = piorm_core::query::state::ConditionTree::default();
            fetch_state.filters.filters.push(
                piorm_core::query::filter::FilterNode::Lookup {
                    field_path: vec![model_def.pk_field.clone()],
                    lookup: piorm_core::query::filter::Lookup::Exact,
                    value: piorm_core::value::PiormValue::Int(pk_val),
                },
            );
            fetch_state.limit = Some(1);
            fetch_state.order_by.clear();

            let rows = fetch_rows_for_state(&fetch_state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                if rows.is_empty() {
                    return Ok(py.None());
                }
                match &model_class {
                    Some(cls) => row_to_instance(py, &rows[0], cls),
                    None => Ok(py.None()),
                }
            })
        })
        .map(|b| b.into())
    }

    /// Async bulk UPDATE — apply `kwargs` as SET columns to all rows matching the queryset.
    /// Returns the number of rows affected.
    #[pyo3(signature = (**kwargs))]
    fn update(slf: Bound<'_, Self>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<PyObject> {
        let py = slf.py();
        let state = slf.borrow().state.clone();

        let updates: std::collections::HashMap<String, piorm_core::value::PiormValue> =
            match kwargs {
                Some(kw) => kw
                    .iter()
                    .map(|(k, v)| {
                        let key: String = k.extract()?;
                        let val = crate::model::instance::py_to_piorm_value(&v)?;
                        Ok((key, val))
                    })
                    .collect::<PyResult<_>>()?,
                None => std::collections::HashMap::new(),
            };

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            use piorm_core::backend::QueryBackend;
            use piorm_core::model::registry::ModelRegistry;
            use piorm_core::query::evaluate::bulk_update;
            use piorm_core::registry::DatabaseRegistry;

            let alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };
            let count = bulk_update(&state, &updates, &pool, &backend, &model_def)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| Ok::<PyObject, PyErr>(count.into_py(py)))
        })
        .map(|b| b.into())
    }

    /// Async bulk DELETE — delete all rows matching this queryset.
    /// Returns `(count, {"ModelName": count})`.
    fn delete(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();
        let state = slf.borrow().state.clone();
        let model_name = state.model_name.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            use piorm_core::backend::QueryBackend;
            use piorm_core::model::registry::ModelRegistry;
            use piorm_core::query::evaluate::bulk_delete;
            use piorm_core::registry::DatabaseRegistry;

            let alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };
            let count = bulk_delete(&state, &pool, &backend, &model_def)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                let breakdown = PyDict::new_bound(py);
                breakdown.set_item(&model_name, count)?;
                let tuple = pyo3::types::PyTuple::new_bound(py, [count.into_py(py), breakdown.into()]);
                Ok::<PyObject, PyErr>(tuple.into())
            })
        })
        .map(|b| b.into())
    }

    /// Async — evaluate whole-queryset aggregates; returns a dict of `alias -> value`.
    ///
    /// Example:
    ///   result = await Article.objects.aggregate(n=Count("id"), avg=Avg("views"))
    #[pyo3(signature = (**kwargs))]
    fn aggregate(
        slf: Bound<'_, Self>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        use crate::query::aggregate::extract_aggregate_expr;

        let py = slf.py();
        let mut state = slf.borrow().state.clone();

        if let Some(kw) = kwargs {
            for (k, v) in kw.iter() {
                let alias: String = k.extract()?;
                let expr = extract_aggregate_expr(&v)?;
                state.annotations.insert(alias, expr);
            }
        }

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            use piorm_core::backend::QueryBackend;
            use piorm_core::model::registry::ModelRegistry;
            use piorm_core::query::evaluate::evaluate_aggregate;
            use piorm_core::registry::DatabaseRegistry;

            let alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };
            let result = evaluate_aggregate(&state, &pool, &backend, &model_def)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                let dict = PyDict::new_bound(py);
                for (k, v) in &result {
                    dict.set_item(k, crate::convert::value_to_py(py,v))?;
                }
                Ok::<PyObject, PyErr>(dict.into())
            })
        })
        .map(|b| b.into())
    }

    /// Add per-row annotation(s) — returns a new QuerySet that includes the aggregate
    /// expression(s) as extra columns in each result row.
    ///
    /// Example:
    ///   Article.objects.annotate(comment_count=Count("comments"))
    #[pyo3(signature = (**kwargs))]
    fn annotate(
        slf: Bound<'_, Self>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        use crate::query::aggregate::extract_aggregate_expr;

        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);

        if let Some(kw) = kwargs {
            for (k, v) in kw.iter() {
                let alias: String = k.extract()?;
                let expr = extract_aggregate_expr(&v)?;
                state.annotations.insert(alias, expr);
            }
        }

        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Async — get a single matching object, or create it with `create_defaults` if not found.
    /// Returns `(instance, created: bool)`.
    #[pyo3(signature = (defaults = None, **kwargs))]
    fn get_or_create(
        slf: Bound<'_, Self>,
        defaults: Option<&Bound<'_, PyDict>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let py = slf.py();

        // lookup kwargs are used for both filter and create
        let lookup_vals: Vec<(String, piorm_core::value::PiormValue)> = match kwargs {
            Some(kw) => kw
                .iter()
                .map(|(k, v)| {
                    let key: String = k.extract()?;
                    let val = crate::model::instance::py_to_piorm_value(&v)?;
                    Ok((key, val))
                })
                .collect::<PyResult<Vec<_>>>()?,
            None => vec![],
        };

        let default_vals: Vec<(String, piorm_core::value::PiormValue)> = match defaults {
            Some(d) => d
                .iter()
                .map(|(k, v)| {
                    let key: String = k.extract()?;
                    let val = crate::model::instance::py_to_piorm_value(&v)?;
                    Ok((key, val))
                })
                .collect::<PyResult<Vec<_>>>()?,
            None => vec![],
        };

        let state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Filter by lookup kwargs
            let mut filter_state = state.clone();
            {
                use piorm_core::model::registry::ModelRegistry;
                use piorm_core::query::filter::{FilterNode, Lookup};

                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                let model_def = guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone();

                for (field_name, value) in &lookup_vals {
                    let col = if let Some(f) = model_def.fields.get(field_name.as_str()) {
                        f.attname.clone()
                    } else {
                        field_name.clone()
                    };
                    filter_state.filters.filters.push(FilterNode::Lookup {
                        field_path: vec![col],
                        lookup: Lookup::Exact,
                        value: value.clone(),
                    });
                }
            }
            filter_state.limit = Some(1);

            let rows = fetch_rows_for_state(&filter_state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            if !rows.is_empty() {
                return Python::with_gil(|py| {
                    let instance = match &model_class {
                        Some(cls) => row_to_instance(py, &rows[0], cls)?,
                        None => py.None(),
                    };
                    let tuple = pyo3::types::PyTuple::new_bound(py, [instance, false.into_py(py)]);
                    Ok::<PyObject, PyErr>(tuple.into())
                });
            }

            // Not found — build combined kwargs (lookup + defaults) and create.
            let mut all_fields: Vec<(String, piorm_core::value::PiormValue)> =
                lookup_vals.into_iter().collect();
            for (k, v) in &default_vals {
                // defaults override lookup values
                if !all_fields.iter().any(|(f, _)| f == k) {
                    all_fields.push((k.clone(), v.clone()));
                }
            }

            use piorm_core::backend::QueryBackend;
            use piorm_core::model::registry::ModelRegistry;
            use piorm_core::query::evaluate::create_row;
            use piorm_core::registry::DatabaseRegistry;

            let db_alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&db_alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };

            let pk_val = create_row(&all_fields, &pool, &backend, &model_def)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            let mut fetch_state = state.clone();
            fetch_state.filters = piorm_core::query::state::ConditionTree::default();
            fetch_state.filters.filters.push(piorm_core::query::filter::FilterNode::Lookup {
                field_path: vec![model_def.pk_field.clone()],
                lookup: piorm_core::query::filter::Lookup::Exact,
                value: piorm_core::value::PiormValue::Int(pk_val),
            });
            fetch_state.limit = Some(1);
            fetch_state.order_by.clear();

            let rows = fetch_rows_for_state(&fetch_state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                let instance = if rows.is_empty() {
                    py.None()
                } else {
                    match &model_class {
                        Some(cls) => row_to_instance(py, &rows[0], cls)?,
                        None => py.None(),
                    }
                };
                let tuple = pyo3::types::PyTuple::new_bound(py, [instance, true.into_py(py)]);
                Ok(tuple.into())
            })
        })
        .map(|b| b.into())
    }

    /// Async — update matching rows with `defaults`, or create if none match.
    /// Returns `(instance, created: bool)`.
    #[pyo3(signature = (defaults = None, **kwargs))]
    fn update_or_create(
        slf: Bound<'_, Self>,
        defaults: Option<&Bound<'_, PyDict>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let py = slf.py();

        let lookup_vals: Vec<(String, piorm_core::value::PiormValue)> = match kwargs {
            Some(kw) => kw
                .iter()
                .map(|(k, v)| {
                    let key: String = k.extract()?;
                    let val = crate::model::instance::py_to_piorm_value(&v)?;
                    Ok((key, val))
                })
                .collect::<PyResult<Vec<_>>>()?,
            None => vec![],
        };

        let default_vals: Vec<(String, piorm_core::value::PiormValue)> = match defaults {
            Some(d) => d
                .iter()
                .map(|(k, v)| {
                    let key: String = k.extract()?;
                    let val = crate::model::instance::py_to_piorm_value(&v)?;
                    Ok((key, val))
                })
                .collect::<PyResult<Vec<_>>>()?,
            None => vec![],
        };

        let state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            use piorm_core::backend::QueryBackend;
            use piorm_core::model::registry::ModelRegistry;
            use piorm_core::query::evaluate::{bulk_update, create_row};
            use piorm_core::registry::DatabaseRegistry;

            let db_alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&db_alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };

            // Apply lookup filters to find existing rows.
            let mut filter_state = state.clone();
            for (field_name, value) in &lookup_vals {
                let col = if let Some(f) = model_def.fields.get(field_name.as_str()) {
                    f.attname.clone()
                } else {
                    field_name.clone()
                };
                filter_state.filters.filters.push(
                    piorm_core::query::filter::FilterNode::Lookup {
                        field_path: vec![col],
                        lookup: piorm_core::query::filter::Lookup::Exact,
                        value: value.clone(),
                    },
                );
            }

            // When no defaults are provided, skip the UPDATE entirely (empty SET is a SQL error).
            // Instead, do a SELECT to check if the row exists.
            if default_vals.is_empty() {
                let mut fetch_state = filter_state.clone();
                fetch_state.limit = Some(1);
                let rows = fetch_rows_for_state(&fetch_state)
                    .await
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;
                if !rows.is_empty() {
                    return Python::with_gil(|py| {
                        let instance = match &model_class {
                            Some(cls) => row_to_instance(py, &rows[0], cls)?,
                            None => py.None(),
                        };
                        let tuple =
                            pyo3::types::PyTuple::new_bound(py, [instance, false.into_py(py)]);
                        Ok::<PyObject, PyErr>(tuple.into())
                    });
                }
                // Row not found — fall through to create.
            } else {
                let updates: std::collections::HashMap<String, piorm_core::value::PiormValue> =
                    default_vals.iter().cloned().collect();
                let rows_affected = bulk_update(&filter_state, &updates, &pool, &backend, &model_def)
                    .await
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

                if rows_affected > 0 {
                    // Fetch updated instance.
                    let mut fetch_state = filter_state.clone();
                    fetch_state.limit = Some(1);
                    let rows = fetch_rows_for_state(&fetch_state)
                        .await
                        .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

                    return Python::with_gil(|py| {
                        let instance = if rows.is_empty() {
                            py.None()
                        } else {
                            match &model_class {
                                Some(cls) => row_to_instance(py, &rows[0], cls)?,
                                None => py.None(),
                            }
                        };
                        let tuple =
                            pyo3::types::PyTuple::new_bound(py, [instance, false.into_py(py)]);
                        Ok::<PyObject, PyErr>(tuple.into())
                    });
                }
            }

            // No rows matched — create a new one.
            let mut all_fields: Vec<(String, piorm_core::value::PiormValue)> =
                lookup_vals.into_iter().collect();
            for (k, v) in &default_vals {
                if !all_fields.iter().any(|(f, _)| f == k) {
                    all_fields.push((k.clone(), v.clone()));
                }
            }

            let pk_val = create_row(&all_fields, &pool, &backend, &model_def)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            let mut fetch_state2 = state.clone();
            fetch_state2.filters = piorm_core::query::state::ConditionTree::default();
            fetch_state2.filters.filters.push(piorm_core::query::filter::FilterNode::Lookup {
                field_path: vec![model_def.pk_field.clone()],
                lookup: piorm_core::query::filter::Lookup::Exact,
                value: piorm_core::value::PiormValue::Int(pk_val),
            });
            fetch_state2.limit = Some(1);
            fetch_state2.order_by.clear();

            let rows = fetch_rows_for_state(&fetch_state2)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                let instance = if rows.is_empty() {
                    py.None()
                } else {
                    match &model_class {
                        Some(cls) => row_to_instance(py, &rows[0], cls)?,
                        None => py.None(),
                    }
                };
                let tuple = pyo3::types::PyTuple::new_bound(py, [instance, true.into_py(py)]);
                Ok(tuple.into())
            })
        })
        .map(|b| b.into())
    }

    /// Set FK paths to JOIN in a single query.
    #[pyo3(signature = (*fields))]
    fn select_related(slf: Bound<'_, Self>, fields: Vec<String>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.select_related_fields = fields;
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Set reverse-FK / M2M paths to batch-fetch after the main query.
    #[pyo3(signature = (*fields))]
    fn prefetch_related(slf: Bound<'_, Self>, fields: Vec<String>) -> PyResult<PyObject> {
        let py = slf.py();
        let mut state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);
        state.prefetch_fields = fields;
        Ok(Py::new(py, PyQuerySet { state, model_class })?.into_py(py))
    }

    /// Async — INSERT a list of model instances in batches.
    /// Returns a list of the inserted model instances with PKs populated.
    #[pyo3(signature = (objs, batch_size = None, ignore_conflicts = false))]
    fn bulk_create(
        slf: Bound<'_, Self>,
        objs: Vec<PyObject>,
        batch_size: Option<usize>,
        ignore_conflicts: bool,
    ) -> PyResult<PyObject> {
        let py = slf.py();

        if objs.is_empty() {
            return pyo3_async_runtimes::tokio::future_into_py(py, async move {
                Python::with_gil(|py| {
                    Ok::<PyObject, PyErr>(pyo3::types::PyList::empty_bound(py).into())
                })
            })
            .map(|b| b.into());
        }

        let state = slf.borrow().state.clone();
        let model_class = clone_model_class(py, &slf.borrow().model_class);

        // Extract _values from each object (must happen while we hold the GIL).
        let rows: Vec<Vec<(String, piorm_core::value::PiormValue)>> = objs
            .iter()
            .map(|obj| {
                let bound = obj.bind(py);
                let values = bound.getattr("_values")?;
                let d = values.downcast::<PyDict>().map_err(|_| {
                    pyo3::exceptions::PyTypeError::new_err(
                        "bulk_create: each object must have _values dict",
                    )
                })?;
                d.iter()
                    .map(|(k, v)| {
                        let key: String = k.extract()?;
                        let val = crate::model::instance::py_to_piorm_value(&v)?;
                        Ok((key, val))
                    })
                    .collect::<PyResult<Vec<_>>>()
            })
            .collect::<PyResult<_>>()?;

        let rows_clone = rows.clone();
        let chunk = batch_size.unwrap_or(1000);

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            use piorm_core::backend::QueryBackend;
            use piorm_core::model::registry::ModelRegistry;
            use piorm_core::query::evaluate::bulk_create_rows;
            use piorm_core::registry::DatabaseRegistry;

            let db_alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&db_alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };

            let pks = bulk_create_rows(rows, chunk, &pool, &backend, &model_def, ignore_conflicts)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                let pk_field = model_def.pk_field.clone();
                let instances: Vec<PyObject> = rows_clone
                    .iter()
                    .zip(pks.iter().chain(std::iter::repeat(&0i64)))
                    .map(|(row_vals, &pk)| {
                        let instance = match &model_class {
                            Some(cls) => match cls.bind(py).call0() {
                                Ok(inst) => inst,
                                Err(_) => return py.None(),
                            },
                            None => return py.None(),
                        };
                        if let Ok(values_obj) = instance.getattr("_values") {
                            if let Ok(d) = values_obj.downcast::<PyDict>() {
                                for (k, v) in row_vals {
                                    let _ = d.set_item(k, crate::convert::value_to_py(py,v));
                                }
                                let _ = d.set_item(&pk_field, pk);
                            }
                        }
                        let _ = instance.setattr("_dirty", 0u64);
                        let _ = instance.setattr("_adding", false);
                        instance.into()
                    })
                    .collect();
                Ok::<PyObject, PyErr>(
                    pyo3::types::PyList::new_bound(py, &instances).into(),
                )
            })
        })
        .map(|b| b.into())
    }

    /// Async — UPDATE only the given `fields` on each object in `objs` (matched by pk).
    /// Returns the number of rows updated.
    #[pyo3(signature = (objs, fields, batch_size = None))]
    fn bulk_update(
        slf: Bound<'_, Self>,
        objs: Vec<PyObject>,
        fields: Vec<String>,
        batch_size: Option<usize>,
    ) -> PyResult<PyObject> {
        let py = slf.py();
        let state = slf.borrow().state.clone();

        // Extract pk + field values while holding the GIL.
        // Each element: (pk_value, [(field, value), ...])
        let updates: Vec<(piorm_core::value::PiormValue, Vec<(String, piorm_core::value::PiormValue)>)> = objs
            .iter()
            .map(|obj| {
                let bound = obj.bind(py);
                let values_obj = bound.getattr("_values")?;
                let d = values_obj.downcast::<PyDict>().map_err(|_| {
                    pyo3::exceptions::PyTypeError::new_err(
                        "bulk_update: each object must have _values dict",
                    )
                })?;

                let pk_py = d.get_item("id")?.ok_or_else(|| {
                    pyo3::exceptions::PyValueError::new_err(
                        "bulk_update: object has no 'id' in _values",
                    )
                })?;
                let pk_val = crate::model::instance::py_to_piorm_value(&pk_py)?;

                let field_vals: Vec<(String, piorm_core::value::PiormValue)> = fields
                    .iter()
                    .filter_map(|f| {
                        d.get_item(f.as_str()).ok().flatten().map(|v| {
                            crate::model::instance::py_to_piorm_value(&v)
                                .map(|val| (f.clone(), val))
                        })
                    })
                    .collect::<PyResult<_>>()?;

                Ok((pk_val, field_vals))
            })
            .collect::<PyResult<_>>()?;

        let _chunk = batch_size.unwrap_or(1000);

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            use piorm_core::backend::QueryBackend;
            use piorm_core::model::registry::ModelRegistry;
            use piorm_core::query::evaluate::update_row_by_pk;
            use piorm_core::registry::DatabaseRegistry;

            let db_alias = state.database.clone().unwrap_or_else(|| "default".to_string());
            let pool = {
                let reg = DatabaseRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get_pool(&db_alias)
                    .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?
                    .clone()
            };
            let backend = QueryBackend(pool.backend());
            let model_def = {
                let reg = ModelRegistry::global();
                let guard = reg.read().unwrap();
                guard
                    .get(&state.app_label, &state.model_name)
                    .ok_or_else(|| {
                        Python::with_gil(|_py| {
                            pyo3::exceptions::PyRuntimeError::new_err("model not found")
                        })
                    })?
                    .clone()
            };

            let mut total = 0u64;
            for (pk_val, field_vals) in &updates {
                let affected =
                    update_row_by_pk(pk_val, field_vals, &pool, &backend, &model_def)
                        .await
                        .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;
                total += affected;
            }

            Python::with_gil(|py| Ok::<PyObject, PyErr>(total.into_py(py)))
        })
        .map(|b| b.into())
    }

    fn __repr__(&self) -> String {
        format!(
            "<QuerySet [{}.{}]>",
            self.state.app_label, self.state.model_name
        )
    }
}

fn parse_order_spec(s: &str) -> OrderSpec {
    let descending = s.starts_with('-');
    let field = s.trim_start_matches('-').to_string();
    OrderSpec {
        field,
        descending,
        nulls_last: false,
        nulls_first: false,
    }
}
