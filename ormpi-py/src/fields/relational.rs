use ormpi_core::fields::{DefaultValue, FieldDef, FieldType, OnDelete};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

// ── Helpers ────────────────────────────────────────────────────────────────

/// Resolve the `to` argument (string or class) to a plain model name string.
pub(crate) fn resolve_to_string(py: Python<'_>, to: &PyObject) -> PyResult<String> {
    let bound = to.bind(py);
    // String reference: "Author" or "app.Author"
    if let Ok(s) = bound.extract::<String>() {
        return Ok(s);
    }
    // Class reference: extract __name__
    if let Ok(name) = bound.getattr("__name__") {
        if let Ok(s) = name.extract::<String>() {
            return Ok(s);
        }
    }
    Err(PyTypeError::new_err("'to' must be a Model class or a string"))
}

/// Parse an on_delete string (e.g. "CASCADE") into an `OnDelete` enum value.
pub(crate) fn parse_on_delete(s: &str) -> PyResult<OnDelete> {
    match s.to_uppercase().as_str() {
        "CASCADE" => Ok(OnDelete::Cascade),
        "PROTECT" => Ok(OnDelete::Protect),
        "SET_NULL" => Ok(OnDelete::SetNull),
        "SET_DEFAULT" => Ok(OnDelete::SetDefault),
        "DO_NOTHING" => Ok(OnDelete::DoNothing),
        "RESTRICT" => Ok(OnDelete::Restrict),
        _ => Err(PyValueError::new_err(format!(
            "Unknown on_delete value: '{}'. Choose one of: CASCADE, PROTECT, SET_NULL, \
             SET_DEFAULT, DO_NOTHING, RESTRICT",
            s
        ))),
    }
}

// ── ForeignKey ─────────────────────────────────────────────────────────────

/// A foreign-key relation to another model.
/// Stores `{name}_id` in the database and indexes by default.
#[pyclass(name = "ForeignKey")]
pub struct PyForeignKey {
    pub(crate) inner: FieldDef,
    pub to: String,
    pub on_delete: OnDelete,
    pub related_name: Option<String>,
    pub related_query_name: Option<String>,
    pub to_field: Option<String>,
    pub db_constraint: bool,
}

