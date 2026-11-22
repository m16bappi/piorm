use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple, PyType};

use piorm_core::fields::{DefaultValue, FieldDef, FieldType};
use piorm_core::model::def::{M2MFieldDef, ModelDef, OrderSpec};
use piorm_core::model::registry::ModelRegistry;

// ── Manager ──────────────────────────────────────────────────────────────────

/// Manager attached to each model as `Model.objects`.
/// Delegates to `QuerySet`; full query methods live in Phase 4.
#[pyclass(name = "Manager")]
pub struct PyManager {
    app_label: String,
    model_name: String,
    model_class: PyObject,
}

impl PyManager {
    pub fn new(app_label: String, model_name: String, model_class: PyObject) -> Self {
        Self {
            app_label,
            model_name,
            model_class,
        }
    }
}

#[pymethods]
impl PyManager {
    fn __repr__(&self) -> String {
        format!("<Manager for {}.{}>", self.app_label, self.model_name)
    }

    #[getter]
    fn model_name(&self) -> &str {
        &self.model_name
    }

    #[getter]
    fn app_label(&self) -> &str {
        &self.app_label
    }

    fn get_queryset(&self, py: Python<'_>) -> PyResult<PyObject> {
        let qs = crate::query::queryset::PyQuerySet::new_with_class(
            self.app_label.clone(),
            self.model_name.clone(),
            self.model_class.clone_ref(py),
        );
        Ok(Py::new(py, qs)?.into_py(py))
    }

    fn all(&self, py: Python<'_>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py).call_method0("all").map(|r| r.into())
    }

    #[pyo3(signature = (*args, **kwargs))]
    fn filter(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("filter", args, kwargs)
            .map(|r| r.into())
    }

    #[pyo3(signature = (*args, **kwargs))]
    fn exclude(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("exclude", args, kwargs)
            .map(|r| r.into())
    }

    #[pyo3(signature = (*args, **kwargs))]
    fn get(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("get", args, kwargs)
            .map(|r| r.into())
    }

    #[pyo3(signature = (**kwargs))]
    fn create(&self, py: Python<'_>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("create", (), kwargs)
            .map(|r| r.into())
    }

    #[pyo3(signature = (*fields))]
    fn order_by(&self, py: Python<'_>, fields: Vec<String>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        let args = pyo3::types::PyTuple::new_bound(py, fields.iter().map(|f| f.into_py(py)));
        qs.bind(py)
            .call_method("order_by", &args, None)
            .map(|r| r.into())
    }

    fn distinct(&self, py: Python<'_>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py).call_method0("distinct").map(|r| r.into())
    }

    fn using(&self, py: Python<'_>, alias: String) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method1("using", (alias,))
            .map(|r| r.into())
    }

    fn none(&self, py: Python<'_>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py).call_method0("none").map(|r| r.into())
    }

    fn count(&self, py: Python<'_>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py).call_method0("count").map(|r| r.into())
    }

    fn exists(&self, py: Python<'_>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py).call_method0("exists").map(|r| r.into())
    }

    fn first(&self, py: Python<'_>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py).call_method0("first").map(|r| r.into())
    }

    fn last(&self, py: Python<'_>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py).call_method0("last").map(|r| r.into())
    }

    #[pyo3(signature = (*fields))]
    fn only(&self, py: Python<'_>, fields: Vec<String>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        let py_fields = pyo3::types::PyTuple::new_bound(py, fields.iter().map(|s| s.as_str()));
        qs.bind(py)
            .call_method1("only", &py_fields)
            .map(|r| r.into())
    }

    #[pyo3(signature = (*fields))]
    fn defer(&self, py: Python<'_>, fields: Vec<String>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        let py_fields = pyo3::types::PyTuple::new_bound(py, fields.iter().map(|s| s.as_str()));
        qs.bind(py)
            .call_method1("defer", &py_fields)
            .map(|r| r.into())
    }

    fn earliest(&self, py: Python<'_>, field: String) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method1("earliest", (field,))
            .map(|r| r.into())
    }

    fn latest(&self, py: Python<'_>, field: String) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method1("latest", (field,))
            .map(|r| r.into())
    }

    #[pyo3(signature = (sql, params = None))]
    fn raw(&self, py: Python<'_>, sql: String, params: Option<PyObject>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method1("raw", (sql, params))
            .map(|r| r.into())
    }

    #[pyo3(signature = (*fields))]
    fn values(&self, py: Python<'_>, fields: Vec<String>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        let py_fields = pyo3::types::PyTuple::new_bound(py, fields.iter().map(|s| s.as_str()));
        qs.bind(py)
            .call_method1("values", &py_fields)
            .map(|r| r.into())
    }

