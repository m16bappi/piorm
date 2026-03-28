use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use once_cell::sync::Lazy;

use super::def::ModelDef;

static MODEL_REGISTRY: Lazy<Arc<RwLock<ModelRegistry>>> =
    Lazy::new(|| Arc::new(RwLock::new(ModelRegistry::new())));

pub struct ModelRegistry {
    models: HashMap<String, ModelDef>, // key: "app_label.ModelName"
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    pub fn global() -> Arc<RwLock<ModelRegistry>> {
        MODEL_REGISTRY.clone()
    }

    pub fn register(&mut self, def: ModelDef) {
        let key = format!("{}.{}", def.app_label, def.model_name);
        self.models.insert(key, def);
    }

    pub fn get(&self, app_label: &str, model_name: &str) -> Option<&ModelDef> {
        let key = format!("{}.{}", app_label, model_name);
        self.models.get(&key)
    }

    pub fn get_by_name(&self, model_name: &str) -> Option<&ModelDef> {
        self.models.values().find(|m| m.model_name == model_name)
    }

    pub fn all(&self) -> impl Iterator<Item = &ModelDef> {
        self.models.values()
    }

    pub fn get_all_for_app(&self, app_label: &str) -> Vec<&ModelDef> {
        self.models
            .values()
            .filter(|m| m.app_label == app_label)
            .collect()
    }

    pub fn count(&self) -> usize {
        self.models.len()
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::{DefaultValue, FieldType};
    use indexmap::IndexMap;

    fn make_model(app: &str, name: &str) -> ModelDef {
        use crate::fields::FieldDef;
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
                verbose_name: None,
                related_name: None,
            },
        );
        ModelDef {
            app_label: app.to_string(),
            model_name: name.to_string(),
            table_name: format!("{}_{}", app, name.to_lowercase()),
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
    fn test_register_and_get() {
        let mut reg = ModelRegistry::new();
        reg.register(make_model("blog", "Article"));
        let m = reg.get("blog", "Article").expect("should find Article");
        assert_eq!(m.model_name, "Article");
        assert_eq!(m.app_label, "blog");
    }

    #[test]
    fn test_get_by_name() {
        let mut reg = ModelRegistry::new();
        reg.register(make_model("blog", "Article"));
        reg.register(make_model("auth", "User"));
        let m = reg.get_by_name("User").expect("should find User");
        assert_eq!(m.app_label, "auth");
    }

    #[test]
    fn test_get_all_for_app() {
        let mut reg = ModelRegistry::new();
        reg.register(make_model("blog", "Article"));
        reg.register(make_model("blog", "Comment"));
        reg.register(make_model("auth", "User"));
        let blog_models = reg.get_all_for_app("blog");
        assert_eq!(blog_models.len(), 2);
    }

    #[test]
    fn test_missing_returns_none() {
        let reg = ModelRegistry::new();
        assert!(reg.get("foo", "Bar").is_none());
        assert!(reg.get_by_name("Nonexistent").is_none());
    }
}
