use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;

use crate::fields::FieldDef;
use crate::model::def::{ConstraintDef, IndexDef, ModelDef};
use crate::model::registry::ModelRegistry;
use super::file::MigrationFile;
use super::operations::MigrationOperation;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A complete, immutable snapshot of the schema at a given migration version.
#[derive(Debug, Clone, Default)]
pub struct ProjectState {
    /// Key: `"{app_label}.{ModelName}"`
    pub models: HashMap<String, ModelState>,
}

#[derive(Debug, Clone)]
pub struct ModelState {
    pub app_label: String,
    pub name: String,
    pub fields: IndexMap<String, FieldState>,
    pub options: ModelOptions,
}

#[derive(Debug, Clone)]
pub struct FieldState {
    pub name: String,
    /// Stable string for change-detection (e.g. `"CharField(max_length=100)"`).
    pub field_type_str: String,
    pub field_def: FieldDef,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelOptions {
    pub db_table: Option<String>,
    /// Ordering expressed as `"field"` / `"-field"` strings.
    pub ordering: Vec<String>,
    pub unique_together: Vec<Vec<String>>,
    pub indexes: Vec<IndexDef>,
    pub constraints: Vec<ConstraintDef>,
    pub managed: bool,
}

// ---------------------------------------------------------------------------
// Conversions from core model types
// ---------------------------------------------------------------------------

impl ModelState {
    pub fn from_model_def(def: &ModelDef) -> Self {
        let fields = def
            .fields
            .iter()
            .map(|(name, fd)| {
                let fs = FieldState {
                    name: name.clone(),
                    field_type_str: fd.field_type.to_type_str(),
                    field_def: fd.clone(),
                };
                (name.clone(), fs)
            })
            .collect();

        let ordering: Vec<String> = def
            .ordering
            .iter()
            .map(|o| {
                if o.descending {
                    format!("-{}", o.field)
                } else {
                    o.field.clone()
                }
            })
            .collect();

        let options = ModelOptions {
            db_table: Some(def.table_name.clone()),
            ordering,
            unique_together: def.unique_together.clone(),
            indexes: def.indexes.clone(),
            constraints: def.constraints.clone(),
            managed: def.managed,
        };

        ModelState {
            app_label: def.app_label.clone(),
            name: def.model_name.clone(),
            fields,
            options,
        }
    }
}

// ---------------------------------------------------------------------------
// ProjectState constructors
// ---------------------------------------------------------------------------

impl ProjectState {
    /// Build a state snapshot from the current `ModelRegistry` (represents the
    /// code's current model definitions, not the on-disk migration history).
    pub fn from_registry() -> Self {
        let registry = ModelRegistry::global();
        let guard = registry.read().unwrap();
        let mut state = ProjectState::default();
        for model_def in guard.all() {
            if model_def.abstract_model || model_def.proxy {
                continue;
            }
            let key = format!("{}.{}", model_def.app_label, model_def.model_name);
            state.models.insert(key, ModelState::from_model_def(model_def));
        }
        state
    }

