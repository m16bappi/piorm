use std::path::Path;

use piorm_core::backend::QueryBackend;
use piorm_core::migration::{MigrationExecutor, MigrationGraph, MigrationRecorder};
use piorm_core::pool::PiormPool;

#[derive(clap::Args)]
pub struct MigrateArgs {
    /// App label (optional, for filtering)
    pub app_label: Option<String>,
    /// Target migration name, or "zero" for full rollback
    pub migration_name: Option<String>,

    /// Mark migrations as applied without running SQL
    #[arg(long)]
    pub fake: bool,

    /// Show plan without executing
    #[arg(long)]
    pub plan: bool,
}

pub async fn run_migrate(
    args: MigrateArgs,
    pool: PiormPool,
    migrations_dir: &Path,
    _verbosity: u8,
) -> piorm_core::Result<i32> {
    if !migrations_dir.exists() {
        println!("No migrations directory found at {}", migrations_dir.display());
        return Ok(0);
    }

    let backend = QueryBackend(pool.backend());
    let graph = MigrationGraph::load_from_dir(migrations_dir)?;
    let recorder = MigrationRecorder {
        pool: pool.clone(),
        backend,
    };
    recorder.ensure_table().await?;

    if args.plan {
        let applied = recorder.applied().await?;
        let pending = graph.migrations_to_apply(None, &applied);
        if pending.is_empty() {
            println!("(no pending migrations)");
        } else {
            for name in pending {
                println!("  [ ] {}", name);
            }
        }
        return Ok(0);
    }

    let executor = MigrationExecutor {
        recorder,
        graph,
        pool,
        backend,
    };

    let target = args.migration_name.as_deref();

    if target == Some("zero") {
        executor.rollback_to("zero", args.fake).await?;
    } else {
        executor.migrate(target, args.fake).await?;
    }

    Ok(0)
}