    #[pyo3(signature = (*fields, flat = false))]
    fn values_list(&self, py: Python<'_>, fields: Vec<String>, flat: bool) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        let kwargs = pyo3::types::PyDict::new_bound(py);
        kwargs.set_item("flat", flat)?;
        let py_fields = pyo3::types::PyTuple::new_bound(py, fields.iter().map(|s| s.as_str()));
        qs.bind(py)
            .call_method("values_list", py_fields, Some(&kwargs))
            .map(|r| r.into())
    }

    #[pyo3(signature = (**kwargs))]
    fn aggregate(&self, py: Python<'_>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("aggregate", (), kwargs)
            .map(|r| r.into())
    }

    #[pyo3(signature = (**kwargs))]
    fn annotate(&self, py: Python<'_>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("annotate", (), kwargs)
            .map(|r| r.into())
    }

    #[pyo3(signature = (defaults = None, **kwargs))]
    fn get_or_create(
        &self,
        py: Python<'_>,
        defaults: Option<&Bound<'_, PyDict>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("get_or_create", (defaults,), kwargs)
            .map(|r| r.into())
    }

    #[pyo3(signature = (defaults = None, **kwargs))]
    fn update_or_create(
        &self,
        py: Python<'_>,
        defaults: Option<&Bound<'_, PyDict>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("update_or_create", (defaults,), kwargs)
            .map(|r| r.into())
    }

    #[pyo3(signature = (*fields))]
    fn select_related(&self, py: Python<'_>, fields: Vec<String>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        let args = pyo3::types::PyTuple::new_bound(py, fields.iter().map(|f| f.into_py(py)));
        qs.bind(py)
            .call_method("select_related", &args, None)
            .map(|r| r.into())
    }

    #[pyo3(signature = (*fields))]
    fn prefetch_related(&self, py: Python<'_>, fields: Vec<String>) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        let args = pyo3::types::PyTuple::new_bound(py, fields.iter().map(|f| f.into_py(py)));
        qs.bind(py)
            .call_method("prefetch_related", &args, None)
            .map(|r| r.into())
    }

    #[pyo3(signature = (objs, batch_size = None, ignore_conflicts = false))]
    fn bulk_create(
        &self,
        py: Python<'_>,
        objs: Vec<PyObject>,
        batch_size: Option<usize>,
        ignore_conflicts: bool,
    ) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        let kwargs = pyo3::types::PyDict::new_bound(py);
        kwargs.set_item("ignore_conflicts", ignore_conflicts)?;
        qs.bind(py)
            .call_method("bulk_create", (objs, batch_size), Some(&kwargs))
            .map(|r| r.into())
    }

    #[pyo3(signature = (objs, fields, batch_size = None))]
    fn bulk_update(
        &self,
        py: Python<'_>,
        objs: Vec<PyObject>,
        fields: Vec<String>,
        batch_size: Option<usize>,
    ) -> PyResult<PyObject> {
        let qs = self.get_queryset(py)?;
        qs.bind(py)
            .call_method("bulk_update", (objs, fields, batch_size), None)
            .map(|r| r.into())
    }
}

// ── Model base class ─────────────────────────────────────────────────────────

/// Rust-side base class for all piorm models.
/// Accepts arbitrary kwargs in `__new__` so that Python subclasses can be
/// instantiated with keyword arguments.  The actual initialisation is done by
/// the Python-level `Model.__init__`.
#[pyclass(name = "Model", subclass, frozen)]
pub struct PyModel {}

#[pymethods]
impl PyModel {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        Self {}
    }

    /// Async — INSERT or UPDATE this model instance in the database.
    #[pyo3(signature = (using = None, force_insert = false, force_update = false, update_fields = None))]
    fn save(
        slf: Bound<'_, Self>,
        using: Option<String>,
        force_insert: bool,
        force_update: bool,
        update_fields: Option<Vec<String>>,
    ) -> PyResult<PyObject> {
        super::instance::save(
            slf.into_any(),
            using,
            force_insert,
            force_update,
            update_fields,
        )
    }

    /// Async — DELETE this model instance from the database.
    /// Returns `(rows_deleted, {"ModelName": rows_deleted})`.
    #[pyo3(signature = (using = None))]
    fn delete(slf: Bound<'_, Self>, using: Option<String>) -> PyResult<PyObject> {
        super::instance::delete(slf.into_any(), using)
    }

    /// Async — Re-fetch this instance's values from the database.
    #[pyo3(signature = (using = None, fields = None))]
    fn refresh_from_db(
        slf: Bound<'_, Self>,
        using: Option<String>,
        fields: Option<Vec<String>>,
    ) -> PyResult<PyObject> {
        super::instance::refresh_from_db(slf.into_any(), using, fields)
    }
}

