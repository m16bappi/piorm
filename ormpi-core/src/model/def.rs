use indexmap::IndexMap;

use crate::fields::FieldDef;

/// Full metadata for a registered model class.
#[derive(Debug, Clone)]
pub struct ModelDef {
    pub app_label: String,
    pub model_name: String,           // PascalCase class name
    pub table_name: String,           // snake_case table name
    pub fields: IndexMap<String, FieldDef>, // ordered, keyed by attname
    pub pk_field: String,             // attname of the PK field
    pub ordering: Vec<OrderSpec>,
    pub unique_together: Vec<Vec<String>>,
    pub index_together: Vec<Vec<String>>, // deprecated but supported
    pub indexes: Vec<IndexDef>,
    pub constraints: Vec<ConstraintDef>,
    pub m2m_fields: Vec<M2MFieldDef>,
    pub abstract_model: bool,
    pub proxy: bool,
    pub managed: bool,                // if false, ormpi won't create/migrate the table
    pub default_manager_name: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OrderSpec {
    pub field: String,
    pub descending: bool,
    pub nulls_last: bool,
    pub nulls_first: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IndexDef {
    pub name: Option<String>,
    pub fields: Vec<String>,
    pub unique: bool,
    pub condition: Option<String>, // partial index WHERE clause
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConstraintDef {
    pub name: String,
    pub kind: ConstraintKind,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ConstraintKind {
    UniqueConstraint {
        fields: Vec<String>,
        condition: Option<String>,
    },
    CheckConstraint {
        check: String,
    },
}

#[derive(Debug, Clone)]
pub struct M2MFieldDef {
    pub field_name: String,          // attribute name on the model
    pub to_model: String,
    pub through_model: Option<String>,
    pub junction_table: String,      // computed: "{from_model}_{field_name}"
    pub from_column: String,         // "{from_model_lower}_id"
    pub to_column: String,           // "{to_model_lower}_id"
}

impl ModelDef {
    /// Compute the default table name from the model class name (PascalCase → snake_case).
    /// e.g. `"ArticleComment"` → `"article_comment"`
    pub fn default_table_name(model_name: &str) -> String {
        let mut out = String::new();
        for (i, ch) in model_name.chars().enumerate() {
            if ch.is_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(ch.to_lowercase().next().unwrap());
        }
        out
    }

    /// Get the primary key FieldDef.
    pub fn pk(&self) -> Option<&FieldDef> {
        self.fields.get(&self.pk_field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::{DefaultValue, FieldType};

    fn make_model() -> ModelDef {
        let mut fields = IndexMap::new();
        fields.insert(
            "id".to_string(),
            FieldDef {
                name: "id".to_string(),
                attname: "id".to_string(),
                db_column: None,
                field_type: FieldType::AutoBigInt,
                primary_key: true,
                null: false,
                unique: false,
                db_index: false,
                editable: false,
                default: DefaultValue::None,
                choices: vec![],
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                verbose_name: Some("ID".to_string()),
                related_name: None,
            },
        );
        ModelDef {
            app_label: "blog".to_string(),
            model_name: "Article".to_string(),
            table_name: "article".to_string(),
            fields,
            pk_field: "id".to_string(),
            ordering: vec![],
            unique_together: vec![],
            index_together: vec![],
            indexes: vec![],
            constraints: vec![],
            m2m_fields: vec![],
            abstract_model: false,
            proxy: false,
            managed: true,
            default_manager_name: "objects".to_string(),
        }
    }

    #[test]
    fn test_default_table_name() {
        assert_eq!(ModelDef::default_table_name("Article"), "article");
        assert_eq!(ModelDef::default_table_name("UserProfile"), "user_profile");
        assert_eq!(ModelDef::default_table_name("ArticleComment"), "article_comment");
    }

    #[test]
    fn test_pk_lookup() {
        let m = make_model();
        let pk = m.pk().expect("pk field should exist");
        assert_eq!(pk.name, "id");
        assert!(pk.primary_key);
    }
}