#[pymethods]
impl PyForeignKey {
    #[new]
    #[pyo3(signature = (
        to,
        on_delete,
        *,
        related_name = None,
        related_query_name = None,
        to_field = None,
        db_constraint = true,
        null = false,
        blank = false,
        db_index = true,
        db_column = None,
        help_text = "",
        verbose_name = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        to: PyObject,
        on_delete: &str,
        related_name: Option<String>,
        related_query_name: Option<String>,
        to_field: Option<String>,
        db_constraint: bool,
        null: bool,
        blank: bool,
        db_index: bool,
        db_column: Option<String>,
        help_text: &str,
        verbose_name: Option<String>,
    ) -> PyResult<Self> {
        let to_str = resolve_to_string(py, &to)?;
        let on_delete_val = parse_on_delete(on_delete)?;

        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(), // set to "{name}_id" in contribute_to_class
                db_column,
                field_type: FieldType::ForeignKey {
                    to: to_str.clone(),
                    on_delete: on_delete_val,
                    db_column: None,
                },
                null,
                blank,
                unique: false,
                db_index,
                primary_key: false,
                editable: true,
                default: DefaultValue::None,
                choices: vec![],
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                help_text: help_text.to_string(),
                verbose_name,
                related_name: related_name.clone(),
            },
            to: to_str,
            on_delete: on_delete_val,
            related_name,
            related_query_name,
            to_field,
            db_constraint,
        })
    }

    fn contribute_to_class(
        &mut self,
        _cls: Bound<'_, PyAny>,
        name: String,
    ) -> PyResult<()> {
        self.inner.name = name.clone();
        self.inner.attname = format!("{}_id", name); // FK stores {name}_id
        Ok(())
    }

    /// Descriptor `__get__`: on instance returns the cached related object or
    /// the raw FK id value.  On class access returns the field itself.
    fn __get__(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        obj: Option<Bound<'_, PyAny>>,
        _objtype: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        use pyo3::prelude::IntoPy;
        match obj {
            None => Ok(slf.into_py(py)),
            Some(instance) => {
                // Check cache first
                let cache_key = format!("_ormpi_cache_{}", slf.inner.name);
                if let Ok(cached) = instance.getattr(cache_key.as_str()) {
                    if !cached.is_none() {
                        return Ok(cached.into_py(py));
                    }
                }
                // Return the raw FK id value from _ormpi_values
                let values = instance.getattr("_ormpi_values")?;
                if let Ok(dict) = values.downcast_into::<PyDict>() {
                    match dict.get_item(slf.inner.attname.as_str())? {
                        Some(v) => Ok(v.into_py(py)),
                        None => Ok(py.None()),
                    }
                } else {
                    Ok(py.None())
                }
            }
        }
    }

    /// Descriptor `__set__`: stores the FK id and caches the related object.
    fn __set__(
        &self,
        instance: Bound<'_, PyAny>,
        value: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = instance.py();
        let values = instance.getattr("_ormpi_values")?;
        if let Ok(dict) = values.downcast_into::<PyDict>() {
            if value.is_none() {
                dict.set_item(self.inner.attname.as_str(), py.None())?;
            } else {
                // Extract the pk from the related instance
                if let Ok(pk) = value.getattr("pk") {
                    dict.set_item(self.inner.attname.as_str(), &pk)?;
                } else {
                    // Treat the value itself as a raw id
                    dict.set_item(self.inner.attname.as_str(), &value)?;
                }
                // Cache the related object
                let cache_key = format!("_ormpi_cache_{}", self.inner.name);
                instance.setattr(cache_key.as_str(), &value)?;
            }
        }
        instance
            .getattr("_ormpi_dirty")?
            .call_method1("add", (self.inner.attname.as_str(),))?;
        Ok(())
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn attname(&self) -> String {
        if self.inner.attname.is_empty() {
            format!("{}_id", self.inner.name)
        } else {
            self.inner.attname.clone()
        }
    }

    #[getter]
    fn column(&self) -> String {
        self.inner
            .db_column
            .clone()
            .unwrap_or_else(|| format!("{}_id", self.inner.name))
    }

    #[getter]
    fn null(&self) -> bool {
        self.inner.null
    }

    #[getter]
    fn db_index(&self) -> bool {
        self.inner.db_index
    }

    #[getter]
    fn to_model(&self) -> &str {
        &self.to
    }

    fn __repr__(&self) -> String {
        format!("<ForeignKey '{}' → {}>", self.inner.name, self.to)
    }
}

// ── OneToOneField ──────────────────────────────────────────────────────────

/// A one-to-one relation: like ForeignKey but with unique=True.
#[pyclass(name = "OneToOneField")]
pub struct PyOneToOneField {
    pub(crate) inner: FieldDef,
    pub to: String,
    pub on_delete: OnDelete,
    pub related_name: Option<String>,
    pub db_constraint: bool,
}

