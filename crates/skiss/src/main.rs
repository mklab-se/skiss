//! skiss: an infinite canvas for your notes, stored as Markdown.

use anyhow::Result;
use clap::{CommandFactory, Parser};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

mod banner;
mod canvas;
mod cli;
mod commands;
mod update;

use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Handle dynamic shell completions (when invoked via COMPLETE=<shell> skiss)
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();

    // Initialize logging based on -v / -q.
    let filter = if cli.verbose > 0 {
        match cli.verbose {
            1 => "skiss=debug",
            _ => "skiss=trace",
        }
    } else if cli.quiet {
        "error"
    } else {
        "skiss=info"
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).without_time())
        .with(EnvFilter::new(filter))
        .init();

    if cli.no_color {
        colored::control::set_override(false);
    }

    // Spawn a background update check (skipped in quiet mode or if disabled via env).
    let update_handle = if !cli.quiet && std::env::var("SKISS_NO_UPDATE_CHECK").is_err() {
        Some(tokio::spawn(update::check_for_updates()))
    } else {
        None
    };

    let result = cli.run().await;

    if let Some(handle) = update_handle {
        let _ = handle.await;
    }

    result
}
