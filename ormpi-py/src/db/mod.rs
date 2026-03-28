pub mod connections;
pub use connections::{PyConnectionProxy, PyDatabaseWrapper};

use pyo3::prelude::*;

pub fn register_db(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDatabaseWrapper>()?;
    m.add_class::<PyConnectionProxy>()?;
    Ok(())
}
