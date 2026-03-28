use std::path::Path;

use serde::{Deserialize, Serialize};

use super::operations::{MigrationOperation, StoredOperation};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Top-level structure of a `.toml` migration file (new format used by 7.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationFile {
    pub migration: MigrationMeta,
    pub operations: Vec<StoredOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationMeta {
    pub name: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub replaces: Vec<String>,
}

// ---------------------------------------------------------------------------
// Impl
// ---------------------------------------------------------------------------

impl MigrationFile {
    /// Create a new `MigrationFile` from its parts.
    pub fn new(name: &str, dependencies: Vec<String>, stored_ops: Vec<StoredOperation>) -> Self {
        MigrationFile {
            migration: MigrationMeta {
                name: name.to_string(),
                dependencies,
                replaces: vec![],
            },
            operations: stored_ops,
        }
    }

    /// Serialize to TOML and write to `path`.
    pub fn write_to_file(&self, path: &Path) -> crate::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::OrmpiError::MigrationError(format!("TOML serialize error: {e}")))?;
        std::fs::write(path, content)
            .map_err(|e| crate::OrmpiError::MigrationError(format!("Write error: {e}")))?;
        Ok(())
    }

    /// Read and deserialize a TOML migration file from `path`.
    pub fn read_from_file(path: &Path) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::OrmpiError::MigrationError(format!("Read error: {e}")))?;
        let file: MigrationFile = toml::from_str(&content)
            .map_err(|e| crate::OrmpiError::MigrationError(format!("TOML parse error: {e}")))?;
        Ok(file)
    }

    /// Convert stored operations to boxed trait objects.
    pub fn to_operations(&self) -> crate::Result<Vec<Box<dyn MigrationOperation>>> {
        Ok(self.operations.iter().map(|op| op.clone().into_op()).collect())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::operations::{StoredField, StoredModelOptions};

    fn make_stored_field(name: &str, type_str: &str) -> StoredField {
        StoredField {
            name: name.to_string(),
            attname: Some(name.to_string()),
            field_type_str: type_str.to_string(),
            null: false,
            blank: false,
            unique: false,
            primary_key: name == "id",
            db_index: false,
            editable: name != "id",
            max_length: None,
            help_text: None,
            auto_increment: false,
        }
    }

    #[test]
    fn test_roundtrip_write_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0001_initial.toml");

        let file = MigrationFile::new(
            "0001_initial",
            vec![],
            vec![StoredOperation::CreateModel {
                app_label: "blog".to_string(),
                name: "Post".to_string(),
                fields: vec![
                    make_stored_field("id", "BigIntegerField"),
                    make_stored_field("title", "CharField(max_length=200)"),
                ],
                options: StoredModelOptions { managed: true, ..Default::default() },
            }],
        );

        file.write_to_file(&path).unwrap();
        let loaded = MigrationFile::read_from_file(&path).unwrap();

        assert_eq!(loaded.migration.name, "0001_initial");
        assert_eq!(loaded.operations.len(), 1);

        match &loaded.operations[0] {
            StoredOperation::CreateModel { name, fields, .. } => {
                assert_eq!(name, "Post");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "id");
                assert_eq!(fields[1].name, "title");
            }
            other => panic!("unexpected op: {:?}", other),
        }
    }

    #[test]
    fn test_roundtrip_preserves_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0002_add_body.toml");

        let file = MigrationFile::new(
            "0002_add_body",
            vec!["0001_initial".to_string()],
            vec![StoredOperation::AddField {
                app_label: "blog".to_string(),
                model: "Post".to_string(),
                field: make_stored_field("body", "TextField"),
            }],
        );

        file.write_to_file(&path).unwrap();
        let loaded = MigrationFile::read_from_file(&path).unwrap();

        assert_eq!(loaded.migration.dependencies, vec!["0001_initial"]);
    }

    #[test]
    fn test_to_operations_returns_correct_types() {
        let file = MigrationFile::new(
            "0001_initial",
            vec![],
            vec![
                StoredOperation::CreateModel {
                    app_label: "app".to_string(),
                    name: "User".to_string(),
                    fields: vec![make_stored_field("id", "BigIntegerField")],
                    options: StoredModelOptions { managed: true, ..Default::default() },
                },
                StoredOperation::AddField {
                    app_label: "app".to_string(),
                    model: "User".to_string(),
                    field: make_stored_field("email", "TextField"),
                },
            ],
        );

        let ops = file.to_operations().unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].name(), "CreateModel");
        assert_eq!(ops[1].name(), "AddField");
    }
}
