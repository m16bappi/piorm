pub mod aggregate;
pub mod iter;
pub mod q;
pub mod queryset;

use pyo3::prelude::*;
use pyo3::types::PyModule;

pub fn register_query(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<queryset::PyQuerySet>()?;
    m.add_class::<queryset::PyRawQuerySet>()?;
    m.add_class::<q::PyQ>()?;
    m.add_class::<iter::PyAsyncIter>()?;
    m.add_class::<aggregate::PyCount>()?;
    m.add_class::<aggregate::PySum>()?;
    m.add_class::<aggregate::PyAvg>()?;
    m.add_class::<aggregate::PyMax>()?;
    m.add_class::<aggregate::PyMin>()?;
    Ok(())
}
