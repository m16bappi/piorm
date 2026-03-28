use sea_query::{Alias, Expr, ExprTrait, SelectStatement};

use crate::fields::FieldType;
use crate::model::def::ModelDef;
use crate::model::registry::ModelRegistry;
use crate::{OrmpiError, Result};

/// Add LEFT JOINs + aliased columns to `stmt` for each FK path in `related_paths`.
///
/// For `related_paths = ["author"]`:
///   - Adds `LEFT JOIN authors ON articles.author_id = authors.id`
///   - Adds `authors.id AS author__id`, `authors.name AS author__name`, …
///
/// Supports nested paths like `"author__profile"` (two-level JOIN).
pub fn apply_select_related(
    stmt: &mut SelectStatement,
    base_model: &ModelDef,
    related_paths: &[String],
    registry: &ModelRegistry,
) -> Result<()> {
    for path in related_paths {
        let parts: Vec<&str> = path.split("__").collect();
        let mut current_model = base_model;
        let mut prefix = String::new();

        for (i, field_name) in parts.iter().enumerate() {
            // Fields are keyed by attname (e.g. "author_id"), but callers pass
            // the Python field name (e.g. "author").  Try both.
            let field = current_model
                .fields
                .get(*field_name)
                .or_else(|| {
                    // Try attname convention: "{field_name}_id" for FKs.
                    current_model.fields.get(&format!("{}_id", field_name))
                })
                .or_else(|| {
                    // Full search by .name attribute.
                    current_model.fields.values().find(|f| f.name == *field_name)
                })
                .ok_or_else(|| OrmpiError::UnknownField {
                    keyword: field_name.to_string(),
                    choices: current_model
                        .fields
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", "),
                })?;

            let to_model_name = match &field.field_type {
                FieldType::ForeignKey { to, .. } | FieldType::OneToOne { to, .. } => to.clone(),
                _ => {
                    return Err(OrmpiError::FieldError {
                        model: current_model.model_name.clone(),
                        field: field_name.to_string(),
                        message: "select_related can only follow ForeignKey/OneToOneField"
                            .to_string(),
                    })
                }
            };

            // Resolve related model — try full name first, then by bare name.
            let related_model = registry
                .get_by_name(to_model_name.split('.').last().unwrap_or(&to_model_name))
                .ok_or_else(|| {
                    OrmpiError::Other(format!(
                        "Model '{}' not found in registry",
                        to_model_name
                    ))
                })?;

            // Build the column alias prefix for this level, e.g. "author" or "author__profile".
            if i == 0 {
                prefix = parts[0].to_string();
            } else {
                prefix = parts[..=i].join("__");
            }

            // SELECT related_table.col AS {prefix}__{col}
            for rel_field in related_model.fields.values() {
                // Skip M2M (no column)
                if matches!(rel_field.field_type, FieldType::ManyToMany { .. }) {
                    continue;
                }
                let aliased_col = format!("{}__{}", prefix, rel_field.attname);
                stmt.expr_as(
                    Expr::col((
                        Alias::new(&related_model.table_name),
                        Alias::new(&rel_field.column_name()),
                    )),
                    Alias::new(&aliased_col),
                );
            }

            // LEFT JOIN related_table ON current.fk_col = related.pk
            let fk_col = field.column_name();
            let pk_col = related_model.pk_field.clone();
            stmt.left_join(
                Alias::new(&related_model.table_name),
                Expr::col((
                    Alias::new(&current_model.table_name),
                    Alias::new(&fk_col),
                ))
                .equals((
                    Alias::new(&related_model.table_name),
                    Alias::new(&pk_col),
                )),
            );

            current_model = related_model;
        }
    }
    Ok(())
}

/// Find a reverse FK relation in the registry.
///
/// Searches all registered models for a ForeignKey / OneToOne that points to
/// `base_model_name` and whose `related_name` (or default reverse name) matches
/// `lookup_name`.
///
/// Returns `(related_model_def, fk_column_name, base_pk_field)`.
pub fn find_reverse_relation<'a>(
    registry: &'a ModelRegistry,
    base_model: &ModelDef,
    lookup_name: &str,
) -> Option<(&'a ModelDef, String, String)> {
    let base_name = base_model.model_name.as_str();
    let base_pk = base_model.pk_field.clone();

    for model_def in registry.all() {
        for field in model_def.fields.values() {
            let to = match &field.field_type {
                FieldType::ForeignKey { to, .. } | FieldType::OneToOne { to, .. } => to,
                _ => continue,
            };

            // Compare bare model names case-insensitively.
            let to_simple = to.split('.').last().unwrap_or(to);
            if !to_simple.eq_ignore_ascii_case(base_name) {
                continue;
            }

            // Check explicit related_name first.
            if let Some(rn) = &field.related_name {
                if rn == lookup_name {
                    return Some((model_def, field.column_name(), base_pk));
                }
            }

            // Fall back to the default reverse manager name: lowercase model name.
            let default_name = model_def.model_name.to_lowercase();
            let default_set = format!("{}_set", default_name);
            if lookup_name == default_name || lookup_name == default_set {
                return Some((model_def, field.column_name(), base_pk));
            }
        }
    }
    None
}