#[pymethods]
impl PyOneToOneField {
    #[new]
    #[pyo3(signature = (
        to,
        on_delete,
        *,
        related_name = None,
        db_constraint = true,
        null = false,
        blank = false,
        db_column = None,
        help_text = "",
        verbose_name = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        to: PyObject,
        on_delete: &str,
        related_name: Option<String>,
        db_constraint: bool,
        null: bool,
        blank: bool,
        db_column: Option<String>,
        help_text: &str,
        verbose_name: Option<String>,
    ) -> PyResult<Self> {
        let to_str = resolve_to_string(py, &to)?;
        let on_delete_val = parse_on_delete(on_delete)?;

        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column,
                field_type: FieldType::OneToOne {
                    to: to_str.clone(),
                    on_delete: on_delete_val,
                },
                null,
                blank,
                unique: true, // O2O is always unique
                db_index: true,
                primary_key: false,
                editable: true,
                default: DefaultValue::None,
                choices: vec![],
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                help_text: help_text.to_string(),
                verbose_name,
                related_name: related_name.clone(),
            },
            to: to_str,
            on_delete: on_delete_val,
            related_name,
            db_constraint,
        })
    }

    fn contribute_to_class(
        &mut self,
        _cls: Bound<'_, PyAny>,
        name: String,
    ) -> PyResult<()> {
        self.inner.name = name.clone();
        self.inner.attname = format!("{}_id", name);
        Ok(())
    }

    fn __get__(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        obj: Option<Bound<'_, PyAny>>,
        _objtype: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        use pyo3::prelude::IntoPy;
        match obj {
            None => Ok(slf.into_py(py)),
            Some(instance) => {
                let values = instance.getattr("_ormpi_values")?;
                if let Ok(dict) = values.downcast_into::<PyDict>() {
                    match dict.get_item(slf.inner.attname.as_str())? {
                        Some(v) => Ok(v.into_py(py)),
                        None => Ok(py.None()),
                    }
                } else {
                    Ok(py.None())
                }
            }
        }
    }

    fn __set__(
        &self,
        instance: Bound<'_, PyAny>,
        value: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = instance.py();
        let values = instance.getattr("_ormpi_values")?;
        if let Ok(dict) = values.downcast_into::<PyDict>() {
            if value.is_none() {
                dict.set_item(self.inner.attname.as_str(), py.None())?;
            } else if let Ok(pk) = value.getattr("pk") {
                dict.set_item(self.inner.attname.as_str(), &pk)?;
            } else {
                dict.set_item(self.inner.attname.as_str(), &value)?;
            }
        }
        instance
            .getattr("_ormpi_dirty")?
            .call_method1("add", (self.inner.attname.as_str(),))?;
        Ok(())
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn attname(&self) -> String {
        if self.inner.attname.is_empty() {
            format!("{}_id", self.inner.name)
        } else {
            self.inner.attname.clone()
        }
    }

    #[getter]
    fn unique(&self) -> bool {
        self.inner.unique
    }

    #[getter]
    fn null(&self) -> bool {
        self.inner.null
    }

    #[getter]
    fn to_model(&self) -> &str {
        &self.to
    }

    fn __repr__(&self) -> String {
        format!("<OneToOneField '{}' → {}>", self.inner.name, self.to)
    }
}

// ── ManyToManyField ────────────────────────────────────────────────────────

/// A many-to-many relation managed via a junction table.
/// Does NOT produce a column in the owning model's table.
#[pyclass(name = "ManyToManyField")]
pub struct PyManyToManyField {
    pub(crate) inner: FieldDef,
    pub to: String,
    pub through: Option<String>,
    pub db_table: Option<String>,
    pub related_name: Option<String>,
    pub symmetrical: bool,
}

