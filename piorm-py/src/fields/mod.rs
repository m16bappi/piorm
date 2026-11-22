pub mod relational;
pub mod scalar;
pub mod special;
pub mod temporal;

use piorm_core::fields::DefaultValue;
use piorm_core::value::PiormValue;
use pyo3::prelude::*;
use pyo3::types::PyModule;

pub(crate) fn parse_default(py: Python<'_>, default: Option<PyObject>) -> PyResult<DefaultValue> {
    match default {
        None => Ok(DefaultValue::None),
        Some(obj) => {
            let bound = obj.bind(py);
            if bound.is_none() {
                return Ok(DefaultValue::Null);
            }
            // Extract bool before i64 — Python bool is a subclass of int
            if let Ok(b) = bound.extract::<bool>() {
                return Ok(DefaultValue::Value(PiormValue::Bool(b)));
            }
            if let Ok(i) = bound.extract::<i64>() {
                return Ok(DefaultValue::Value(PiormValue::Int(i)));
            }
            if let Ok(f) = bound.extract::<f64>() {
                return Ok(DefaultValue::Value(PiormValue::Float(f)));
            }
            if let Ok(s) = bound.extract::<String>() {
                return Ok(DefaultValue::Value(PiormValue::Text(s)));
            }
            Err(pyo3::exceptions::PyValueError::new_err(
                "Unsupported default value type",
            ))
        }
    }
}

/// Generate the common non-constructor PyO3 methods for a field class.
/// Requires `inner: FieldDef` field on the struct.
#[macro_export]
macro_rules! impl_field_common {
    ($cls:ident, $display_name:literal) => {
        #[pymethods]
        impl $cls {
            /// Called by the model metaclass to attach the field to its model.
            fn contribute_to_class(
                &mut self,
                _cls: pyo3::Bound<'_, pyo3::PyAny>,
                name: String,
            ) -> pyo3::PyResult<()> {
                self.inner.name = name.clone();
                self.inner.attname = name;
                Ok(())
            }

            /// Descriptor `__get__`: return field itself when accessed on the class,
            /// or the stored value when accessed on an instance.
            fn __get__(
                slf: pyo3::PyRef<'_, Self>,
                py: pyo3::Python<'_>,
                obj: Option<pyo3::Bound<'_, pyo3::PyAny>>,
                _objtype: Option<pyo3::Bound<'_, pyo3::PyAny>>,
            ) -> pyo3::PyResult<pyo3::PyObject> {
                use pyo3::prelude::IntoPy;
                match obj {
                    None => Ok(slf.into_py(py)),
                    Some(instance) => {
                        let values = instance.getattr("_values")?;
                        if let Ok(dict) =
                            values.downcast_into::<pyo3::types::PyDict>()
                        {
                            match dict.get_item(slf.inner.name.as_str())? {
                                Some(v) => Ok(v.into_py(py)),
                                None => Ok(py.None()),
                            }
                        } else {
                            Ok(py.None())
                        }
                    }
                }
            }

            /// Descriptor `__set__`: write value into `_values` and update
            /// the `_dirty` bitmask (only for DB-fetched instances).
            fn __set__(
                &self,
                instance: pyo3::Bound<'_, pyo3::PyAny>,
                value: pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let values = instance.getattr("_values")?;
                if let Ok(dict) = values.downcast_into::<pyo3::types::PyDict>() {
                    dict.set_item(self.inner.name.as_str(), &value)?;
                }
                // Only track dirty when loaded from DB (adding=False).
                // For new (adding=True) instances, save() does a full INSERT anyway.
                let is_adding: bool = instance
                    .getattr("_adding")
                    .ok()
                    .and_then(|v| v.extract::<bool>().ok())
                    .unwrap_or(true);
                if !is_adding {
                    let bit: u64 = instance
                        .get_type()
                        .getattr("_field_bits")
                        .ok()
                        .and_then(|fb| fb.downcast_into::<pyo3::types::PyDict>().ok())
                        .and_then(|d| d.get_item(self.inner.name.as_str()).ok().flatten())
                        .and_then(|v| v.extract::<u64>().ok())
                        .unwrap_or(0);
                    if bit != 0 {
                        let current: u64 = instance
                            .getattr("_dirty")
                            .ok()
                            .and_then(|v| v.extract::<u64>().ok())
                            .unwrap_or(0);
                        instance.setattr("_dirty", current | bit)?;
                    }
                }
                Ok(())
            }

            #[getter]
            fn name(&self) -> &str {
                &self.inner.name
            }

            #[getter]
            fn attname(&self) -> &str {
                &self.inner.attname
            }

            #[getter]
            fn null(&self) -> bool {
                self.inner.null
            }

            #[getter]
            fn unique(&self) -> bool {
                self.inner.unique
            }

            #[getter]
            fn primary_key(&self) -> bool {
                self.inner.primary_key
            }

            #[getter]
            fn db_index(&self) -> bool {
                self.inner.db_index
            }

            fn __repr__(&self) -> String {
                format!("<{} '{}'>", $display_name, self.inner.name)
            }
        }
    };
}

