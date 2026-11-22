pub mod commands;
pub mod config;

use clap::Parser;

use commands::inspectdb::{run_inspectdb, InspectdbArgs};
use commands::makemigrations::{run_makemigrations, MakemigrationsArgs};
use commands::migrate::{run_migrate, MigrateArgs};
use commands::showmigrations::{run_showmigrations, ShowmigrationsArgs};
use commands::sqlmigrate::{run_sqlmigrate, SqlmigrateArgs};

#[derive(Parser)]
#[command(
    name = "piorm",
    about = "PiOrm database management commands",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct Cli {
    /// Database URL (e.g. sqlite://./db.sqlite3 or postgres://user:pass@host/db)
    #[arg(long, env = "DATABASE_URL", global = true)]
    pub database_url: Option<String>,

    /// Directory containing migration TOML files
    #[arg(long, env = "MIGRATIONS_DIR", default_value = "./migrations", global = true)]
    pub migrations_dir: String,

    /// Verbosity level (0 = quiet, 1 = normal, 2 = verbose)
    #[arg(long, short = 'v', default_value = "1", global = true)]
    pub verbosity: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Detect model changes and create migration files
    Makemigrations(MakemigrationsArgs),
    /// Apply or reverse database migrations
    Migrate(MigrateArgs),
    /// List all migrations and their applied status
    Showmigrations(ShowmigrationsArgs),
    /// Print SQL for a migration without running it
    Sqlmigrate(SqlmigrateArgs),
    /// Introspect database and output model definitions
    Inspectdb(InspectdbArgs),
    /// Print version information
    Version,
}

pub async fn run_cli(cli: Cli) -> piorm_core::Result<i32> {
    let migrations_dir = std::path::PathBuf::from(&cli.migrations_dir);

    match cli.command {
        Commands::Version => {
            println!("piorm {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Commands::Makemigrations(args) => {
            run_makemigrations(args, &migrations_dir, cli.verbosity).await
        }
        Commands::Migrate(args) => {
            let pool = init_pool(cli.database_url.as_deref()).await?;
            run_migrate(args, pool, &migrations_dir, cli.verbosity).await
        }
        Commands::Showmigrations(args) => {
            let pool = init_pool(cli.database_url.as_deref()).await?;
            run_showmigrations(args, pool, &migrations_dir).await
        }
        Commands::Sqlmigrate(args) => {
            let pool = init_pool(cli.database_url.as_deref()).await?;
            run_sqlmigrate(args, pool, &migrations_dir).await
        }
        Commands::Inspectdb(args) => {
            let pool = init_pool(cli.database_url.as_deref()).await?;
            run_inspectdb(args, pool, &migrations_dir).await
        }
    }
}

async fn init_pool(database_url: Option<&str>) -> piorm_core::Result<piorm_core::pool::PiormPool> {
    let url = database_url.ok_or_else(|| {
        piorm_core::PiormError::ConfigError(
            "No database URL specified. Use --database-url or DATABASE_URL env var.".into(),
        )
    })?;
    let db_config = config::parse_database_url("default", url)?;
    piorm_core::pool::PiormPool::connect(&db_config).await
}
