use std::path::Path;

use piorm_core::migration::{
    generate_migration_name, AutoDetector, MigrationFile, MigrationMeta, ProjectState,
};

#[derive(clap::Args)]
pub struct MakemigrationsArgs {
    /// App labels to detect changes for (default: all)
    pub apps: Vec<String>,

    /// Override the generated migration name
    #[arg(long)]
    pub name: Option<String>,

    /// Create an empty migration file
    #[arg(long)]
    pub empty: bool,

    /// Show what would be created without writing files
    #[arg(long)]
    pub dry_run: bool,

    /// Exit with non-zero if unapplied changes are detected (for CI)
    #[arg(long)]
    pub check: bool,
}

pub async fn run_makemigrations(
    args: MakemigrationsArgs,
    migrations_dir: &Path,
    verbosity: u8,
) -> piorm_core::Result<i32> {
    // Load schema state from migration history on disk
    let from_state = if migrations_dir.exists() {
        ProjectState::from_migrations(migrations_dir)?
    } else {
        ProjectState::default()
    };

    // Load current code state from the Rust model registry
    let to_state = ProjectState::from_registry();

    // Detect schema changes
    let detector = AutoDetector::new(from_state, to_state);
    let ops = detector.detect_changes();

    if ops.is_empty() && !args.empty {
        if verbosity >= 1 {
            println!("No changes detected.");
        }
        return Ok(0);
    }

    if args.check {
        if ops.is_empty() {
            return Ok(0);
        }
        eprintln!("Model changes detected but not yet migrated.");
        return Ok(1);
    }

    if args.dry_run {
        if ops.is_empty() {
            println!("Would create empty migration.");
        } else {
            println!("Would create migration with {} operation(s):", ops.len());
            for op in &ops {
                println!("  - {}", op.describe());
            }
        }
        return Ok(0);
    }

    // Ensure the migrations directory exists
    if !migrations_dir.exists() {
        std::fs::create_dir_all(migrations_dir)
            .map_err(|e| piorm_core::PiormError::Other(e.to_string()))?;
    }

    // List existing migrations to generate the next name and find dependencies
    let existing = list_migration_names(migrations_dir)?;
    let migration_name = args
        .name
        .unwrap_or_else(|| generate_migration_name(&existing, &ops));

    // Use the last existing migration as a dependency
    let dependencies: Vec<String> = existing.last().cloned().into_iter().collect();

    // Convert runtime ops to storable representation
    let stored_ops = ops.iter().map(|op| op.to_stored()).collect();

    let migration_file = MigrationFile {
        migration: MigrationMeta {
            name: migration_name.clone(),
            dependencies,
            replaces: vec![],
        },
        operations: stored_ops,
    };

    let file_path = migrations_dir.join(format!("{}.toml", migration_name));
    migration_file.write_to_file(&file_path)?;

    if verbosity >= 1 {
        println!("Migrations:");
        println!("  {}", file_path.display());
        for op in &ops {
            println!("    - {}", op.describe());
        }
    }

    Ok(0)
}

fn list_migration_names(dir: &Path) -> piorm_core::Result<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| piorm_core::PiormError::Other(e.to_string()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) == Some("toml") {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
            } else {
                None
            }
        })
        .collect();
    names.sort();
    Ok(names)
}
