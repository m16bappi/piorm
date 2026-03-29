use sea_query::{Alias, ColumnDef, ForeignKeyAction, Index, Table};

use crate::backend::QueryBackend;
use crate::fields::FieldType;
use crate::fields::OnDelete;
use crate::model::def::ModelDef;
use crate::model::registry::ModelRegistry;

/// All DDL statements needed to create a model's tables and indexes.
#[derive(Debug, Clone)]
pub struct DdlStatements {
    /// CREATE TABLE statement for the model's main table.
    pub create_table: String,
    /// CREATE [UNIQUE] INDEX statements for db_index fields, unique_together, and Meta.indexes.
    pub create_indexes: Vec<String>,
    /// ALTER TABLE ADD FOREIGN KEY / REFERENCES statements.
    pub create_fk_constraints: Vec<String>,
    /// CREATE TABLE statements for auto-generated M2M junction tables.
    pub create_m2m_tables: Vec<String>,
}

/// Convert a `ModelDef` into all SQL DDL statements needed to create its tables.
pub fn model_to_ddl(model_def: &ModelDef, backend: QueryBackend) -> DdlStatements {
    // ── 1. CREATE TABLE ───────────────────────────────────────────────────────

    let mut table = Table::create()
        .table(Alias::new(&model_def.table_name))
        .if_not_exists()
        .to_owned();

    for field in model_def.fields.values() {
        if let Some(col_def) = field.to_column_def() {
            table.col(col_def);
        }
    }

    let create_table = backend.build_table_create(&table);

    // ── 2. CREATE INDEX statements ────────────────────────────────────────────

    let mut create_indexes: Vec<String> = Vec::new();

    // db_index=True fields (not already pk or unique — those get implicit indexes)
    for field in model_def.fields.values() {
        if field.db_index && !field.unique && !field.primary_key {
            let idx_name = format!("idx_{}_{}", model_def.table_name, field.column_name());
            let idx = Index::create()
                .name(&idx_name)
                .table(Alias::new(&model_def.table_name))
                .col(Alias::new(&field.column_name()))
                .to_owned();
            create_indexes.push(backend.build_index_create(&idx));
        }
    }

    // unique_together groups
    for (i, unique_cols) in model_def.unique_together.iter().enumerate() {
        let name = format!("uniq_{}_{}", model_def.table_name, i);
        let mut idx = Index::create()
            .name(&name)
            .unique()
            .table(Alias::new(&model_def.table_name))
            .to_owned();
        for col in unique_cols {
            idx.col(Alias::new(col));
        }
        create_indexes.push(backend.build_index_create(&idx));
    }

    // Meta.indexes
    for index_def in &model_def.indexes {
        let name = index_def.name.clone().unwrap_or_else(|| {
            format!("idx_{}_{}", model_def.table_name, index_def.fields.join("_"))
        });
        let mut idx = Index::create()
            .name(&name)
            .table(Alias::new(&model_def.table_name))
            .to_owned();
        for col in &index_def.fields {
            idx.col(Alias::new(col));
        }
        if index_def.unique {
            idx.unique();
        }
        create_indexes.push(backend.build_index_create(&idx));
    }

    // ── 3. FK constraints ─────────────────────────────────────────────────────

    let mut create_fk_constraints: Vec<String> = Vec::new();

    let reg = ModelRegistry::global();
    let guard = reg.read().unwrap();

    for field in model_def.fields.values() {
        match &field.field_type {
            FieldType::ForeignKey {
                to,
                on_delete,
                db_column: _,
            }
            | FieldType::OneToOne { to, on_delete } => {
                // Resolve the referenced model's table name.
                let ref_table = guard
                    .get_by_name(to)
                    .map(|m| m.table_name.clone())
                    .unwrap_or_else(|| to.to_lowercase());

                let fk_sql = build_fk_alter(
                    &backend,
                    &model_def.table_name,
                    &field.column_name(),
                    &ref_table,
                    "id", // always reference pk `id`
                    *on_delete,
                );
                create_fk_constraints.push(fk_sql);
            }
            _ => {}
        }
    }

    drop(guard);

    // ── 4. M2M junction tables ────────────────────────────────────────────────

    let mut create_m2m_tables: Vec<String> = Vec::new();

    for m2m in &model_def.m2m_fields {
        if m2m.through_model.is_some() {
            continue; // custom through model — user manages it
        }
        let jt = Table::create()
            .table(Alias::new(&m2m.junction_table))
            .if_not_exists()
            .col(
                ColumnDef::new(Alias::new("id"))
                    .big_integer()
                    .auto_increment()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(Alias::new(&m2m.from_column))
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new(&m2m.to_column))
                    .big_integer()
                    .not_null(),
            )
            .to_owned();
        create_m2m_tables.push(backend.build_table_create(&jt));
    }

    DdlStatements {
        create_table,
        create_indexes,
        create_fk_constraints,
        create_m2m_tables,
    }
}

