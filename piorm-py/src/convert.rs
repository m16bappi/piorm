use chrono::{Datelike, Timelike};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use piorm_core::value::PiormValue;

pub fn value_to_py(py: Python<'_>, value: &PiormValue) -> PyObject {
    match value {
        PiormValue::Null => py.None(),
        PiormValue::Bool(b) => b.into_py(py),
        PiormValue::Int(i) => i.into_py(py),
        PiormValue::Float(f) => f.into_py(py),
        PiormValue::Text(s) => s.into_py(py),
        PiormValue::Bytes(b) => PyBytes::new_bound(py, b).into(),
        PiormValue::Uuid(u) => u.to_string().into_py(py),
        PiormValue::Decimal(d) => d.to_string().into_py(py),
        PiormValue::Date(d) => {
            py.import_bound("datetime")
                .and_then(|m| m.getattr("date"))
                .and_then(|cls| cls.call1((d.year(), d.month(), d.day())))
                .map(|o| o.into())
                .unwrap_or_else(|_| d.to_string().into_py(py))
        }
        PiormValue::Time(t) => {
            py.import_bound("datetime")
                .and_then(|m| m.getattr("time"))
                .and_then(|cls| cls.call1((t.hour(), t.minute(), t.second())))
                .map(|o| o.into())
                .unwrap_or_else(|_| t.to_string().into_py(py))
        }
        PiormValue::DateTime(dt) => {
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
                        dt.nanosecond() / 1000, // microseconds
                    ))
                })
                .map(|o| o.into())
                .unwrap_or_else(|_| dt.to_string().into_py(py))
        }
        PiormValue::DateTimeTz(dt) => {
            // Return as ISO 8601 string with timezone info
            dt.to_rfc3339().into_py(py)
        }
        PiormValue::Json(j) => {
            py.import_bound("json")
                .and_then(|m| m.getattr("loads"))
                .and_then(|loads| loads.call1((j.to_string(),)))
                .map(|o| o.into())
                .unwrap_or_else(|_| j.to_string().into_py(py))
        }
        PiormValue::List(items) => {
            let py_list: Vec<PyObject> = items.iter().map(|v| value_to_py(py, v)).collect();
            py_list.into_py(py)
        }
        PiormValue::FExpr(_) => py.None(),
    }
}
