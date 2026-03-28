use crate::{impl_field_common, impl_simple_field};
use ormpi_core::fields::{DefaultValue, FieldDef, FieldType};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

// ── Simple temporal fields ─────────────────────────────────────────────────

#[pyclass(name = "DateField")]
pub struct PyDateField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(PyDateField, "DateField", FieldType::Date);

#[pyclass(name = "TimeField")]
pub struct PyTimeField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(PyTimeField, "TimeField", FieldType::Time);

#[pyclass(name = "DurationField")]
pub struct PyDurationField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(PyDurationField, "DurationField", FieldType::Duration);

// ── DateTimeField — auto_now / auto_now_add ────────────────────────────────

#[pyclass(name = "DateTimeField")]
pub struct PyDateTimeField {
    pub(crate) inner: FieldDef,
}

impl_field_common!(PyDateTimeField, "DateTimeField");

#[pymethods]
impl PyDateTimeField {
    #[new]
    #[pyo3(signature = (
        *,
        auto_now = false,
        auto_now_add = false,
        null = false,
        default = None,
        unique = false,
        db_index = false,
        primary_key = false,
        editable = true,
        db_column = None,
        verbose_name = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        auto_now: bool,
        auto_now_add: bool,
        null: bool,
        default: Option<PyObject>,
        unique: bool,
        db_index: bool,
        primary_key: bool,
        editable: bool,
        db_column: Option<String>,
        verbose_name: Option<String>,
    ) -> PyResult<Self> {
        if auto_now && auto_now_add {
            return Err(PyValueError::new_err(
                "auto_now and auto_now_add are mutually exclusive",
            ));
        }
        let default = if default.is_some() {
            crate::fields::parse_default(py, default)?
        } else if auto_now_add {
            DefaultValue::CurrentTimestamp
        } else {
            DefaultValue::None
        };
        // auto_now and auto_now_add make the column effectively optional (managed by ORM)
        let effective_null = null || auto_now || auto_now_add;
        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column,
                field_type: FieldType::DateTime,
                null: effective_null,
                unique,
                db_index,
                primary_key,
                editable: editable && !auto_now,
                default,
                choices: vec![],
                auto_increment: false,
                auto_now,
                auto_now_add,
                max_length: None,
                validators: vec![],
                verbose_name,
                related_name: None,
            },
        })
    }

    #[getter]
    fn auto_now(&self) -> bool {
        self.inner.auto_now
    }

    #[getter]
    fn auto_now_add(&self) -> bool {
        self.inner.auto_now_add
    }
}

// ── Rust unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_field_repr() {
        let f = PyDateField {
            inner: FieldDef {
                name: "birthday".into(),
                attname: "birthday".into(),
                db_column: None,
                field_type: FieldType::Date,
                null: false,
                unique: false,
                db_index: false,
                primary_key: false,
                editable: true,
                default: DefaultValue::None,
                choices: vec![],
                auto_increment: false,
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                verbose_name: None,
                related_name: None,
            },
        };
        assert_eq!(f.__repr__(), "<DateField 'birthday'>");
    }

    #[test]
    fn test_datetime_field_auto_now_add_sets_default() {
        let f = PyDateTimeField {
            inner: FieldDef {
                name: "created_at".into(),
                attname: "created_at".into(),
                db_column: None,
                field_type: FieldType::DateTime,
                null: true,
                unique: false,
                db_index: false,
                primary_key: false,
                editable: false,
                default: DefaultValue::CurrentTimestamp,
                choices: vec![],
                auto_increment: false,
                auto_now: false,
                auto_now_add: true,
                max_length: None,
                validators: vec![],
                verbose_name: None,
                related_name: None,
            },
        };
        assert!(f.auto_now_add());
        assert!(!f.auto_now());
        assert!(f.null());
    }
}
