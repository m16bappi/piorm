use std::collections::{HashMap, HashSet};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PySet, PyTuple};

use ormpi_core::model::registry::ModelRegistry;
use ormpi_core::value::OrmpiValue;

use crate::convert::value_to_py;

/// Look up the `ModelDef` for a given Python class name.
pub(crate) fn get_model_def_by_name(
    class_name: &str,
) -> Option<ormpi_core::model::def::ModelDef> {
    let reg = ModelRegistry::global();
    let guard = reg.read().unwrap();
    guard.get_by_name(class_name).cloned()
}

/// Convert a Python object to `OrmpiValue`.
pub(crate) fn py_to_ormpi_value(obj: &Bound<'_, PyAny>) -> PyResult<OrmpiValue> {
    if obj.is_none() {
        return Ok(OrmpiValue::Null);
    }
    // bool must come before int — Python bool is a subclass of int
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(OrmpiValue::Bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(OrmpiValue::Int(i));
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(OrmpiValue::Float(f));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(OrmpiValue::Text(s));
    }
    if let Ok(b) = obj.extract::<Vec<u8>>() {
        return Ok(OrmpiValue::Bytes(b));
    }
    // Fall back to a JSON-string representation for complex types.
    let json_str = obj.str()?.extract::<String>()?;
    Ok(OrmpiValue::Text(json_str))
}

/// Extract `_ormpi_values` as `HashMap<String, OrmpiValue>` from a model instance.
pub(crate) fn extract_values(
    instance: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, OrmpiValue>> {
    let dict = instance
        .getattr("_ormpi_values")?
        .downcast_into::<PyDict>()?;
    let mut map = HashMap::new();
    for (k, v) in dict.iter() {
        let key: String = k.extract()?;
        let val = py_to_ormpi_value(&v)?;
        map.insert(key, val);
    }
    Ok(map)
}

/// Extract `_ormpi_dirty` as `HashSet<String>` from a model instance.
pub(crate) fn extract_dirty(instance: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    let set = instance
        .getattr("_ormpi_dirty")?
        .downcast_into::<PySet>()?;
    let mut hs = HashSet::new();
    for item in set.iter() {
        hs.insert(item.extract::<String>()?);
    }
    Ok(hs)
}

/// Return the Python class name for a model instance.
pub(crate) fn class_name_of(instance: &Bound<'_, PyAny>) -> PyResult<String> {
    instance
        .get_type()
        .name()
        .map(|s| s.to_string())
}

// ── save() ────────────────────────────────────────────────────────────────────

/// `await instance.save(using="default", force_insert=False, force_update=False)`
pub fn save(
    slf: Bound<'_, PyAny>,
    using: Option<String>,
    force_insert: bool,
    force_update: bool,
    update_fields: Option<Vec<String>>,
) -> PyResult<PyObject> {
    let py = slf.py();
    let using = using.unwrap_or_else(|| "default".to_string());

    // Extract everything we need before entering the async block.
    let values = extract_values(&slf)?;
    let dirty = extract_dirty(&slf)?;
    let is_adding = get_is_adding(&slf)?;

    let class_name = class_name_of(&slf)?;
    let model_def = get_model_def_by_name(&class_name).ok_or_else(|| {
        pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Model '{}' is not registered — did you forget to inherit from Model?",
            class_name
        ))
    })?;

    // Get the pool before entering the async block (avoids non-Send guard).
    let pool = {
        let reg = ormpi_core::registry::DatabaseRegistry::global();
        let guard = reg.read().unwrap();
        guard.get_pool(&using).map_err(crate::ormpi_err)?.clone()
    };

    let slf_py = slf.into_py(py);

    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        let pk = ormpi_core::model::executor::save_instance(
            &pool,
            &model_def,
            values,
            dirty,
            is_adding,
            force_insert,
            force_update,
            update_fields,
        )
        .await
        .map_err(crate::ormpi_err)?;

        Python::with_gil(|py| {
            let instance = slf_py.bind(py);
            // Write pk back into _ormpi_values
            let values_dict = instance
                .getattr("_ormpi_values")?
                .downcast_into::<PyDict>()?;
            values_dict.set_item(&model_def.pk_field, value_to_py(py, &pk))?;
            // Clear dirty set
            instance
                .getattr("_ormpi_dirty")?
                .call_method0("clear")?;
            // Update state: adding=False, db=using
            let state = instance
                .getattr("_ormpi_state")?
                .downcast_into::<PyDict>()?;
            state.set_item("adding", false)?;
            state.set_item("db", &using)?;
            Ok(py.None())
        })
    })
    .map(|b| b.into())
}

