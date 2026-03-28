use clap::Parser;
use ormpi_cli::{run_cli, Cli};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = run_cli(cli).await;
    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
