pub mod base;
pub mod instance;

use pyo3::prelude::*;
use pyo3::types::PyModule;

pub fn register_model(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<base::PyModel>()?;
    m.add_class::<base::PyManager>()?;
    m.add_function(wrap_pyfunction!(base::_register_model_class, m)?)?;
    Ok(())
}
