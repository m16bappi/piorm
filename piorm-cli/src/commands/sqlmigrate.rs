use std::path::Path;

use piorm_core::backend::QueryBackend;
use piorm_core::migration::MigrationGraph;
use piorm_core::pool::PiormPool;

#[derive(clap::Args)]
pub struct SqlmigrateArgs {
    /// Migration name (e.g. 0001_initial)
    pub migration_name: String,

    /// Print backwards (rollback) SQL instead of forwards
    #[arg(long)]
    pub backwards: bool,
}

pub async fn run_sqlmigrate(
    args: SqlmigrateArgs,
    pool: PiormPool,
    migrations_dir: &Path,
) -> piorm_core::Result<i32> {
    if !migrations_dir.exists() {
        eprintln!("No migrations directory found at {}", migrations_dir.display());
        return Ok(1);
    }

    let backend = QueryBackend(pool.backend());
    let graph = MigrationGraph::load_from_dir(migrations_dir)?;

    let migration = graph.get(&args.migration_name).ok_or_else(|| {
        piorm_core::PiormError::MigrationError(format!(
            "Migration '{}' not found",
            args.migration_name
        ))
    })?;

    let ops = migration.to_operations()?;
    let direction = if args.backwards { "BACKWARD" } else { "FORWARD" };

    println!("BEGIN;");
    println!();

    for op in &ops {
        let sqls = if args.backwards {
            op.database_backwards(&backend)
        } else {
            op.database_forwards(&backend)
        };
        println!("-- {} operation: {}", direction, op.describe());
        for sql in sqls {
            println!("{};", sql);
        }
        println!();
    }

    println!("COMMIT;");

    Ok(0)
}
