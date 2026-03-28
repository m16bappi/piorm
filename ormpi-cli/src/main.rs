use clap::{Parser, Subcommand};

mod commands;
mod config;

use commands::makemigrations::{run_makemigrations, MakemigrationsArgs};
use commands::migrate::{run_migrate, MigrateArgs};
use commands::showmigrations::{run_showmigrations, ShowmigrationsArgs};

#[derive(Parser)]
#[command(
    name = "ormpi",
    about = "OrmPi database management commands",
    version = env!("CARGO_PKG_VERSION")
)]
struct Cli {
    /// Database URL (e.g. sqlite://./db.sqlite3 or postgres://user:pass@host/db)
    #[arg(long, env = "DATABASE_URL", global = true)]
    database_url: Option<String>,

    /// Directory containing migration TOML files
    #[arg(long, env = "MIGRATIONS_DIR", default_value = "./migrations", global = true)]
    migrations_dir: String,

    /// Verbosity level (0 = quiet, 1 = normal, 2 = verbose)
    #[arg(long, short = 'v', default_value = "1", global = true)]
    verbosity: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Detect model changes and create migration files
    Makemigrations(MakemigrationsArgs),
    /// Apply or reverse database migrations
    Migrate(MigrateArgs),
    /// List all migrations and their applied status
    Showmigrations(ShowmigrationsArgs),
    /// Print version information
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = run(cli).await;
    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn run(cli: Cli) -> ormpi_core::Result<i32> {
    let migrations_dir = std::path::PathBuf::from(&cli.migrations_dir);

    match cli.command {
        Commands::Version => {
            println!("ormpi {}", env!("CARGO_PKG_VERSION"));
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
    }
}

async fn init_pool(database_url: Option<&str>) -> ormpi_core::Result<ormpi_core::pool::OrmpiPool> {
    let url = database_url.ok_or_else(|| {
        ormpi_core::OrmpiError::ConfigError(
            "No database URL specified. Use --database-url or DATABASE_URL env var.".into(),
        )
    })?;
    let db_config = config::parse_database_url("default", url)?;
    ormpi_core::pool::OrmpiPool::connect(&db_config).await
}
