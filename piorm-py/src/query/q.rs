use pyo3::prelude::*;
use pyo3::types::PyDict;

use piorm_core::query::filter::{Lookup, QNode};

use crate::model::instance::py_to_piorm_value;

/// Python Q object — composes filter conditions with AND / OR / NOT.
///
/// Usage:
///   Q(title="hello")
///   Q(title__icontains="world") | Q(views__gt=100)
///   ~Q(active=False)
#[pyclass(name = "Q")]
pub struct PyQ {
    pub node: QNode,
}

impl PyQ {
    pub fn new(node: QNode) -> Self {
        Self { node }
    }
}

#[pymethods]
impl PyQ {
    /// Create a Q object from keyword arguments.
    #[new]
    #[pyo3(signature = (**kwargs))]
    fn py_new(kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let kw = kwargs.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("Q() requires at least one keyword argument")
        })?;

        let mut nodes: Vec<QNode> = Vec::new();
        for (k, v) in kw.iter() {
            let key: String = k.extract()?;
            let value = py_to_piorm_value(&v)?;
            let (field_parts, lookup) = split_key(&key);
            nodes.push(QNode::Leaf {
                field_path: field_parts,
                lookup,
                value,
            });
        }

        let node = if nodes.len() == 1 {
            nodes.remove(0)
        } else {
            QNode::And(nodes)
        };

        Ok(Self { node })
    }

    /// `Q(...) | Q(...)` → OR node.
    fn __or__(&self, py: Python<'_>, other: &Bound<'_, PyQ>) -> PyResult<PyObject> {
        let node = QNode::Or(vec![self.node.clone(), other.borrow().node.clone()]);
        Ok(Py::new(py, PyQ::new(node))?.into_py(py))
    }

    /// `Q(...) & Q(...)` → AND node.
    fn __and__(&self, py: Python<'_>, other: &Bound<'_, PyQ>) -> PyResult<PyObject> {
        let node = QNode::And(vec![self.node.clone(), other.borrow().node.clone()]);
        Ok(Py::new(py, PyQ::new(node))?.into_py(py))
    }

    /// `~Q(...)` → NOT node.
    fn __invert__(&self, py: Python<'_>) -> PyResult<PyObject> {
        let node = QNode::Not(Box::new(self.node.clone()));
        Ok(Py::new(py, PyQ::new(node))?.into_py(py))
    }

    fn __repr__(&self) -> String {
        format!("<Q: {:?}>", self.node)
    }
}

/// Split "field__lookup" into (field_parts, Lookup).
pub(crate) fn split_key(key: &str) -> (Vec<String>, Lookup) {
    let parts: Vec<&str> = key.split("__").collect();
    match parts.last() {
        Some(last) if Lookup::from_str(last).is_some() => {
            let lookup = Lookup::from_str(last).unwrap();
            let field_parts = parts[..parts.len() - 1]
                .iter()
                .map(|s| s.to_string())
                .collect();
            (field_parts, lookup)
        }
        _ => (parts.iter().map(|s| s.to_string()).collect(), Lookup::Exact),
    }
}