/// Build an `ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY ...` statement.
fn build_fk_alter(
    backend: &QueryBackend,
    from_table: &str,
    from_col: &str,
    to_table: &str,
    to_col: &str,
    on_delete: OnDelete,
) -> String {
    use crate::backend::DatabaseBackend;

    let on_delete_clause = match on_delete {
        OnDelete::Cascade => "CASCADE",
        OnDelete::SetNull => "SET NULL",
        OnDelete::SetDefault => "SET DEFAULT",
        OnDelete::Restrict => "RESTRICT",
        OnDelete::DoNothing | OnDelete::Protect => "NO ACTION",
    };

    match backend.0 {
        DatabaseBackend::Sqlite => {
            // SQLite doesn't support ALTER TABLE ADD FOREIGN KEY.
            // Return a comment so callers know to enable PRAGMA foreign_keys = ON.
            format!(
                "-- SQLite FK: {from_table}.{from_col} → {to_table}.{to_col} ON DELETE {on_delete_clause}"
            )
        }
        DatabaseBackend::Mysql => {
            let fk_name = format!("fk_{from_table}_{from_col}");
            format!(
                "ALTER TABLE `{from_table}` ADD CONSTRAINT `{fk_name}` \
                 FOREIGN KEY (`{from_col}`) REFERENCES `{to_table}` (`{to_col}`) \
                 ON DELETE {on_delete_clause}"
            )
        }
        DatabaseBackend::Postgres => {
            let fk_name = format!("fk_{from_table}_{from_col}");
            format!(
                r#"ALTER TABLE "{from_table}" ADD CONSTRAINT "{fk_name}" \
                 FOREIGN KEY ("{from_col}") REFERENCES "{to_table}" ("{to_col}") \
                 ON DELETE {on_delete_clause}"#
            )
        }
    }
}

