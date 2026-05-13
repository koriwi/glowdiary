mod db;
mod error;
mod off;
mod tools;

use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use tools::GlowDiary;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "glowdiary", version, about = "Food Diary MCP Server")]
struct Cli {
    /// Path to the SQLite database file.
    #[arg(long, default_value = "./glowdiary.db")]
    db_path: String,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialise logging (stderr, so MCP stdio on stdout stays clean)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(cli.log_level.parse()?),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!(
        "Starting GlowDiary MCP server (db: {})",
        cli.db_path
    );

    // Open database (creates if missing, runs migrations)
    let conn = db::open(&cli.db_path)?;
    tracing::info!("Database ready");

    // Build and serve the MCP server over stdio
    let service = GlowDiary::new(conn)
        .serve(stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("Failed to serve: {e:?}");
        })?;

    tracing::info!("MCP server initialised, waiting for requests");

    // Wait for shutdown (client closes stdin, or SIGTERM)
    service.waiting().await?;

    tracing::info!("Shutting down");
    Ok(())
}
