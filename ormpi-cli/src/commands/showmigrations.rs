use std::collections::HashSet;
use std::path::Path;

use ormpi_core::backend::QueryBackend;
use ormpi_core::migration::{MigrationGraph, MigrationRecorder};
use ormpi_core::pool::OrmpiPool;

#[derive(clap::Args)]
pub struct ShowmigrationsArgs {
    /// App labels to filter (default: all)
    pub apps: Vec<String>,

    /// Show migrations in plan order
    #[arg(long, short = 'p')]
    pub plan: bool,
}

pub async fn run_showmigrations(
    _args: ShowmigrationsArgs,
    pool: OrmpiPool,
    migrations_dir: &Path,
) -> ormpi_core::Result<i32> {
    if !migrations_dir.exists() {
        println!("(no migrations)");
        return Ok(0);
    }

    let backend = QueryBackend(pool.backend());
    let graph = MigrationGraph::load_from_dir(migrations_dir)?;
    let recorder = MigrationRecorder { pool, backend };
    recorder.ensure_table().await?;

    let applied: HashSet<String> = recorder.applied().await?.into_iter().collect();
    let migrations = graph.topological_order();

    if migrations.is_empty() {
        println!("(no migrations)");
        return Ok(0);
    }

    for name in &migrations {
        let mark = if applied.contains(name) { "X" } else { " " };
        println!(" [{}] {}", mark, name);
    }

    Ok(0)
}