// ── delete() ─────────────────────────────────────────────────────────────────

/// `await instance.delete(using="default")`
///
/// Returns `(rows_deleted, {"ModelName": rows_deleted})`.
pub fn delete(slf: Bound<'_, PyAny>, using: Option<String>) -> PyResult<PyObject> {
    let py = slf.py();
    let using = using.unwrap_or_else(|| "default".to_string());

    let class_name = class_name_of(&slf)?;
    let model_def = get_model_def_by_name(&class_name).ok_or_else(|| {
        pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Model '{}' is not registered",
            class_name
        ))
    })?;

    let values = extract_values(&slf)?;
    let pk_val = values
        .get(&model_def.pk_field)
        .cloned()
        .unwrap_or(OrmpiValue::Null);

    let pool = {
        let reg = ormpi_core::registry::DatabaseRegistry::global();
        let guard = reg.read().unwrap();
        guard.get_pool(&using).map_err(crate::ormpi_err)?.clone()
    };

    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        let (rows, counts) =
            ormpi_core::model::executor::delete_instance(&pool, &model_def, pk_val)
                .await
                .map_err(crate::ormpi_err)?;

        Python::with_gil(|py| -> PyResult<PyObject> {
            let counts_dict = PyDict::new_bound(py);
            for (k, v) in &counts {
                counts_dict.set_item(k, v)?;
            }
            let result = PyTuple::new_bound(py, [rows.into_py(py), counts_dict.into_py(py)]);
            Ok(result.into_py(py))
        })
    })
    .map(|b| b.into())
}

// ── refresh_from_db() ─────────────────────────────────────────────────────────

/// `await instance.refresh_from_db(using="default", fields=None)`
pub fn refresh_from_db(
    slf: Bound<'_, PyAny>,
    using: Option<String>,
    fields: Option<Vec<String>>,
) -> PyResult<PyObject> {
    let py = slf.py();
    let using = using.unwrap_or_else(|| "default".to_string());

    let class_name = class_name_of(&slf)?;
    let model_def = get_model_def_by_name(&class_name).ok_or_else(|| {
        pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Model '{}' is not registered",
            class_name
        ))
    })?;

    let values = extract_values(&slf)?;
    let pk_val = values
        .get(&model_def.pk_field)
        .cloned()
        .unwrap_or(OrmpiValue::Null);

    let pool = {
        let reg = ormpi_core::registry::DatabaseRegistry::global();
        let guard = reg.read().unwrap();
        guard.get_pool(&using).map_err(crate::ormpi_err)?.clone()
    };

    let slf_py = slf.into_py(py);

    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        let fresh = ormpi_core::model::executor::fetch_instance(&pool, &model_def, pk_val)
            .await
            .map_err(crate::ormpi_err)?;

        Python::with_gil(|py| {
            let instance = slf_py.bind(py);
            let values_dict = instance
                .getattr("_ormpi_values")?
                .downcast_into::<PyDict>()?;

            for (col, json_val) in &fresh {
                // Determine if this field should be refreshed.
                if let Some(ref field_names) = fields {
                    if !field_names.contains(col) {
                        continue;
                    }
                }
                let py_val = json_to_py(py, json_val.clone());
                values_dict.set_item(col, py_val)?;
            }
            // Clear dirty after refresh.
            instance
                .getattr("_ormpi_dirty")?
                .call_method0("clear")?;
            Ok(py.None())
        })
    })
    .map(|b| b.into())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_is_adding(instance: &Bound<'_, PyAny>) -> PyResult<bool> {
    // Default to True if _ormpi_state doesn't exist yet.
    match instance.getattr("_ormpi_state") {
        Ok(state) => {
            if let Ok(dict) = state.downcast_into::<PyDict>() {
                match dict.get_item("adding")? {
                    Some(v) => v.extract::<bool>(),
                    None => Ok(true),
                }
            } else {
                Ok(true)
            }
        }
        Err(_) => Ok(true),
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
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            val.to_string().into_py(py)
        }
    }
}
