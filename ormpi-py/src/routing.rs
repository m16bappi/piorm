use std::sync::Mutex;

use pyo3::prelude::*;

/// Global router chain — stores instantiated Python router objects.
static ROUTER_CHAIN: Mutex<Vec<Py<PyAny>>> = Mutex::new(vec![]);

/// Replace the global router chain with the given instances.
pub fn set_routers(routers: Vec<Py<PyAny>>) {
    let mut chain = ROUTER_CHAIN.lock().unwrap();
    *chain = routers;
}

/// Import and instantiate routers from dotted-path strings.
///
/// E.g. `"myapp.routers.PrimaryReplicaRouter"` → `import myapp.routers; PrimaryReplicaRouter()`
pub fn load_routers(py: Python<'_>, paths: &[String]) -> PyResult<Vec<Py<PyAny>>> {
    paths
        .iter()
        .map(|path| {
            let mut parts: Vec<&str> = path.rsplitn(2, '.').collect();
            parts.reverse();
            if parts.len() != 2 {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Invalid router path '{path}'. Expected 'module.ClassName'."
                )));
            }
            let module_path = parts[0];
            let class_name = parts[1];
            let module = py.import_bound(module_path)?;
            let cls = module.getattr(class_name)?;
            let instance = cls.call0()?;
            Ok(instance.into())
        })
        .collect()
}

/// Resolve the read database alias for `model_name`.
///
/// If `explicit_using` is provided it always wins.
/// Otherwise walks the router chain; first non-None wins; falls back to "default".
pub fn get_read_alias(model_name: &str, explicit_using: Option<&str>) -> String {
    if let Some(alias) = explicit_using {
        return alias.to_string();
    }
    let chain = ROUTER_CHAIN.lock().unwrap();
    if chain.is_empty() {
        return "default".to_string();
    }
    Python::with_gil(|py| {
        for router in chain.iter() {
            if let Ok(result) = router.bind(py).call_method1("db_for_read", (model_name,)) {
                if let Ok(Some(alias)) = result.extract::<Option<String>>() {
                    return alias;
                }
            }
        }
        "default".to_string()
    })
}

/// Resolve the write database alias for `model_name`.
pub fn get_write_alias(model_name: &str, explicit_using: Option<&str>) -> String {
    if let Some(alias) = explicit_using {
        return alias.to_string();
    }
    let chain = ROUTER_CHAIN.lock().unwrap();
    if chain.is_empty() {
        return "default".to_string();
    }
    Python::with_gil(|py| {
        for router in chain.iter() {
            if let Ok(result) = router.bind(py).call_method1("db_for_write", (model_name,)) {
                if let Ok(Some(alias)) = result.extract::<Option<String>>() {
                    return alias;
                }
            }
        }
        "default".to_string()
    })
}
