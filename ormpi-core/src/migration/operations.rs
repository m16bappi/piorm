use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::fields::{DefaultValue, FieldDef, FieldType};
use crate::model::def::{ConstraintDef, IndexDef};
use super::state::{FieldState, ModelOptions, ModelState, ProjectState};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

pub trait MigrationOperation: std::fmt::Debug + Send + Sync {
    fn operation_type(&self) -> OpType;
    /// Apply this operation to advance state forward (no DB I/O — pure state).
    fn state_forwards(&self, state: &mut ProjectState);
    /// Reverse this operation (stubbed — Task 7.2 will implement DB execution).
    fn state_backwards(&self, _state: &mut ProjectState) {}
    /// Short identifier used for migration name generation.
    fn primary_identifier(&self) -> String;
}

// ---------------------------------------------------------------------------
// OpType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpType {
    CreateModel,
    AddField,
    AlterField,
    AlterIndex,
    RemoveField,
    DeleteModel,
}

// ---------------------------------------------------------------------------
// StoredField — serializable subset of FieldDef
// ---------------------------------------------------------------------------

fn default_editable() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredField {
    pub name: String,
    #[serde(default)]
    pub attname: Option<String>,
    pub field_type_str: String,
    #[serde(default)]
    pub null: bool,
    #[serde(default)]
    pub blank: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub primary_key: bool,
    #[serde(default)]
    pub db_index: bool,
    #[serde(default = "default_editable")]
    pub editable: bool,
    #[serde(default)]
    pub max_length: Option<u32>,
    #[serde(default)]
    pub help_text: Option<String>,
}

impl From<&FieldDef> for StoredField {
    fn from(fd: &FieldDef) -> Self {
        StoredField {
            name: fd.name.clone(),
            attname: Some(fd.attname.clone()),
            field_type_str: fd.field_type.to_type_str(),
            null: fd.null,
            blank: fd.blank,
            unique: fd.unique,
            primary_key: fd.primary_key,
            db_index: fd.db_index,
            editable: fd.editable,
            max_length: fd.max_length,
            help_text: if fd.help_text.is_empty() {
                None
            } else {
                Some(fd.help_text.clone())
            },
        }
    }
}

impl StoredField {
    pub fn to_field_def(&self) -> FieldDef {
        let field_type =
            FieldType::from_type_str(&self.field_type_str).unwrap_or(FieldType::Text);
        let attname = self
            .attname
            .clone()
            .unwrap_or_else(|| self.name.clone());
        FieldDef {
            name: self.name.clone(),
            attname,
            db_column: None,
            field_type,
            null: self.null,
            blank: self.blank,
            unique: self.unique,
            primary_key: self.primary_key,
            db_index: self.db_index,
            editable: self.editable,
            default: DefaultValue::None,
            choices: vec![],
            auto_now: false,
            auto_now_add: false,
            max_length: self.max_length,
            validators: vec![],
            help_text: self.help_text.clone().unwrap_or_default(),
            verbose_name: None,
            related_name: None,
        }
    }

    pub fn to_field_state(&self) -> FieldState {
        FieldState {
            name: self.name.clone(),
            field_type_str: self.field_type_str.clone(),
            field_def: self.to_field_def(),
        }
    }
}

// ---------------------------------------------------------------------------
// Concrete in-memory operation structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CreateModelOp {
    pub app_label: String,
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub options: ModelOptions,
}

impl MigrationOperation for CreateModelOp {
    fn operation_type(&self) -> OpType {
        OpType::CreateModel
    }
    fn state_forwards(&self, state: &mut ProjectState) {
        let mut field_map: IndexMap<String, FieldState> = IndexMap::new();
        for fd in &self.fields {
            field_map.insert(
                fd.name.clone(),
                FieldState {
                    name: fd.name.clone(),
                    field_type_str: fd.field_type.to_type_str(),
                    field_def: fd.clone(),
                },
            );
        }
        let key = format!("{}.{}", self.app_label, self.name);
        state.models.insert(
            key,
            ModelState {
                app_label: self.app_label.clone(),
                name: self.name.clone(),
                fields: field_map,
                options: self.options.clone(),
            },
        );
    }
    fn primary_identifier(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, Clone)]
pub struct DeleteModelOp {
    pub app_label: String,
    pub name: String,
}

impl MigrationOperation for DeleteModelOp {
    fn operation_type(&self) -> OpType {
        OpType::DeleteModel
    }
    fn state_forwards(&self, state: &mut ProjectState) {
        let key = format!("{}.{}", self.app_label, self.name);
        state.models.remove(&key);
    }
    fn primary_identifier(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, Clone)]
pub struct AddFieldOp {
    pub app_label: String,
    pub model: String,
    pub field: FieldDef,
}

impl MigrationOperation for AddFieldOp {
    fn operation_type(&self) -> OpType {
        OpType::AddField
    }
    fn state_forwards(&self, state: &mut ProjectState) {
        let key = format!("{}.{}", self.app_label, self.model);
        if let Some(model_state) = state.models.get_mut(&key) {
            model_state.fields.insert(
                self.field.name.clone(),
                FieldState {
                    name: self.field.name.clone(),
                    field_type_str: self.field.field_type.to_type_str(),
                    field_def: self.field.clone(),
                },
            );
        }
    }
    fn primary_identifier(&self) -> String {
        format!("{}_{}", self.model.to_lowercase(), self.field.name)
    }
}

#[derive(Debug, Clone)]
pub struct RemoveFieldOp {
    pub app_label: String,
    pub model: String,
    pub name: String,
}

