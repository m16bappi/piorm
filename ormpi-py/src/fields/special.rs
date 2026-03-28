use crate::{impl_field_common, impl_simple_field};
use ormpi_core::fields::FieldType;
use pyo3::prelude::*;

// ── UUID / JSON / Binary fields ────────────────────────────────────────────

#[pyclass(name = "UUIDField")]
pub struct PyUUIDField {
    pub(crate) inner: ormpi_core::fields::FieldDef,
}
impl_simple_field!(PyUUIDField, "UUIDField", FieldType::Uuid);

#[pyclass(name = "JSONField")]
pub struct PyJSONField {
    pub(crate) inner: ormpi_core::fields::FieldDef,
}
impl_simple_field!(PyJSONField, "JSONField", FieldType::Json);

#[pyclass(name = "BinaryField")]
pub struct PyBinaryField {
    pub(crate) inner: ormpi_core::fields::FieldDef,
}
impl_simple_field!(PyBinaryField, "BinaryField", FieldType::Binary);

// ── IP address fields ──────────────────────────────────────────────────────

#[pyclass(name = "IPAddressField")]
pub struct PyIPAddressField {
    pub(crate) inner: ormpi_core::fields::FieldDef,
}
impl_simple_field!(PyIPAddressField, "IPAddressField", FieldType::IpAddress);

#[pyclass(name = "GenericIPAddressField")]
pub struct PyGenericIPAddressField {
    pub(crate) inner: ormpi_core::fields::FieldDef,
}
impl_simple_field!(
    PyGenericIPAddressField,
    "GenericIPAddressField",
    FieldType::GenericIpAddress
);

// ── Rust unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ormpi_core::fields::{DefaultValue, FieldDef};

    #[test]
    fn test_uuid_field_repr() {
        let f = PyUUIDField {
            inner: FieldDef {
                name: "id".into(),
                attname: "id".into(),
                db_column: None,
                field_type: FieldType::Uuid,
                null: false,
                unique: false,
                db_index: false,
                primary_key: true,
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
        assert_eq!(f.__repr__(), "<UUIDField 'id'>");
        assert!(f.primary_key());
    }
}
