use pyo3::prelude::*;

use ormpi_core::query::aggregate::AggregateExpr;

/// Python wrapper for aggregate functions used with `.annotate()` and `.aggregate()`.
///
/// Usage:
///   Article.objects.aggregate(n=Count("id"), avg=Avg("views"))
///   Article.objects.annotate(comment_count=Count("comments"))

#[pyclass(name = "Count")]
#[derive(Clone)]
pub struct PyCount {
    pub inner: AggregateExpr,
}

#[pymethods]
impl PyCount {
    #[new]
    #[pyo3(signature = (field = None, distinct = false))]
    fn new(field: Option<String>, distinct: bool) -> Self {
        Self {
            inner: AggregateExpr::Count { field, distinct },
        }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            AggregateExpr::Count { field: None, distinct: false } => "Count(*)".to_string(),
            AggregateExpr::Count { field: Some(f), distinct: true } => {
                format!("Count({f}, distinct=True)")
            }
            AggregateExpr::Count { field: Some(f), .. } => format!("Count({f})"),
            _ => "Count(?)".to_string(),
        }
    }
}

#[pyclass(name = "Sum")]
#[derive(Clone)]
pub struct PySum {
    pub inner: AggregateExpr,
}

#[pymethods]
impl PySum {
    #[new]
    fn new(field: String) -> Self {
        Self { inner: AggregateExpr::Sum(field) }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            AggregateExpr::Sum(f) => format!("Sum({f})"),
            _ => "Sum(?)".to_string(),
        }
    }
}

#[pyclass(name = "Avg")]
#[derive(Clone)]
pub struct PyAvg {
    pub inner: AggregateExpr,
}

#[pymethods]
impl PyAvg {
    #[new]
    fn new(field: String) -> Self {
        Self { inner: AggregateExpr::Avg(field) }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            AggregateExpr::Avg(f) => format!("Avg({f})"),
            _ => "Avg(?)".to_string(),
        }
    }
}

#[pyclass(name = "Max")]
#[derive(Clone)]
pub struct PyMax {
    pub inner: AggregateExpr,
}

#[pymethods]
impl PyMax {
    #[new]
    fn new(field: String) -> Self {
        Self { inner: AggregateExpr::Max(field) }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            AggregateExpr::Max(f) => format!("Max({f})"),
            _ => "Max(?)".to_string(),
        }
    }
}

#[pyclass(name = "Min")]
#[derive(Clone)]
pub struct PyMin {
    pub inner: AggregateExpr,
}

#[pymethods]
impl PyMin {
    #[new]
    fn new(field: String) -> Self {
        Self { inner: AggregateExpr::Min(field) }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            AggregateExpr::Min(f) => format!("Min({f})"),
            _ => "Min(?)".to_string(),
        }
    }
}

/// Extract an `AggregateExpr` from any of the Python aggregate wrapper objects.
pub(crate) fn extract_aggregate_expr(obj: &Bound<'_, PyAny>) -> PyResult<AggregateExpr> {
    if let Ok(c) = obj.extract::<pyo3::PyRef<PyCount>>() {
        return Ok(c.inner.clone());
    }
    if let Ok(s) = obj.extract::<pyo3::PyRef<PySum>>() {
        return Ok(s.inner.clone());
    }
    if let Ok(a) = obj.extract::<pyo3::PyRef<PyAvg>>() {
        return Ok(a.inner.clone());
    }
    if let Ok(m) = obj.extract::<pyo3::PyRef<PyMax>>() {
        return Ok(m.inner.clone());
    }
    if let Ok(m) = obj.extract::<pyo3::PyRef<PyMin>>() {
        return Ok(m.inner.clone());
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "Expected an aggregate function (Count, Sum, Avg, Max, Min)",
    ))
}