    /// Build a state snapshot by replaying all `.toml` migration files found in
    /// `migration_dir`, applied in lexicographic (filename-sorted) order.
    pub fn from_migrations(migration_dir: &Path) -> crate::Result<Self> {
        let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(migration_dir)
            .map_err(|e| crate::OrmpiError::Other(e.to_string()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();

        entries.sort(); // lexicographic → relies on 4-digit numeric prefix

        let mut state = ProjectState::default();
        for path in entries {
            let migration_file = MigrationFile::read_from_file(&path)
                .map_err(|e| crate::OrmpiError::Other(e.to_string()))?;
            for stored_op in migration_file.operations {
                let op: Box<dyn MigrationOperation> = stored_op.into_op();
                op.state_forwards(&mut state);
            }
        }
        Ok(state)
    }

    /// Apply a single operation (convenience wrapper).
    pub fn apply_operation(&mut self, op: &dyn MigrationOperation) {
        op.state_forwards(self);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::{DefaultValue, FieldType};
    use indexmap::IndexMap;

    fn make_field_def(name: &str, ft: FieldType) -> FieldDef {
        FieldDef {
            name: name.to_string(),
            attname: name.to_string(),
            db_column: None,
            field_type: ft,
            null: false,
            unique: false,
            primary_key: false,
            db_index: false,
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
        }
    }

    fn make_model_state(app: &str, name: &str, fields: &[(&str, FieldType)]) -> ModelState {
        let mut field_map = IndexMap::new();
        for (fname, ft) in fields {
            let fd = make_field_def(fname, ft.clone());
            field_map.insert(
                fname.to_string(),
                FieldState {
                    name: fname.to_string(),
                    field_type_str: ft.to_type_str(),
                    field_def: fd,
                },
            );
        }
        ModelState {
            app_label: app.to_string(),
            name: name.to_string(),
            fields: field_map,
            options: ModelOptions {
                managed: true,
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_from_registry_smoke() {
        // Just verify it doesn't panic and returns a valid state.
        let state = ProjectState::from_registry();
        // Registry may or may not have models registered in this test run.
        let _ = state.models.len();
    }

    #[test]
    fn test_apply_operation_adds_model() {
        use crate::migration::operations::CreateModelOp;
        let mut state = ProjectState::default();
        let op = CreateModelOp {
            app_label: "blog".to_string(),
            name: "Article".to_string(),
            fields: vec![make_field_def("id", FieldType::BigInt)],
            options: ModelOptions { managed: true, ..Default::default() },
        };
        state.apply_operation(&op);
        assert!(state.models.contains_key("blog.Article"));
    }

    #[test]
    fn test_from_migrations_replays_create_and_add_field() {
        use crate::migration::file::MigrationFile;
        use crate::migration::operations::{StoredField, StoredModelOptions, StoredOperation};

        let dir = tempfile::tempdir().unwrap();

        // Migration 1: CreateModel Article with id field
        let mig1 = MigrationFile::new(
            "0001_initial",
            vec![],
            vec![StoredOperation::CreateModel {
                app_label: "blog".to_string(),
                name: "Article".to_string(),
                fields: vec![StoredField {
                    name: "id".to_string(),
                    attname: Some("id".to_string()),
                    field_type_str: "BigIntegerField".to_string(),
                    null: false,
                    blank: false,
                    unique: false,
                    primary_key: true,
                    db_index: false,
                    editable: false,
                    max_length: None,
                    help_text: None,
                    auto_increment: false,
                }],
                options: StoredModelOptions { managed: true, ..Default::default() },
            }],
        );

        // Migration 2: AddField title to Article
        let mig2 = MigrationFile::new(
            "0002_add_title",
            vec!["0001_initial".to_string()],
            vec![StoredOperation::AddField {
                app_label: "blog".to_string(),
                model: "Article".to_string(),
                field: StoredField {
                    name: "title".to_string(),
                    attname: Some("title".to_string()),
                    field_type_str: "CharField(max_length=200)".to_string(),
                    null: false,
                    blank: false,
                    unique: false,
                    primary_key: false,
                    db_index: false,
                    editable: true,
                    max_length: Some(200),
                    help_text: None,
                    auto_increment: false,
                },
            }],
        );

        mig1.write_to_file(&dir.path().join("0001_initial.toml")).unwrap();
        mig2.write_to_file(&dir.path().join("0002_add_title.toml")).unwrap();

        let state = ProjectState::from_migrations(dir.path()).unwrap();
        let article = state.models.get("blog.Article").expect("Article should exist");
        assert!(article.fields.contains_key("id"), "should have id field");
        assert!(article.fields.contains_key("title"), "should have title field");
        assert_eq!(
            article.fields["title"].field_type_str,
            "CharField(max_length=200)"
        );
    }
}
