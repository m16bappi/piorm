use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use indexmap::IndexMap;

use super::file::MigrationFile;

// ---------------------------------------------------------------------------
// MigrationGraph
// ---------------------------------------------------------------------------

/// A directed acyclic graph of migration files, keyed by migration name.
pub struct MigrationGraph {
    /// Ordered map: name → file, preserving insertion (load) order.
    migrations: IndexMap<String, MigrationFile>,
}

impl MigrationGraph {
    /// Load all `.toml` migration files from `dir` and build the graph.
    pub fn load_from_dir(dir: &Path) -> crate::Result<Self> {
        let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .map_err(|e| crate::PiormError::MigrationError(format!("Cannot read dir: {e}")))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();

        entries.sort(); // lexicographic order

        let mut migrations: IndexMap<String, MigrationFile> = IndexMap::new();
        for path in &entries {
            let file = MigrationFile::read_from_file(path)?;
            migrations.insert(file.migration.name.clone(), file);
        }

        Ok(MigrationGraph { migrations })
    }

    /// Build a `MigrationGraph` directly from an in-memory map (for tests).
    pub fn from_files(files: Vec<MigrationFile>) -> Self {
        let mut migrations = IndexMap::new();
        for f in files {
            migrations.insert(f.migration.name.clone(), f);
        }
        MigrationGraph { migrations }
    }

    /// Return migration names in topological order (dependencies before dependents).
    /// Among independent migrations at the same level, sort by name for determinism.
    pub fn topological_order(&self) -> Vec<String> {
        // Kahn's algorithm
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new(); // dep → migrations that depend on it

        for (name, file) in &self.migrations {
            in_degree.entry(name.as_str()).or_insert(0);
            for dep in &file.migration.dependencies {
                // Only track deps that are in our graph
                if self.migrations.contains_key(dep.as_str()) {
                    *in_degree.entry(name.as_str()).or_insert(0) += 1;
                    dependents.entry(dep.as_str()).or_default().push(name.as_str());
                }
            }
        }

        // Start with zero-in-degree nodes, sorted by name
        let mut queue: VecDeque<&str> = {
            let mut v: Vec<&str> = in_degree
                .iter()
                .filter(|(_, &deg)| deg == 0)
                .map(|(&name, _)| name)
                .collect();
            v.sort();
            VecDeque::from(v)
        };

        let mut result: Vec<String> = Vec::new();

        while let Some(current) = queue.pop_front() {
            result.push(current.to_string());

            if let Some(deps) = dependents.get(current) {
                let mut next_batch: Vec<&str> = Vec::new();
                for &dep_name in deps {
                    let deg = in_degree.get_mut(dep_name).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next_batch.push(dep_name);
                    }
                }
                next_batch.sort();
                for name in next_batch {
                    queue.push_back(name);
                }
            }
        }

