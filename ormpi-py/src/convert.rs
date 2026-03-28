use chrono::{Datelike, Timelike};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use ormpi_core::value::OrmpiValue;

pub fn value_to_py(py: Python<'_>, value: &OrmpiValue) -> PyObject {
    match value {
        OrmpiValue::Null => py.None(),
        OrmpiValue::Bool(b) => b.into_py(py),
        OrmpiValue::Int(i) => i.into_py(py),
        OrmpiValue::Float(f) => f.into_py(py),
        OrmpiValue::Text(s) => s.into_py(py),
        OrmpiValue::Bytes(b) => PyBytes::new_bound(py, b).into(),
        OrmpiValue::Uuid(u) => u.to_string().into_py(py),
        OrmpiValue::Decimal(d) => d.to_string().into_py(py),
        OrmpiValue::Date(d) => {
            py.import_bound("datetime")
                .and_then(|m| m.getattr("date"))
                .and_then(|cls| cls.call1((d.year(), d.month(), d.day())))
                .map(|o| o.into())
                .unwrap_or_else(|_| d.to_string().into_py(py))
        }
        OrmpiValue::Time(t) => {
            py.import_bound("datetime")
                .and_then(|m| m.getattr("time"))
                .and_then(|cls| cls.call1((t.hour(), t.minute(), t.second())))
                .map(|o| o.into())
                .unwrap_or_else(|_| t.to_string().into_py(py))
        }
        OrmpiValue::DateTime(dt) => {
            py.import_bound("datetime")
                .and_then(|m| m.getattr("datetime"))
                .and_then(|cls| {
                    cls.call1((
                        dt.year(),
                        dt.month(),
                        dt.day(),
                        dt.hour(),
                        dt.minute(),
                        dt.second(),
                    ))
                })
                .map(|o| o.into())
                .unwrap_or_else(|_| dt.to_string().into_py(py))
        }
        OrmpiValue::DateTimeTz(dt) => {
            // Return as ISO 8601 string with timezone info
            dt.to_rfc3339().into_py(py)
        }
        OrmpiValue::Json(j) => {
            py.import_bound("json")
                .and_then(|m| m.getattr("loads"))
                .and_then(|loads| loads.call1((j.to_string(),)))
                .map(|o| o.into())
                .unwrap_or_else(|_| j.to_string().into_py(py))
        }
    }
}
