use ormpi_core::backend::DatabaseBackend;
use pyo3::prelude::*;

use crate::ormpi_err;

/// A handle to a specific database pool, keyed by alias.
#[pyclass(name = "DatabaseWrapper")]
pub struct PyDatabaseWrapper {
    pub alias: String,
}

#[pymethods]
impl PyDatabaseWrapper {
    #[getter]
    fn vendor(&self) -> PyResult<String> {
        let registry = ormpi_core::registry::DatabaseRegistry::global();
        let guard = registry.read().unwrap();
        let config = guard.get_config(&self.alias).map_err(ormpi_err)?;
        Ok(match config.backend {
            DatabaseBackend::Sqlite => "sqlite".to_string(),
            DatabaseBackend::Mysql => "mysql".to_string(),
            DatabaseBackend::Postgres => "postgresql".to_string(),
        })
    }

    #[getter]
    fn alias(&self) -> &str {
        &self.alias
    }

    fn __repr__(&self) -> String {
        format!("<DatabaseWrapper alias={:?}>", self.alias)
    }
}

/// Proxy object that resolves database aliases to `DatabaseWrapper` instances.
///
/// Usage:
///   connections["default"]       → DatabaseWrapper
///   "default" in connections     → True/False
///   connections.all_aliases()    → list[str]
#[pyclass(name = "ConnectionProxy")]
pub struct PyConnectionProxy;

#[pymethods]
impl PyConnectionProxy {
    #[new]
    fn new() -> Self {
        PyConnectionProxy
    }

    fn __getitem__(&self, alias: String) -> PyResult<PyDatabaseWrapper> {
        // Validate the alias exists before returning the wrapper.
        let registry = ormpi_core::registry::DatabaseRegistry::global();
        let guard = registry.read().unwrap();
        guard.get_pool(&alias).map_err(ormpi_err)?;
        Ok(PyDatabaseWrapper { alias })
    }

    fn __contains__(&self, alias: String) -> bool {
        let registry = ormpi_core::registry::DatabaseRegistry::global();
        let guard = registry.read().unwrap();
        guard.pools.contains_key(&alias)
    }

    fn all_aliases(&self) -> Vec<String> {
        let registry = ormpi_core::registry::DatabaseRegistry::global();
        let guard = registry.read().unwrap();
        guard.pools.keys().cloned().collect()
    }

    fn __repr__(&self) -> String {
        "<ConnectionProxy>".to_string()
    }
}
