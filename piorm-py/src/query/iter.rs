use pyo3::prelude::*;

use piorm_core::query::state::QueryState;

use super::queryset::{fetch_rows_for_state, row_to_instance};

/// Async iterator that evaluates a QuerySet lazily on first `__anext__` call.
#[pyclass(name = "QuerySetIterator")]
pub struct PyAsyncIter {
    pub state: QueryState,
    pub model_class: Option<PyObject>,
    /// Cached evaluated list (as Python list), set on first `__anext__`.
    pub cache: Option<PyObject>,
    pub pos: usize,
}

#[pymethods]
impl PyAsyncIter {
    fn __aiter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    fn __anext__(slf: Bound<'_, Self>) -> PyResult<PyObject> {
        let py = slf.py();

        // If already evaluated, return the next cached item (or StopAsyncIteration).
        {
            let borrow = slf.borrow();
            if let Some(ref cache) = borrow.cache {
                let pos = borrow.pos;
                let list = cache.bind(py);
                let list = list.downcast::<pyo3::types::PyList>()?;
                if pos >= list.len() {
                    return Err(pyo3::exceptions::PyStopAsyncIteration::new_err(()));
                }
                let item = list.get_item(pos)?.into_py(py);
                drop(borrow); // release before mut borrow
                slf.borrow_mut().pos += 1;
                // Wrap in a coroutine that immediately returns the item.
                return pyo3_async_runtimes::tokio::future_into_py(py, async move {
                    Python::with_gil(|py| Ok(item.clone_ref(py)))
                })
                .map(|b| b.into());
            }
        }

        // Not yet evaluated — run the query now.
        let state = slf.borrow().state.clone();
        let model_class = slf
            .borrow()
            .model_class
            .as_ref()
            .map(|obj| obj.clone_ref(py));
        let self_py: PyObject = slf.into_py(py);

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rows = fetch_rows_for_state(&state)
                .await
                .map_err(|e| Python::with_gil(|_py| crate::piorm_err(e)))?;

            Python::with_gil(|py| {
                // Build a Python list from the rows.
                let items: PyResult<Vec<PyObject>> = match &model_class {
                    Some(cls) => rows.iter().map(|row| row_to_instance(py, row, cls)).collect(),
                    None => {
                        // Return dicts if no model class (e.g. values() mode).
                        rows.iter()
                            .map(|row| {
                                let d = pyo3::types::PyDict::new_bound(py);
                                for (k, v) in &row.values {
                                    d.set_item(k, crate::convert::value_to_py(py, v))?;
                                }
                                Ok(d.into())
                            })
                            .collect()
                    }
                };
                let items = items?;

                let list = pyo3::types::PyList::new_bound(py, &items);

                // Store cache and advance pos.
                {
                    let self_bound = self_py.bind(py);
                    let mut iter = self_bound.downcast::<PyAsyncIter>()?.borrow_mut();
                    iter.cache = Some(list.clone().into());
                    iter.pos = 0;
                }

                if items.is_empty() {
                    return Err(pyo3::exceptions::PyStopAsyncIteration::new_err(()));
                }

                let first = items[0].clone_ref(py);
                self_py.bind(py).downcast::<PyAsyncIter>()?.borrow_mut().pos = 1;
                Ok(first)
            })
        })
        .map(|b| b.into())
    }
}
