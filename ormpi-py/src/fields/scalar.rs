use crate::{impl_field_common, impl_integer_field, impl_simple_field};
use ormpi_core::fields::{DefaultValue, FieldDef, FieldType};
use pyo3::prelude::*;

// ── Integer fields (support auto_increment kwarg) ──────────────────────────

#[pyclass(name = "IntegerField")]
pub struct PyIntegerField {
    pub(crate) inner: FieldDef,
}
impl_integer_field!(PyIntegerField, "IntegerField", FieldType::Int);

#[pyclass(name = "BigIntegerField")]
pub struct PyBigIntegerField {
    pub(crate) inner: FieldDef,
}
impl_integer_field!(PyBigIntegerField, "BigIntegerField", FieldType::BigInt);

#[pyclass(name = "SmallIntegerField")]
pub struct PySmallIntegerField {
    pub(crate) inner: FieldDef,
}
impl_integer_field!(PySmallIntegerField, "SmallIntegerField", FieldType::SmallInt);

#[pyclass(name = "PositiveSmallIntegerField")]
pub struct PyPositiveSmallIntegerField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(
    PyPositiveSmallIntegerField,
    "PositiveSmallIntegerField",
    FieldType::PositiveSmallInt
);

#[pyclass(name = "PositiveIntegerField")]
pub struct PyPositiveIntegerField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(
    PyPositiveIntegerField,
    "PositiveIntegerField",
    FieldType::PositiveInt
);

#[pyclass(name = "PositiveBigIntegerField")]
pub struct PyPositiveBigIntegerField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(
    PyPositiveBigIntegerField,
    "PositiveBigIntegerField",
    FieldType::PositiveBigInt
);

// ── Float / Boolean / Text ─────────────────────────────────────────────────

#[pyclass(name = "FloatField")]
pub struct PyFloatField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(PyFloatField, "FloatField", FieldType::Float);

#[pyclass(name = "BooleanField")]
pub struct PyBooleanField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(PyBooleanField, "BooleanField", FieldType::Boolean);

#[pyclass(name = "TextField")]
pub struct PyTextField {
    pub(crate) inner: FieldDef,
}
impl_simple_field!(PyTextField, "TextField", FieldType::Text);

// ── CharField — requires max_length ───────────────────────────────────────

#[pyclass(name = "CharField")]
pub struct PyCharField {
    pub(crate) inner: FieldDef,
}

impl_field_common!(PyCharField, "CharField");

#[pymethods]
impl PyCharField {
    #[new]
    #[pyo3(signature = (
        max_length,
        *,
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
        max_length: u32,
        null: bool,
        default: Option<PyObject>,
        unique: bool,
        db_index: bool,
        primary_key: bool,
        editable: bool,
        db_column: Option<String>,
        verbose_name: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column,
                field_type: FieldType::Char { max_length },
                null,
                unique,
                db_index,
                primary_key,
                editable,
                default: crate::fields::parse_default(py, default)?,
                choices: vec![],
                auto_increment: false,
                auto_now: false,
                auto_now_add: false,
                max_length: Some(max_length),
                validators: vec![],
                verbose_name,
                related_name: None,
            },
        })
    }

    #[getter]
    fn max_length(&self) -> Option<u32> {
        self.inner.max_length
    }
}

// ── DecimalField — requires max_digits and decimal_places ─────────────────

#[pyclass(name = "DecimalField")]
pub struct PyDecimalField {
    pub(crate) inner: FieldDef,
}

impl_field_common!(PyDecimalField, "DecimalField");

#[pymethods]
impl PyDecimalField {
    #[new]
    #[pyo3(signature = (
        max_digits,
        decimal_places,
        *,
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
        max_digits: u32,
        decimal_places: u32,
        null: bool,
        default: Option<PyObject>,
        unique: bool,
        db_index: bool,
        primary_key: bool,
        editable: bool,
        db_column: Option<String>,
        verbose_name: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column,
                field_type: FieldType::Decimal {
                    max_digits,
                    decimal_places,
                },
                null,
                unique,
                db_index,
                primary_key,
                editable,
                default: crate::fields::parse_default(py, default)?,
                choices: vec![],
                auto_increment: false,
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                verbose_name,
                related_name: None,
            },
        })
    }
}

