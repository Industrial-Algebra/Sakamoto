//! Sakamoto CLI — pipeline-oriented coding agent orchestrator.
//!
//! Usage:
//!   sakamoto run "fix clippy warnings"
//!   sakamoto init
//!   sakamoto check

mod commands;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

/// Sakamoto — pipeline-oriented coding agent orchestrator.
#[derive(Parser)]
#[command(name = "sakamoto", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a pipeline with the given task description.
    Run {
        /// The task to execute (e.g., "fix clippy warnings").
        task: String,

        /// Pipeline name to execute (defaults to "default").
        #[arg(short, long, default_value = "default")]
        pipeline: String,
    },

    /// Generate a sakamoto.toml in the current directory.
    Init,

    /// Validate configuration and report status.
    Check,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run { task, pipeline } => commands::run::execute(&task, &pipeline).await?,
        Commands::Init => commands::init::execute()?,
        Commands::Check => commands::check::execute()?,
    }

    Ok(())
}