#[pymethods]
impl PyManyToManyField {
    #[new]
    #[pyo3(signature = (
        to,
        *,
        through = None,
        db_table = None,
        related_name = None,
        symmetrical = None,
    ))]
    fn new(
        py: Python<'_>,
        to: PyObject,
        through: Option<PyObject>,
        db_table: Option<String>,
        related_name: Option<String>,
        symmetrical: Option<bool>,
    ) -> PyResult<Self> {
        let to_str = resolve_to_string(py, &to)?;
        let through_str = match through {
            Some(t) => Some(resolve_to_string(py, &t)?),
            None => None,
        };
        let sym = symmetrical.unwrap_or_else(|| to_str == "self");

        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column: None,
                field_type: FieldType::ManyToMany {
                    to: to_str.clone(),
                    through: through_str.clone(),
                    db_table: db_table.clone(),
                },
                null: false,
                blank: true, // M2M is always blank=True conceptually
                unique: false,
                db_index: false,
                primary_key: false,
                editable: true,
                default: DefaultValue::None,
                choices: vec![],
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                help_text: String::new(),
                verbose_name: None,
                related_name: related_name.clone(),
            },
            to: to_str,
            through: through_str,
            db_table,
            related_name,
            symmetrical: sym,
        })
    }

    fn contribute_to_class(
        &mut self,
        _cls: Bound<'_, PyAny>,
        name: String,
    ) -> PyResult<()> {
        self.inner.name = name;
        // No attname — M2M doesn't add a column to this table
        Ok(())
    }

    fn __get__(
        slf: PyRef<'_, Self>,
        py: Python<'_>,
        obj: Option<Bound<'_, PyAny>>,
        _objtype: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        use pyo3::prelude::IntoPy;
        match obj {
            None => Ok(slf.into_py(py)),
            // Return a ManyRelatedManager placeholder — full impl in Phase 4
            Some(_) => Ok(py.None()),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn to_model(&self) -> &str {
        &self.to
    }

    #[getter]
    fn symmetrical(&self) -> bool {
        self.symmetrical
    }

    fn __repr__(&self) -> String {
        format!("<ManyToManyField '{}' ↔ {}>", self.inner.name, self.to)
    }
}

// ── on_delete constant registration ───────────────────────────────────────

pub fn register_constants(m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add("CASCADE", "CASCADE")?;
    m.add("PROTECT", "PROTECT")?;
    m.add("SET_NULL", "SET_NULL")?;
    m.add("SET_DEFAULT", "SET_DEFAULT")?;
    m.add("DO_NOTHING", "DO_NOTHING")?;
    m.add("RESTRICT", "RESTRICT")?;
    Ok(())
}

// ── Rust unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_on_delete_cascade() {
        assert!(matches!(parse_on_delete("CASCADE"), Ok(OnDelete::Cascade)));
        assert!(matches!(
            parse_on_delete("cascade"),
            Ok(OnDelete::Cascade)
        ));
    }

    #[test]
    fn test_parse_on_delete_unknown() {
        assert!(parse_on_delete("EXPLODE").is_err());
    }

    #[test]
    fn test_fk_attname_after_contribute() {
        let mut fk = PyForeignKey {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column: None,
                field_type: FieldType::ForeignKey {
                    to: "Author".into(),
                    on_delete: OnDelete::Cascade,
                    db_column: None,
                },
                null: false,
                blank: false,
                unique: false,
                db_index: true,
                primary_key: false,
                editable: true,
                default: DefaultValue::None,
                choices: vec![],
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                help_text: String::new(),
                verbose_name: None,
                related_name: None,
            },
            to: "Author".into(),
            on_delete: OnDelete::Cascade,
            related_name: None,
            related_query_name: None,
            to_field: None,
            db_constraint: true,
        };
        fk.inner.name = "author".into();
        fk.inner.attname = format!("{}_id", fk.inner.name);
        assert_eq!(fk.attname(), "author_id");
        assert_eq!(fk.column(), "author_id");
    }

    #[test]
    fn test_o2o_is_unique() {
        let o2o = PyOneToOneField {
            inner: FieldDef {
                name: "profile".into(),
                attname: "profile_id".into(),
                db_column: None,
                field_type: FieldType::OneToOne {
                    to: "Profile".into(),
                    on_delete: OnDelete::Cascade,
                },
                null: false,
                blank: false,
                unique: true,
                db_index: true,
                primary_key: false,
                editable: true,
                default: DefaultValue::None,
                choices: vec![],
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                help_text: String::new(),
                verbose_name: None,
                related_name: None,
            },
            to: "Profile".into(),
            on_delete: OnDelete::Cascade,
            related_name: None,
            db_constraint: true,
        };
        assert!(o2o.unique());
    }

    #[test]
    fn test_m2m_no_column() {
        let m2m = PyManyToManyField {
            inner: FieldDef {
                name: "tags".into(),
                attname: String::new(),
                db_column: None,
                field_type: FieldType::ManyToMany {
                    to: "Tag".into(),
                    through: None,
                    db_table: None,
                },
                null: false,
                blank: true,
                unique: false,
                db_index: false,
                primary_key: false,
                editable: true,
                default: DefaultValue::None,
                choices: vec![],
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                help_text: String::new(),
                verbose_name: None,
                related_name: None,
            },
            to: "Tag".into(),
            through: None,
            db_table: None,
            related_name: None,
            symmetrical: false,
        };
        // ManyToMany should not produce a column
        assert!(m2m.inner.to_column_def().is_none());
    }
}
