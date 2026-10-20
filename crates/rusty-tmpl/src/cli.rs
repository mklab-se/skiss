//! CLI argument definitions using clap.

use anyhow::Result;
use clap::Parser;

/// A template for building Rust command-line tools.
#[derive(Parser)]
#[command(name = "rusty-tmpl")]
#[command(author, version, about)]
#[command(long_about = "A template for building Rust command-line tools.\n\n\
    Run without a subcommand to print \"Hello world!\". The reusable plumbing \
    (AI integration, shell completions, and versioning) is wired up so you can \
    focus on your tool's own commands.")]
#[command(propagate_version = true)]
pub struct Cli {
    /// Increase output verbosity (-v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Manage AI features (shows status when run without a subcommand)
    Ai {
        #[command(subcommand)]
        command: Option<AiCommands>,
    },

    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Show version information
    Version,
}

#[derive(clap::Subcommand)]
pub enum AiCommands {
    /// Test AI integration by sending a message
    Test {
        /// Message to send (default: "Say hello in one sentence.")
        message: Option<String>,
    },
    /// Enable AI features for rusty-tmpl
    Enable,
    /// Disable AI features for rusty-tmpl
    Disable,
    /// Interactively configure AI provider and model settings
    Config,
    /// Show AI status (same as running `rusty-tmpl ai` without a subcommand)
    Status,
}

/// Shells supported by `rusty-tmpl completion`.
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

impl Cli {
    /// Dispatch to the selected subcommand.
    pub async fn run(self) -> Result<()> {
        match self.command {
            Some(Commands::Ai { command }) => crate::commands::ai::run(command).await,
            Some(Commands::Completion { shell }) => {
                crate::commands::completion::generate_completions(shell);
                Ok(())
            }
            Some(Commands::Version) => {
                crate::banner::print_banner_with_version();
                Ok(())
            }
            // No subcommand: the almost-empty default. Replace this with your
            // tool's behavior, or route to a real command. `--help` still works.
            None => {
                println!("Hello world!");
                Ok(())
            }
        }
    }
}
