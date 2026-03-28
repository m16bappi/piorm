use super::operations::{
    AddFieldOp, AlterFieldOp, AlterModelOptionsOp, CreateModelOp, DeleteModelOp,
    MigrationOperation, OpType, RemoveFieldOp,
};
use super::state::{ModelOptions, ProjectState};

// ---------------------------------------------------------------------------
// AutoDetector
// ---------------------------------------------------------------------------

pub struct AutoDetector {
    /// State built from migration history on disk.
    pub from_state: ProjectState,
    /// State built from the current code / model registry.
    pub to_state: ProjectState,
}

impl AutoDetector {
    pub fn new(from_state: ProjectState, to_state: ProjectState) -> Self {
        Self { from_state, to_state }
    }

    /// Diff `from_state` → `to_state` and return the minimal set of operations
    /// needed to bring the database schema up to date.
    pub fn detect_changes(&self) -> Vec<Box<dyn MigrationOperation>> {
        let mut ops: Vec<Box<dyn MigrationOperation>> = Vec::new();

        // 1. New models
        for (key, to_model) in &self.to_state.models {
            if !self.from_state.models.contains_key(key) {
                ops.push(Box::new(CreateModelOp {
                    app_label: to_model.app_label.clone(),
                    name: to_model.name.clone(),
                    fields: to_model
                        .fields
                        .values()
                        .map(|fs| fs.field_def.clone())
                        .collect(),
                    options: to_model.options.clone(),
                }));
            }
        }

        // 2. Deleted models
        for (key, from_model) in &self.from_state.models {
            if !self.to_state.models.contains_key(key) {
                ops.push(Box::new(DeleteModelOp {
                    app_label: from_model.app_label.clone(),
                    name: from_model.name.clone(),
                    fields: from_model.fields.values().map(|fs| fs.field_def.clone()).collect(),
                    options: from_model.options.clone(),
                }));
            }
        }

        // 3. Field-level & options changes for models that exist in both states
        for (key, to_model) in &self.to_state.models {
            if let Some(from_model) = self.from_state.models.get(key) {
                self.detect_field_changes(from_model, to_model, &mut ops);
            }
        }

        // 4. Sort for deterministic ordering
        self.sort_operations(&mut ops);

        ops
    }

    fn detect_field_changes(
        &self,
        from: &super::state::ModelState,
        to: &super::state::ModelState,
        ops: &mut Vec<Box<dyn MigrationOperation>>,
    ) {
        // Added fields
        for (fname, to_field) in &to.fields {
            if !from.fields.contains_key(fname) {
                ops.push(Box::new(AddFieldOp {
                    app_label: to.app_label.clone(),
                    model: to.name.clone(),
                    field: to_field.field_def.clone(),
                }));
            }
        }

        // Removed fields
        for (fname, from_field) in &from.fields {
            if !to.fields.contains_key(fname) {
                ops.push(Box::new(RemoveFieldOp {
                    app_label: to.app_label.clone(),
                    model: to.name.clone(),
                    name: fname.clone(),
                    field: from_field.field_def.clone(),
                }));
            }
        }

        // Changed fields (compare by stable type string)
        for (fname, to_field) in &to.fields {
            if let Some(from_field) = from.fields.get(fname) {
                if from_field.field_type_str != to_field.field_type_str {
                    ops.push(Box::new(AlterFieldOp {
                        app_label: to.app_label.clone(),
                        model: to.name.clone(),
                        name: fname.clone(),
                        old_field: from_field.field_def.clone(),
                        new_field: to_field.field_def.clone(),
                    }));
                }
            }
        }

        // Changed model options
        if from.options != to.options {
            ops.push(Box::new(AlterModelOptionsOp {
                app_label: to.app_label.clone(),
                model: to.name.clone(),
                options: to.options.clone(),
            }));
        }
    }

    fn sort_operations(&self, ops: &mut Vec<Box<dyn MigrationOperation>>) {
        ops.sort_by_key(|op| match op.operation_type() {
            OpType::CreateModel => 0,
            OpType::AddField    => 1,
            OpType::AlterField  => 2,
            OpType::AlterIndex  => 3,
            OpType::RemoveField => 4,
            OpType::DeleteModel => 5,
        });
    }
}

// ---------------------------------------------------------------------------
// Migration name generation
// ---------------------------------------------------------------------------