        result
    }

    /// Get a migration by name.
    pub fn get(&self, name: &str) -> Option<&MigrationFile> {
        self.migrations.get(name)
    }

    /// Return migrations NOT in `applied`, up to and including `target`
    /// (or all unapplied if target is None).
    pub fn migrations_to_apply(&self, target: Option<&str>, applied: &[String]) -> Vec<String> {
        let applied_set: HashSet<&str> = applied.iter().map(|s| s.as_str()).collect();
        let order = self.topological_order();

        let mut result: Vec<String> = Vec::new();
        for name in &order {
            if !applied_set.contains(name.as_str()) {
                result.push(name.clone());
            }
            if let Some(t) = target {
                if name == t {
                    break;
                }
            }
        }
        result
    }

    /// Return applied migrations that come after `target` (in reverse order for unapplying).
    /// `"zero"` means unapply all applied migrations.
    pub fn migrations_to_unapply(&self, target: &str, applied: &[String]) -> Vec<String> {
        let applied_set: HashSet<&str> = applied.iter().map(|s| s.as_str()).collect();
        let order = self.topological_order();

        if target == "zero" {
            // All applied, reversed
            let mut result: Vec<String> = order
                .into_iter()
                .filter(|n| applied_set.contains(n.as_str()))
                .collect();
            result.reverse();
            return result;
        }

        // Find target position; unapply everything applied that comes after it
        let target_pos = order.iter().position(|n| n == target);

        let mut result: Vec<String> = order
            .into_iter()
            .enumerate()
            .filter(|(i, n)| {
                applied_set.contains(n.as_str()) && target_pos.map_or(true, |tp| *i > tp)
            })
            .map(|(_, n)| n)
            .collect();
        result.reverse();
        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::file::{MigrationFile, MigrationMeta};

    fn make_file(name: &str, deps: &[&str]) -> MigrationFile {
        MigrationFile {
            migration: MigrationMeta {
                name: name.to_string(),
                dependencies: deps.iter().map(|s| s.to_string()).collect(),
                replaces: vec![],
            },
            operations: vec![],
        }
    }

    fn make_graph(files: &[(&str, &[&str])]) -> MigrationGraph {
        let v = files.iter().map(|(name, deps)| make_file(name, deps)).collect();
        MigrationGraph::from_files(v)
    }

    #[test]
    fn test_topological_order_linear() {
        let g = make_graph(&[
            ("0001_initial", &[]),
            ("0002_add_field", &["0001_initial"]),
            ("0003_alter_field", &["0002_add_field"]),
        ]);
        let order = g.topological_order();
        assert_eq!(order, vec!["0001_initial", "0002_add_field", "0003_alter_field"]);
    }

    #[test]
    fn test_topological_order_parallel_sorted() {
        // 0001 and 0002 are independent; both depend on nothing; should be sorted by name
        let g = make_graph(&[
            ("0002_b", &[]),
            ("0001_a", &[]),
            ("0003_c", &["0001_a", "0002_b"]),
        ]);
        let order = g.topological_order();
        // 0001_a and 0002_b come before 0003_c; their relative order should be sorted
        assert_eq!(order[0], "0001_a");
        assert_eq!(order[1], "0002_b");
        assert_eq!(order[2], "0003_c");
    }

    #[test]
    fn test_migrations_to_apply_none_applied() {
        let g = make_graph(&[
            ("0001_initial", &[]),
            ("0002_add_field", &["0001_initial"]),
        ]);
        let to_apply = g.migrations_to_apply(None, &[]);
        assert_eq!(to_apply, vec!["0001_initial", "0002_add_field"]);
    }

    #[test]
    fn test_migrations_to_apply_some_applied() {
        let g = make_graph(&[
            ("0001_initial", &[]),
            ("0002_add_field", &["0001_initial"]),
            ("0003_alter_field", &["0002_add_field"]),
        ]);
        let applied = vec!["0001_initial".to_string()];
        let to_apply = g.migrations_to_apply(None, &applied);
        assert_eq!(to_apply, vec!["0002_add_field", "0003_alter_field"]);
    }

    #[test]
    fn test_migrations_to_apply_with_target() {
        let g = make_graph(&[
            ("0001_initial", &[]),
            ("0002_add_field", &["0001_initial"]),
            ("0003_alter_field", &["0002_add_field"]),
        ]);
        let to_apply = g.migrations_to_apply(Some("0002_add_field"), &[]);
        assert_eq!(to_apply, vec!["0001_initial", "0002_add_field"]);
    }

    #[test]
    fn test_migrations_to_unapply_zero() {
        let g = make_graph(&[
            ("0001_initial", &[]),
            ("0002_add_field", &["0001_initial"]),
        ]);
        let applied = vec!["0001_initial".to_string(), "0002_add_field".to_string()];
        let to_unapply = g.migrations_to_unapply("zero", &applied);
        // Reversed: latest first
        assert_eq!(to_unapply, vec!["0002_add_field", "0001_initial"]);
    }

    #[test]
    fn test_migrations_to_unapply_partial() {
        let g = make_graph(&[
            ("0001_initial", &[]),
            ("0002_add_field", &["0001_initial"]),
            ("0003_alter_field", &["0002_add_field"]),
        ]);
        let applied = vec![
            "0001_initial".to_string(),
            "0002_add_field".to_string(),
            "0003_alter_field".to_string(),
        ];
        let to_unapply = g.migrations_to_unapply("0001_initial", &applied);
        // Should unapply 0003 and 0002 (in reverse), not 0001
        assert_eq!(to_unapply, vec!["0003_alter_field", "0002_add_field"]);
    }

    #[test]
    fn test_load_from_dir() {
        let dir = tempfile::tempdir().unwrap();

        let f1 = make_file("0001_initial", &[]);
        let f2 = make_file("0002_second", &["0001_initial"]);

        f1.write_to_file(&dir.path().join("0001_initial.toml")).unwrap();
        f2.write_to_file(&dir.path().join("0002_second.toml")).unwrap();

        let g = MigrationGraph::load_from_dir(dir.path()).unwrap();
        assert!(g.get("0001_initial").is_some());
        assert!(g.get("0002_second").is_some());

        let order = g.topological_order();
        assert_eq!(order[0], "0001_initial");
        assert_eq!(order[1], "0002_second");
    }
}