// ── Registration function (called from Python __init_subclass__) ──────────────

/// Collect fields from `cls`, build a `ModelDef`, register it in the global
/// `ModelRegistry`, and attach an `objects` manager to the class.
///
/// Called from Python's `Model.__init_subclass__` so that the call chain works
/// correctly through Python's MRO without relying on PyO3's `__init_subclass__`
/// slot mechanism.
#[pyfunction]
pub fn _register_model_class(cls: Bound<'_, PyType>) -> PyResult<()> {
    let py = cls.py();
    let class_name = cls.name()?.to_string();

    // ── Collect fields from the class __dict__ ────────────────────────────

    let class_dict = cls.getattr("__dict__")?;
    let keys_iter = class_dict.call_method0("keys")?.iter()?;
    let keys: Vec<String> = keys_iter
        .map(|k| k?.extract::<String>())
        .collect::<PyResult<_>>()?;

    let mut fields: IndexMap<String, FieldDef> = IndexMap::new();
    let mut m2m_fields: Vec<M2MFieldDef> = Vec::new();
    let mut has_pk = false;
    let mut pk_field_name = "id".to_string();

    for key in &keys {
        if key.starts_with('_') {
            continue;
        }
        if matches!(
            key.as_str(),
            "Meta" | "objects" | "DoesNotExist" | "MultipleObjectsReturned"
        ) {
            continue;
        }

        let value = match cls.getattr(key.as_str()) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Duck-type check: field objects have a `contribute_to_class` method.
        if value.getattr("contribute_to_class").is_err() {
            continue;
        }

        // Call contribute_to_class — sets name/attname on the field object.
        value.call_method1("contribute_to_class", (&cls, key.as_str()))?;

        // Extract the updated FieldDef from the Rust-side field object.
        let Some(field_def) = extract_field_def(&value)? else {
            continue;
        };

        if matches!(field_def.field_type, FieldType::ManyToMany { .. }) {
            m2m_fields.push(build_m2m_def(&field_def, &class_name)?);
        } else {
            if field_def.primary_key {
                has_pk = true;
                pk_field_name = field_def.attname.clone();
            }
            fields.insert(field_def.attname.clone(), field_def);
        }
    }

    // Auto-add BigIntegerField `id` (auto_increment) when no explicit PK is declared.
    // Insert `id` first so it gets bit 0 in _field_bits (stable ordering guarantee).
    if !has_pk {
        let id_field = FieldDef {
            name: "id".to_string(),
            attname: "id".to_string(),
            db_column: None,
            field_type: FieldType::BigInt,
            primary_key: true,
            null: false,
            unique: false,
            db_index: false,
            editable: false,
            default: DefaultValue::None,
            choices: vec![],
            auto_increment: true,
            auto_now: false,
            auto_now_add: false,
            max_length: None,
            validators: vec![],
            verbose_name: Some("ID".to_string()),
            related_name: None,
        };
        let mut ordered = IndexMap::new();
        ordered.insert("id".to_string(), id_field);
        ordered.extend(fields);
        fields = ordered;
    }

    // ── Parse Meta options ────────────────────────────────────────────────

    let (app_label, db_table, ordering) = parse_meta(&cls)?;
    let table_name = db_table.unwrap_or_else(|| ModelDef::default_table_name(&class_name));

    // ── Build and register ModelDef ───────────────────────────────────────

    let model_def = ModelDef {
        app_label: app_label.clone(),
        model_name: class_name.clone(),
        table_name,
        fields,
        pk_field: pk_field_name.clone(),
        ordering,
        unique_together: vec![],
        index_together: vec![],
        indexes: vec![],
        constraints: vec![],
        m2m_fields,
        abstract_model: false,
        proxy: false,
        managed: true,
        default_manager_name: "objects".to_string(),
    };

    // ── Assign _field_bits for bitmask dirty tracking ─────────────────────
    //
    // Builds {"field_name": 1 << index} using the canonical field order from
    // model_def.fields (an IndexMap — insertion-ordered).  This dict is
    // stored as a ClassVar on the Python class so that __setattr__ in
    // models.py and extract_dirty in instance.rs can both read it.

    let field_bits_dict = PyDict::new_bound(py);
    for (i, name) in model_def.fields.keys().enumerate() {
        field_bits_dict.set_item(name, 1u64 << i)?;
    }

    ModelRegistry::global().write().unwrap().register(model_def);

    // ── Expose pk field name so Python's `pk` property can be dynamic ─────
    cls.setattr("_piorm_pk_field", pk_field_name.as_str())?;
    cls.setattr("_field_bits", field_bits_dict)?;

    // ── Attach `objects` manager ──────────────────────────────────────────

    let manager = PyManager::new(app_label, class_name, cls.clone().into_any().unbind());
    cls.setattr("objects", Py::new(py, manager)?)?;

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Try to downcast `obj` to each known Rust field type and return a clone of
/// its inner `FieldDef`.  Returns `None` for non-field objects.
fn extract_field_def(obj: &Bound<'_, PyAny>) -> PyResult<Option<FieldDef>> {
    use crate::fields::relational::{PyForeignKey, PyManyToManyField, PyOneToOneField};
    use crate::fields::scalar::{
        PyBigIntegerField, PyBooleanField, PyCharField, PyDecimalField, PyEmailField, PyFloatField,
        PyIntegerField, PyPositiveBigIntegerField, PyPositiveIntegerField,
        PyPositiveSmallIntegerField, PySlugField, PySmallIntegerField, PyTextField, PyUrlField,
    };
    use crate::fields::special::{
        PyBinaryField, PyGenericIPAddressField, PyIPAddressField, PyJSONField, PyUUIDField,
    };
    use crate::fields::temporal::{PyDateField, PyDateTimeField, PyDurationField, PyTimeField};

    macro_rules! try_field {
        ($T:ty) => {
            if let Ok(f) = obj.downcast::<$T>() {
                return Ok(Some(f.borrow().inner.clone()));
            }
        };
    }

    try_field!(PyIntegerField);
    try_field!(PyBigIntegerField);
    try_field!(PySmallIntegerField);
    try_field!(PyPositiveSmallIntegerField);
    try_field!(PyPositiveIntegerField);
    try_field!(PyPositiveBigIntegerField);
    try_field!(PyFloatField);
    try_field!(PyBooleanField);
    try_field!(PyTextField);
    try_field!(PyCharField);
    try_field!(PyDecimalField);
    try_field!(PySlugField);
    try_field!(PyEmailField);
    try_field!(PyUrlField);
    try_field!(PyDateField);
    try_field!(PyTimeField);
    try_field!(PyDateTimeField);
    try_field!(PyDurationField);
    try_field!(PyUUIDField);
    try_field!(PyJSONField);
    try_field!(PyBinaryField);
    try_field!(PyIPAddressField);
    try_field!(PyGenericIPAddressField);
    try_field!(PyForeignKey);
    try_field!(PyOneToOneField);
    try_field!(PyManyToManyField);

    Ok(None)
}

/// Build a `M2MFieldDef` from a ManyToMany `FieldDef`.
fn build_m2m_def(field_def: &FieldDef, from_model: &str) -> PyResult<M2MFieldDef> {
    if let FieldType::ManyToMany {
        to,
        through,
        db_table,
    } = &field_def.field_type
    {
        let junction_table = db_table
            .clone()
            .unwrap_or_else(|| format!("{}_{}", from_model.to_lowercase(), field_def.name));
        Ok(M2MFieldDef {
            field_name: field_def.name.clone(),
            to_model: to.clone(),
            through_model: through.clone(),
            junction_table,
            from_column: format!("{}_id", from_model.to_lowercase()),
            to_column: format!("{}_id", to.to_lowercase()),
        })
    } else {
        Err(pyo3::exceptions::PyRuntimeError::new_err(
            "build_m2m_def called on non-M2M field",
        ))
    }
}

/// Extract `Meta` options from the class.
fn parse_meta(cls: &Bound<'_, PyType>) -> PyResult<(String, Option<String>, Vec<OrderSpec>)> {
    let meta = cls.getattr("Meta").ok();

    // app_label is always inferred from the module path; users may not set it.
    let app_label = infer_app_label(cls);

    let db_table = meta
        .as_ref()
        .and_then(|m| m.getattr("db_table").ok())
        .and_then(|v| v.extract::<String>().ok());

    let ordering = meta
        .as_ref()
        .and_then(|m| m.getattr("ordering").ok())
        .and_then(|v| v.extract::<Vec<String>>().ok())
        .unwrap_or_default()
        .into_iter()
        .map(|s| parse_order_spec(&s))
        .collect();

    Ok((app_label, db_table, ordering))
}

/// Infer `app_label` from the module path: `"myapp.models"` → `"myapp"`.
fn infer_app_label(cls: &Bound<'_, PyType>) -> String {
    cls.getattr("__module__")
        .ok()
        .and_then(|m| m.extract::<String>().ok())
        .map(|m| m.split('.').next().unwrap_or("default").to_string())
        .unwrap_or_else(|| "default".to_string())
}

fn parse_order_spec(s: &str) -> OrderSpec {
    let descending = s.starts_with('-');
    let field = s.trim_start_matches('-').to_string();
    OrderSpec {
        field,
        descending,
        nulls_last: false,
        nulls_first: false,
    }
}
