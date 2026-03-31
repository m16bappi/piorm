use std::collections::{HashMap, HashSet};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use ormpi_core::model::registry::ModelRegistry;
use ormpi_core::value::{FExprNode, FExprOp, OrmpiValue};

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
    if let Ok(list) = obj.downcast::<PyList>() {
        let items = list
            .iter()
            .map(|item| py_to_ormpi_value(&item))
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(OrmpiValue::List(items));
    }
    if let Ok(tup) = obj.downcast::<PyTuple>() {
        let items = tup
            .iter()
            .map(|item| py_to_ormpi_value(&item))
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(OrmpiValue::List(items));
    }
    if let Ok(b) = obj.extract::<Vec<u8>>() {
        return Ok(OrmpiValue::Bytes(b));
    }
    // Detect F("field") or _FExpr by Python class name.
    let type_name = obj.get_type().name()
        .map(|n| n.extract::<String>().unwrap_or_default())
        .unwrap_or_default();
    if type_name == "_FExpr" {
        if let (Ok(lhs_obj), Ok(op_obj), Ok(rhs_obj)) = (
            obj.getattr("lhs"),
            obj.getattr("op"),
            obj.getattr("rhs"),
        ) {
            let op_s: String = op_obj.extract()?;
            let lhs = py_to_ormpi_value(&lhs_obj)?;
            let rhs = py_to_ormpi_value(&rhs_obj)?;
            let op = match op_s.as_str() {
                "+" => FExprOp::Add,
                "-" => FExprOp::Sub,
                "*" => FExprOp::Mul,
                "/" => FExprOp::Div,
                "%" => FExprOp::Mod,
                other => return Err(pyo3::exceptions::PyValueError::new_err(
                    format!("Unknown F() operator: {}", other)
                )),
            };
            let node = FExprNode::BinOp {
                lhs: Box::new(lhs.into_fexpr_node()),
                op,
                rhs: Box::new(rhs.into_fexpr_node()),
            };
            return Ok(OrmpiValue::FExpr(Box::new(node)));
        }
    }
    if type_name == "F" {
        if let Ok(name_obj) = obj.getattr("name") {
            if let Ok(field_name) = name_obj.extract::<String>() {
                return Ok(OrmpiValue::FExpr(Box::new(FExprNode::Field(field_name))));
            }
        }
    }
    // Model instance passed as FK value — extract pk.
    if let Ok(pk_attr) = obj.getattr("pk") {
        if pk_attr.is_none() {
            return Ok(OrmpiValue::Null);
        }
        if let Ok(i) = pk_attr.extract::<i64>() {
            return Ok(OrmpiValue::Int(i));
        }
    }
    // Fall back to a JSON-string representation for complex types.
    let json_str = obj.str()?.extract::<String>()?;
    Ok(OrmpiValue::Text(json_str))
}

/// Extract `_values` as `HashMap<String, OrmpiValue>` from a model instance.
pub(crate) fn extract_values(
    instance: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, OrmpiValue>> {
    let dict = instance
        .getattr("_values")?
        .downcast_into::<PyDict>()?;
    let mut map = HashMap::new();
    for (k, v) in dict.iter() {
        let key: String = k.extract()?;
        let val = py_to_ormpi_value(&v)?;
        map.insert(key, val);
    }
    Ok(map)
}

/// Extract dirty field names from the `_dirty` integer bitmask on a model instance.
///
/// Reads `_dirty` (u64) from the instance and `_field_bits` (PyDict) from the class,
/// then returns the set of field names whose bit is set.
pub(crate) fn extract_dirty(instance: &Bound<'_, PyAny>) -> PyResult<HashSet<String>> {
    let dirty_mask: u64 = instance.getattr("_dirty")?.extract()?;

    // Fast path: nothing is dirty — skip the class lookup entirely.
    if dirty_mask == 0 {
        return Ok(HashSet::new());
    }

    let field_bits = instance
        .get_type()
        .getattr("_field_bits")?
        .downcast_into::<PyDict>()?;

    let mut dirty = HashSet::new();
    for (name, bit) in field_bits.iter() {
        let bit: u64 = bit.extract()?;
        if dirty_mask & bit != 0 {
            dirty.insert(name.extract::<String>()?);
        }
    }
    Ok(dirty)
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

    // Resolve the write alias via the router chain (explicit `using` always wins).
    let using =
        crate::routing::get_write_alias(&model_def.model_name, using.as_deref());

    // Get the pool before entering the async block (avoids non-Send guard).
    let pool = crate::get_pool_for_alias(&using)?;

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
            // Write pk back into _values
            let values_dict = instance
                .getattr("_values")?
                .downcast_into::<PyDict>()?;
            values_dict.set_item(&model_def.pk_field, value_to_py(py, &pk))?;
            // Clear dirty bitmask, flip adding flag, record db alias.
            instance.setattr("_dirty", 0u64)?;
            instance.setattr("_adding", false)?;
            instance.setattr("_db", &using)?;
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

    let class_name = class_name_of(&slf)?;
    let model_def = get_model_def_by_name(&class_name).ok_or_else(|| {
        pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Model '{}' is not registered",
            class_name
        ))
    })?;

    let using = crate::routing::get_write_alias(&model_def.model_name, using.as_deref());

    let values = extract_values(&slf)?;
    let pk_val = values
        .get(&model_def.pk_field)
        .cloned()
        .unwrap_or(OrmpiValue::Null);

    let pool = crate::get_pool_for_alias(&using)?;

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

    let pool = crate::get_pool_for_alias(&using)?;

    let slf_py = slf.into_py(py);

    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        let fresh = ormpi_core::model::executor::fetch_instance(&pool, &model_def, pk_val)
            .await
            .map_err(crate::ormpi_err)?;

        Python::with_gil(|py| {
            let instance = slf_py.bind(py);
            let values_dict = instance
                .getattr("_values")?
                .downcast_into::<PyDict>()?;

            for (col, json_val) in &fresh {
                // Determine if this field should be refreshed.
                if let Some(ref field_names) = fields {
                    if !field_names.contains(col) {
                        continue;
                    }
                }
                let ormpi_val = json_val_to_ormpi(json_val.clone());
                let py_val = value_to_py(py, &ormpi_val);
                values_dict.set_item(col, py_val)?;
            }
            // Clear dirty bitmask after refresh.
            instance.setattr("_dirty", 0u64)?;
            Ok(py.None())
        })
    })
    .map(|b| b.into())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_is_adding(instance: &Bound<'_, PyAny>) -> PyResult<bool> {
    match instance.getattr("_adding") {
        Ok(v) => v.extract::<bool>().or(Ok(true)),
        Err(_) => Ok(true), // conservative default during construction
    }
}

/// Convert a `serde_json::Value` to `OrmpiValue` so that `refresh_from_db`
/// uses the same `OrmpiValue → PyObject` conversion path as `row_to_instance`.
fn json_val_to_ormpi(val: serde_json::Value) -> OrmpiValue {
    match val {
        serde_json::Value::Null => OrmpiValue::Null,
        serde_json::Value::Bool(b) => OrmpiValue::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                OrmpiValue::Int(i)
            } else if let Some(f) = n.as_f64() {
                OrmpiValue::Float(f)
            } else {
                OrmpiValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => OrmpiValue::Text(s),
        other => OrmpiValue::Json(other),
    }
}