impl MigrationOperation for RemoveFieldOp {
    fn operation_type(&self) -> OpType {
        OpType::RemoveField
    }
    fn state_forwards(&self, state: &mut ProjectState) {
        let key = format!("{}.{}", self.app_label, self.model);
        if let Some(model_state) = state.models.get_mut(&key) {
            model_state.fields.shift_remove(&self.name);
        }
    }
    fn primary_identifier(&self) -> String {
        format!("{}_{}", self.model.to_lowercase(), self.name)
    }
}

#[derive(Debug, Clone)]
pub struct AlterFieldOp {
    pub app_label: String,
    pub model: String,
    pub name: String,
    pub old_field: FieldDef,
    pub new_field: FieldDef,
}

impl MigrationOperation for AlterFieldOp {
    fn operation_type(&self) -> OpType {
        OpType::AlterField
    }
    fn state_forwards(&self, state: &mut ProjectState) {
        let key = format!("{}.{}", self.app_label, self.model);
        if let Some(model_state) = state.models.get_mut(&key) {
            model_state.fields.insert(
                self.name.clone(),
                FieldState {
                    name: self.name.clone(),
                    field_type_str: self.new_field.field_type.to_type_str(),
                    field_def: self.new_field.clone(),
                },
            );
        }
    }
    fn primary_identifier(&self) -> String {
        format!("{}_{}", self.model.to_lowercase(), self.name)
    }
}

#[derive(Debug, Clone)]
pub struct AlterModelOptionsOp {
    pub app_label: String,
    pub model: String,
    pub options: ModelOptions,
}

impl MigrationOperation for AlterModelOptionsOp {
    fn operation_type(&self) -> OpType {
        OpType::AlterIndex
    }
    fn state_forwards(&self, state: &mut ProjectState) {
        let key = format!("{}.{}", self.app_label, self.model);
        if let Some(model_state) = state.models.get_mut(&key) {
            model_state.options = self.options.clone();
        }
    }
    fn primary_identifier(&self) -> String {
        self.model.to_lowercase()
    }
}

// ---------------------------------------------------------------------------
// StoredOperation — TOML-serializable migration file representation
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

/// Serializable model options stored in TOML migration files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredModelOptions {
    #[serde(default)]
    pub db_table: Option<String>,
    #[serde(default)]
    pub ordering: Vec<String>,
    #[serde(default)]
    pub unique_together: Vec<Vec<String>>,
    #[serde(default)]
    pub indexes: Vec<IndexDef>,
    #[serde(default)]
    pub constraints: Vec<ConstraintDef>,
    #[serde(default = "default_true")]
    pub managed: bool,
}

impl From<&ModelOptions> for StoredModelOptions {
    fn from(o: &ModelOptions) -> Self {
        StoredModelOptions {
            db_table: o.db_table.clone(),
            ordering: o.ordering.clone(),
            unique_together: o.unique_together.clone(),
            indexes: o.indexes.clone(),
            constraints: o.constraints.clone(),
            managed: o.managed,
        }
    }
}

impl From<&StoredModelOptions> for ModelOptions {
    fn from(s: &StoredModelOptions) -> Self {
        ModelOptions {
            db_table: s.db_table.clone(),
            ordering: s.ordering.clone(),
            unique_together: s.unique_together.clone(),
            indexes: s.indexes.clone(),
            constraints: s.constraints.clone(),
            managed: s.managed,
        }
    }
}

/// Tagged enum for TOML file serialization/deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum StoredOperation {
    CreateModel {
        app_label: String,
        name: String,
        #[serde(default)]
        fields: Vec<StoredField>,
        #[serde(default)]
        options: StoredModelOptions,
    },
    DeleteModel {
        app_label: String,
        name: String,
    },
    AddField {
        app_label: String,
        model: String,
        field: StoredField,
    },
    RemoveField {
        app_label: String,
        model: String,
        name: String,
    },
    AlterField {
        app_label: String,
        model: String,
        old_field: StoredField,
        new_field: StoredField,
    },
    AlterModelOptions {
        app_label: String,
        model: String,
        #[serde(default)]
        options: StoredModelOptions,
    },
}

impl StoredOperation {
    /// Convert to a boxed `MigrationOperation` for replaying state.
    pub fn into_op(self) -> Box<dyn MigrationOperation> {
        match self {
            StoredOperation::CreateModel { app_label, name, fields, options } => {
                Box::new(CreateModelOp {
                    app_label,
                    name,
                    fields: fields.iter().map(|f| f.to_field_def()).collect(),
                    options: ModelOptions::from(&options),
                })
            }
            StoredOperation::DeleteModel { app_label, name } => {
                Box::new(DeleteModelOp { app_label, name })
            }
            StoredOperation::AddField { app_label, model, field } => {
                Box::new(AddFieldOp {
                    app_label,
                    model,
                    field: field.to_field_def(),
                })
            }
            StoredOperation::RemoveField { app_label, model, name } => {
                Box::new(RemoveFieldOp { app_label, model, name })
            }
            StoredOperation::AlterField { app_label, model, old_field, new_field } => {
                Box::new(AlterFieldOp {
                    app_label,
                    model,
                    name: new_field.name.clone(),
                    old_field: old_field.to_field_def(),
                    new_field: new_field.to_field_def(),
                })
            }
            StoredOperation::AlterModelOptions { app_label, model, options } => {
                Box::new(AlterModelOptionsOp {
                    app_label,
                    model,
                    options: ModelOptions::from(&options),
                })
            }
        }
    }
}

/// Top-level structure of a `.toml` migration file.
#[derive(Debug, Serialize, Deserialize)]
pub struct MigrationFile {
    pub name: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub operations: Vec<StoredOperation>,
}