// ── SlugField / EmailField / UrlField — max_length with defaults ───────────

#[pyclass(name = "SlugField")]
pub struct PySlugField {
    pub(crate) inner: FieldDef,
}

impl_field_common!(PySlugField, "SlugField");

#[pymethods]
impl PySlugField {
    #[new]
    #[pyo3(signature = (
        max_length = 50,
        *,
        null = false,
        default = None,
        unique = false,
        db_index = true,
        primary_key = false,
        editable = true,
        db_column = None,
        verbose_name = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        max_length: u32,
        null: bool,
        default: Option<PyObject>,
        unique: bool,
        db_index: bool,
        primary_key: bool,
        editable: bool,
        db_column: Option<String>,
        verbose_name: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column,
                field_type: FieldType::Slug { max_length },
                null,
                unique,
                db_index,
                primary_key,
                editable,
                default: crate::fields::parse_default(py, default)?,
                choices: vec![],
                auto_increment: false,
                auto_now: false,
                auto_now_add: false,
                max_length: Some(max_length),
                validators: vec![],
                verbose_name,
                related_name: None,
            },
        })
    }
}

#[pyclass(name = "EmailField")]
pub struct PyEmailField {
    pub(crate) inner: FieldDef,
}

impl_field_common!(PyEmailField, "EmailField");

#[pymethods]
impl PyEmailField {
    #[new]
    #[pyo3(signature = (
        max_length = 254,
        *,
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
        max_length: u32,
        null: bool,
        default: Option<PyObject>,
        unique: bool,
        db_index: bool,
        primary_key: bool,
        editable: bool,
        db_column: Option<String>,
        verbose_name: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column,
                field_type: FieldType::Email { max_length },
                null,
                unique,
                db_index,
                primary_key,
                editable,
                default: crate::fields::parse_default(py, default)?,
                choices: vec![],
                auto_increment: false,
                auto_now: false,
                auto_now_add: false,
                max_length: Some(max_length),
                validators: vec![],
                verbose_name,
                related_name: None,
            },
        })
    }
}

#[pyclass(name = "UrlField")]
pub struct PyUrlField {
    pub(crate) inner: FieldDef,
}

impl_field_common!(PyUrlField, "UrlField");

#[pymethods]
impl PyUrlField {
    #[new]
    #[pyo3(signature = (
        max_length = 200,
        *,
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
        max_length: u32,
        null: bool,
        default: Option<PyObject>,
        unique: bool,
        db_index: bool,
        primary_key: bool,
        editable: bool,
        db_column: Option<String>,
        verbose_name: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FieldDef {
                name: String::new(),
                attname: String::new(),
                db_column,
                field_type: FieldType::Url { max_length },
                null,
                unique,
                db_index,
                primary_key,
                editable,
                default: crate::fields::parse_default(py, default)?,
                choices: vec![],
                auto_increment: false,
                auto_now: false,
                auto_now_add: false,
                max_length: Some(max_length),
                validators: vec![],
                verbose_name,
                related_name: None,
            },
        })
    }
}

// ── Rust unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn int_field() -> PyIntegerField {
        PyIntegerField {
            inner: FieldDef {
                name: "age".into(),
                attname: "age".into(),
                db_column: None,
                field_type: FieldType::Int,
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
        }
    }

    #[test]
    fn test_int_field_repr() {
        let f = int_field();
        assert_eq!(f.__repr__(), "<IntegerField 'age'>");
    }

    #[test]
    fn test_char_field_max_length() {
        let f = PyCharField {
            inner: FieldDef {
                name: "title".into(),
                attname: "title".into(),
                db_column: None,
                field_type: FieldType::Char { max_length: 100 },
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
                max_length: Some(100),
                validators: vec![],
                verbose_name: None,
                related_name: None,
            },
        };
        assert_eq!(f.max_length(), Some(100));
        assert_eq!(f.__repr__(), "<CharField 'title'>");
    }

    #[test]
    fn test_contribute_to_class() {
        let mut f = int_field();
        f.inner.name = String::new();
        f.inner.attname = String::new();
        f.inner.name = "score".into();
        f.inner.attname = "score".into();
        assert_eq!(f.name(), "score");
    }
}