/// Convert `OnDelete` to sea-query's `ForeignKeyAction`.
pub fn on_delete_to_sea(action: OnDelete) -> ForeignKeyAction {
    match action {
        OnDelete::Cascade => ForeignKeyAction::Cascade,
        OnDelete::SetNull => ForeignKeyAction::SetNull,
        OnDelete::SetDefault => ForeignKeyAction::SetDefault,
        OnDelete::Restrict => ForeignKeyAction::Restrict,
        OnDelete::DoNothing | OnDelete::Protect => ForeignKeyAction::NoAction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{DatabaseBackend, QueryBackend};
    use crate::fields::{DefaultValue, FieldDef, FieldType};
    use crate::model::def::{M2MFieldDef, ModelDef};
    use indexmap::IndexMap;

    fn sqlite() -> QueryBackend {
        QueryBackend(DatabaseBackend::Sqlite)
    }

    fn make_article_def() -> ModelDef {
        let mut fields = IndexMap::new();
        fields.insert(
            "id".to_string(),
            FieldDef {
                name: "id".into(),
                attname: "id".into(),
                db_column: None,
                field_type: FieldType::Int,
                primary_key: true,
                null: false,
                unique: false,
                db_index: false,
                editable: false,
                default: DefaultValue::None,
                choices: vec![],
                auto_increment: true,
                auto_now: false,
                auto_now_add: false,
                max_length: None,
                validators: vec![],
                verbose_name: None,
                related_name: None,
            },
        );
        fields.insert(
            "title".to_string(),
            FieldDef {
                name: "title".into(),
                attname: "title".into(),
                db_column: None,
                field_type: FieldType::Char { max_length: 200 },
                primary_key: false,
                null: false,
                unique: false,
                db_index: true, // generates an index
                editable: true,
                default: DefaultValue::None,
                choices: vec![],
                auto_increment: false,
                auto_now: false,
                auto_now_add: false,
                max_length: Some(200),
                validators: vec![],
                verbose_name: None,
                related_name: None,
            },
        );
        fields.insert(
            "author_id".to_string(),
            FieldDef {
                name: "author".into(),
                attname: "author_id".into(),
                db_column: None,
                field_type: FieldType::ForeignKey {
                    to: "User".into(),
                    on_delete: OnDelete::Cascade,
                    db_column: None,
                },
                primary_key: false,
                null: true,
                unique: false,
                db_index: true,
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
        );
        ModelDef {
            app_label: "blog".to_string(),
            model_name: "Article".to_string(),
            table_name: "blog_article".to_string(),
            fields,
            pk_field: "id".to_string(),
            ordering: vec![],
            unique_together: vec![vec!["title".to_string(), "author_id".to_string()]],
            index_together: vec![],
            indexes: vec![],
            constraints: vec![],
            m2m_fields: vec![M2MFieldDef {
                field_name: "tags".to_string(),
                to_model: "Tag".to_string(),
                through_model: None,
                junction_table: "blog_article_tags".to_string(),
                from_column: "article_id".to_string(),
                to_column: "tag_id".to_string(),
            }],
            abstract_model: false,
            proxy: false,
            managed: true,
            default_manager_name: "objects".to_string(),
        }
    }

    #[test]
    fn test_create_table_has_columns() {
        let ddl = model_to_ddl(&make_article_def(), sqlite());
        assert!(
            ddl.create_table.contains("blog_article"),
            "table name missing: {}",
            ddl.create_table
        );
        assert!(
            ddl.create_table.contains("title"),
            "title column missing: {}",
            ddl.create_table
        );
    }

    #[test]
    fn test_fk_column_in_create_table() {
        let ddl = model_to_ddl(&make_article_def(), sqlite());
        assert!(
            ddl.create_table.contains("author_id"),
            "FK column missing: {}",
            ddl.create_table
        );
    }

    #[test]
    fn test_db_index_generates_create_index() {
        let ddl = model_to_ddl(&make_article_def(), sqlite());
        let has_title_idx = ddl
            .create_indexes
            .iter()
            .any(|s| s.contains("title") && s.contains("INDEX"));
        assert!(has_title_idx, "expected index on title, got: {:?}", ddl.create_indexes);
    }

    #[test]
    fn test_unique_together_generates_unique_index() {
        let ddl = model_to_ddl(&make_article_def(), sqlite());
        let has_unique = ddl
            .create_indexes
            .iter()
            .any(|s| s.contains("UNIQUE") || s.contains("unique"));
        assert!(has_unique, "expected UNIQUE index, got: {:?}", ddl.create_indexes);
    }

    #[test]
    fn test_m2m_junction_table_generated() {
        let ddl = model_to_ddl(&make_article_def(), sqlite());
        assert_eq!(ddl.create_m2m_tables.len(), 1, "expected 1 M2M table");
        assert!(
            ddl.create_m2m_tables[0].contains("blog_article_tags"),
            "junction table name missing: {}",
            ddl.create_m2m_tables[0]
        );
    }

    #[test]
    fn test_fk_constraint_generated() {
        let ddl = model_to_ddl(&make_article_def(), sqlite());
        let has_fk = ddl
            .create_fk_constraints
            .iter()
            .any(|s| s.contains("author_id"));
        assert!(has_fk, "expected FK constraint, got: {:?}", ddl.create_fk_constraints);
    }
}
