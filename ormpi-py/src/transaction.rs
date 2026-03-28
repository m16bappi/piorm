use std::sync::Arc;

use ormpi_core::transaction::{OrmpiTransaction, TransactionState};
use pyo3::prelude::*;

use crate::ormpi_err;

/// Python-facing transaction object.
///
/// The inner transaction is held behind an `Arc<tokio::sync::Mutex<...>>` so
/// async methods can take a clone of the Arc without borrowing `self`.
/// Thread safety is enforced by the Mutex; the Python GIL prevents concurrent
/// Python-side access.
#[pyclass(name = "Transaction")]
pub struct PyTransaction {
    /// The actual sqlx transaction.  Set to None after commit / rollback.
    inner: Arc<tokio::sync::Mutex<Option<OrmpiTransaction>>>,
    /// Callbacks to fire after a successful commit.
    on_commit_callbacks: Vec<Py<PyAny>>,
    /// Alias retained for repr after the transaction is consumed.
    alias: String,
}

impl PyTransaction {
    pub fn new(txn: OrmpiTransaction) -> Self {
        let alias = txn.alias.clone();
        PyTransaction {
            inner: Arc::new(tokio::sync::Mutex::new(Some(txn))),
            on_commit_callbacks: Vec::new(),
            alias,
        }
    }

    /// Returns an Arc clone suitable for passing into async blocks.
    pub fn inner_arc(&self) -> Arc<tokio::sync::Mutex<Option<OrmpiTransaction>>> {
        Arc::clone(&self.inner)
    }
}

#[pymethods]
impl PyTransaction {
    /// Commit the transaction and fire registered `on_commit` callbacks.
    fn commit<'py>(&mut self, py: Python<'py>) -> PyResult<PyObject> {
        let arc = Arc::clone(&self.inner);
        let callbacks = std::mem::take(&mut self.on_commit_callbacks);

        let future = async move {
            let mut guard = arc.lock().await;
            let txn = guard.take().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Transaction already consumed")
            })?;
            txn.commit().await.map_err(ormpi_err)?;

            // Fire on_commit callbacks (needs GIL).
            Python::with_gil(|py| {
                for cb in &callbacks {
                    if let Err(e) = cb.bind(py).call0() {
                        tracing::warn!("on_commit callback failed: {:?}", e);
                    }
                }
                Ok::<_, PyErr>(py.None())
            })
        };

        pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
    }

    /// Roll back the transaction.  `on_commit` callbacks are silently dropped.
    fn rollback<'py>(&mut self, py: Python<'py>) -> PyResult<PyObject> {
        let arc = Arc::clone(&self.inner);
        self.on_commit_callbacks.clear();

        let future = async move {
            let mut guard = arc.lock().await;
            let txn = guard.take().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Transaction already consumed")
            })?;
            txn.rollback().await.map_err(ormpi_err)?;
            Python::with_gil(|py| Ok::<_, PyErr>(py.None()))
        };

        pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
    }

    /// Register a zero-argument callable invoked after a successful commit.
    fn on_commit(&mut self, callback: Py<PyAny>) -> PyResult<()> {
        if self.is_open() {
            self.on_commit_callbacks.push(callback);
            Ok(())
        } else {
            Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Cannot register on_commit: transaction is not open",
            ))
        }
    }

    /// Execute raw SQL on this transaction's connection.
    ///
    /// Used internally by savepoint helpers and exposed for advanced use.
    fn execute_raw<'py>(&self, py: Python<'py>, sql: String) -> PyResult<PyObject> {
        let arc = Arc::clone(&self.inner);

        let future = async move {
            let mut guard = arc.lock().await;
            let txn = guard.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Transaction already consumed")
            })?;
            txn.execute_raw(&sql).await.map_err(ormpi_err)?;
            Python::with_gil(|py| Ok::<_, PyErr>(py.None()))
        };

        pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
    }

    /// `True` while the transaction has not been committed or rolled back.
    #[getter]
    fn is_open(&self) -> bool {
        // Try to get the state without async by using `try_lock`.
        // If the lock is held by an in-flight future this defaults to false.
        if let Ok(guard) = self.inner.try_lock() {
            guard
                .as_ref()
                .map(|t| t.state == TransactionState::Open)
                .unwrap_or(false)
        } else {
            // Lock is held — treat as open (in-flight operation).
            true
        }
    }

    fn __repr__(&self) -> String {
        if self.is_open() {
            format!("<Transaction db={:?} state=Open>", self.alias)
        } else {
            format!("<Transaction db={:?} consumed>", self.alias)
        }
    }
}

// ── begin() function ──────────────────────────────────────────────────────────

/// `await begin(using="default")` — open a new database transaction.
pub fn py_begin(py: Python<'_>, using: Option<String>) -> PyResult<PyObject> {
    let alias = using.unwrap_or_else(|| "default".to_string());
    let pool = {
        let registry = ormpi_core::registry::DatabaseRegistry::global();
        let guard = registry.read().unwrap();
        guard.get_pool(&alias).map_err(ormpi_err)?.clone()
    };

    let future = async move {
        let txn = OrmpiTransaction::begin(&pool, &alias)
            .await
            .map_err(ormpi_err)?;
        Python::with_gil(|py| Py::new(py, PyTransaction::new(txn)).map(|p| p.into_py(py)))
    };

    pyo3_async_runtimes::tokio::future_into_py(py, future).map(|b| b.into())
}