/// Produce a migration filename (without `.toml`) such as `"0001_initial"`.
pub fn generate_migration_name(
    existing_migrations: &[String],
    ops: &[Box<dyn MigrationOperation>],
) -> String {
    let next_number = existing_migrations.len() + 1;
    let prefix = format!("{:04}", next_number);

    let suffix = match ops.first() {
        Some(op) => match op.operation_type() {
            OpType::CreateModel => "initial".to_string(),
            OpType::AddField    => format!("add_{}", op.primary_identifier()),
            OpType::RemoveField => format!("remove_{}", op.primary_identifier()),
            OpType::AlterField  => format!("alter_{}", op.primary_identifier()),
            _                   => "auto".to_string(),
        },
        None => "auto".to_string(),
    };

    format!("{}_{}", prefix, suffix)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::{DefaultValue, FieldDef, FieldType};
    use crate::migration::state::{FieldState, ModelState};
    use indexmap::IndexMap;
    use std::collections::HashMap;

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

    fn state_with(models: &[(&str, &str, &[(&str, FieldType)])]) -> ProjectState {
        let mut map = HashMap::new();
        for (app, name, fields) in models {
            let key = format!("{}.{}", app, name);
            map.insert(key, make_model_state(app, name, fields));
        }
        ProjectState { models: map }
    }

    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_new_model() {
        let from = state_with(&[]);
        let to = state_with(&[("blog", "Article", &[("id", FieldType::BigInt)])]);
        let det = AutoDetector::new(from, to);
        let ops = det.detect_changes();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation_type(), OpType::CreateModel);
        assert_eq!(ops[0].primary_identifier(), "Article");
    }

    #[test]
    fn test_detect_deleted_model() {
        let from = state_with(&[("blog", "Article", &[("id", FieldType::BigInt)])]);
        let to = state_with(&[]);
        let det = AutoDetector::new(from, to);
        let ops = det.detect_changes();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation_type(), OpType::DeleteModel);
    }

    #[test]
    fn test_detect_added_field() {
        let from = state_with(&[("blog", "Article", &[("id", FieldType::BigInt)])]);
        let to = state_with(&[(
            "blog",
            "Article",
            &[
                ("id", FieldType::BigInt),
                ("title", FieldType::Char { max_length: 200 }),
            ],
        )]);
        let det = AutoDetector::new(from, to);
        let ops = det.detect_changes();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation_type(), OpType::AddField);
    }

    #[test]
    fn test_detect_removed_field() {
        let from = state_with(&[(
            "blog",
            "Article",
            &[
                ("id", FieldType::BigInt),
                ("title", FieldType::Char { max_length: 200 }),
            ],
        )]);
        let to = state_with(&[("blog", "Article", &[("id", FieldType::BigInt)])]);
        let det = AutoDetector::new(from, to);
        let ops = det.detect_changes();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation_type(), OpType::RemoveField);
    }

    #[test]
    fn test_detect_altered_field() {
        let from = state_with(&[(
            "blog",
            "Article",
            &[
                ("id", FieldType::BigInt),
                ("title", FieldType::Char { max_length: 100 }),
            ],
        )]);
        let to = state_with(&[(
            "blog",
            "Article",
            &[
                ("id", FieldType::BigInt),
                ("title", FieldType::Char { max_length: 200 }),
            ],
        )]);
        let det = AutoDetector::new(from, to);
        let ops = det.detect_changes();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation_type(), OpType::AlterField);
    }

    #[test]
    fn test_no_spurious_changes() {
        let fields = &[
            ("id", FieldType::BigInt),
            ("title", FieldType::Char { max_length: 200 }),
        ];
        let from = state_with(&[("blog", "Article", fields)]);
        let to = state_with(&[("blog", "Article", fields)]);
        let det = AutoDetector::new(from, to);
        let ops = det.detect_changes();
        assert!(ops.is_empty(), "expected no changes, got: {:?}", ops.len());
    }

    #[test]
    fn test_sort_create_before_add_field() {
        // Two models: create + add field on another → create comes first regardless
        let from = state_with(&[("blog", "Tag", &[("id", FieldType::BigInt)])]);
        let to = state_with(&[
            ("blog", "Tag", &[
                ("id", FieldType::BigInt),
                ("slug", FieldType::Char { max_length: 50 }),
            ]),
            ("blog", "Article", &[("id", FieldType::BigInt)]),
        ]);
        let det = AutoDetector::new(from, to);
        let ops = det.detect_changes();
        assert!(ops.len() >= 2);
        let create_pos = ops.iter().position(|o| o.operation_type() == OpType::CreateModel).unwrap();
        let add_pos = ops.iter().position(|o| o.operation_type() == OpType::AddField).unwrap();
        assert!(create_pos < add_pos, "CreateModel must precede AddField");
    }

    #[test]
    fn test_migration_name_initial() {
        let ops: Vec<Box<dyn MigrationOperation>> = vec![Box::new(CreateModelOp {
            app_label: "blog".to_string(),
            name: "Article".to_string(),
            fields: vec![],
            options: ModelOptions::default(),
        })];
        let name = generate_migration_name(&[], &ops);
        assert_eq!(name, "0001_initial");
    }

    #[test]
    fn test_migration_name_add_field() {
        let ops: Vec<Box<dyn MigrationOperation>> = vec![Box::new(AddFieldOp {
            app_label: "blog".to_string(),
            model: "Article".to_string(),
            field: make_field_def("title", FieldType::Text),
        })];
        let existing = vec!["0001_initial".to_string()];
        let name = generate_migration_name(&existing, &ops);
        assert_eq!(name, "0002_add_article_title");
    }

    #[test]
    fn test_migration_name_empty_ops() {
        let name = generate_migration_name(&[], &[]);
        assert_eq!(name, "0001_auto");
    }
}