/// Generate a complete field class: `#[pyclass]` struct + common methods + default constructor.
#[macro_export]
macro_rules! impl_simple_field {
    ($cls:ident, $display_name:literal, $field_type_expr:expr) => {
        impl_field_common!($cls, $display_name);

        #[pymethods]
        impl $cls {
            #[new]
            #[pyo3(signature = (
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
                py: pyo3::Python<'_>,
                null: bool,
                default: Option<pyo3::PyObject>,
                unique: bool,
                db_index: bool,
                primary_key: bool,
                editable: bool,
                db_column: Option<String>,
                verbose_name: Option<String>,
            ) -> pyo3::PyResult<Self> {
                Ok(Self {
                    inner: piorm_core::fields::FieldDef {
                        name: String::new(),
                        attname: String::new(),
                        db_column,
                        field_type: $field_type_expr,
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
    };
}

/// Generate a field class that supports the `auto_increment` keyword argument.
/// Used for integer field types (IntegerField, BigIntegerField, SmallIntegerField).
#[macro_export]
macro_rules! impl_integer_field {
    ($cls:ident, $display_name:literal, $field_type_expr:expr) => {
        impl_field_common!($cls, $display_name);

        #[pymethods]
        impl $cls {
            #[new]
            #[pyo3(signature = (
                *,
                null = false,
                default = None,
                unique = false,
                db_index = false,
                primary_key = false,
                editable = true,
                db_column = None,
                verbose_name = None,
                auto_increment = false,
            ))]
            #[allow(clippy::too_many_arguments)]
            fn new(
                py: pyo3::Python<'_>,
                null: bool,
                default: Option<pyo3::PyObject>,
                unique: bool,
                db_index: bool,
                primary_key: bool,
                editable: bool,
                db_column: Option<String>,
                verbose_name: Option<String>,
                auto_increment: bool,
            ) -> pyo3::PyResult<Self> {
                Ok(Self {
                    inner: piorm_core::fields::FieldDef {
                        name: String::new(),
                        attname: String::new(),
                        db_column,
                        field_type: $field_type_expr,
                        null,
                        unique,
                        db_index,
                        primary_key,
                        editable,
                        default: crate::fields::parse_default(py, default)?,
                        choices: vec![],
                        auto_increment,
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
    };
}

pub fn register_fields(m: &Bound<'_, PyModule>) -> PyResult<()> {
    relational::register_constants(m)?;
    m.add_class::<scalar::PyIntegerField>()?;
    m.add_class::<scalar::PyBigIntegerField>()?;
    m.add_class::<scalar::PySmallIntegerField>()?;
    m.add_class::<scalar::PyPositiveSmallIntegerField>()?;
    m.add_class::<scalar::PyPositiveIntegerField>()?;
    m.add_class::<scalar::PyPositiveBigIntegerField>()?;
    m.add_class::<scalar::PyFloatField>()?;
    m.add_class::<scalar::PyBooleanField>()?;
    m.add_class::<scalar::PyTextField>()?;
    m.add_class::<scalar::PyCharField>()?;
    m.add_class::<scalar::PyDecimalField>()?;
    m.add_class::<scalar::PySlugField>()?;
    m.add_class::<scalar::PyEmailField>()?;
    m.add_class::<scalar::PyUrlField>()?;
    m.add_class::<temporal::PyDateField>()?;
    m.add_class::<temporal::PyTimeField>()?;
    m.add_class::<temporal::PyDateTimeField>()?;
    m.add_class::<temporal::PyDurationField>()?;
    m.add_class::<special::PyUUIDField>()?;
    m.add_class::<special::PyJSONField>()?;
    m.add_class::<special::PyBinaryField>()?;
    m.add_class::<special::PyIPAddressField>()?;
    m.add_class::<special::PyGenericIPAddressField>()?;
    m.add_class::<relational::PyForeignKey>()?;
    m.add_class::<relational::PyOneToOneField>()?;
    m.add_class::<relational::PyManyToManyField>()?;
    Ok(())
}
