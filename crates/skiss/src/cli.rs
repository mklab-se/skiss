//! CLI argument definitions using clap.

use anyhow::Result;
use clap::Parser;

/// An infinite canvas for your notes, stored as Markdown.
#[derive(Parser)]
#[command(name = "skiss")]
#[command(author, version, about)]
#[command(
    long_about = "An infinite canvas for your notes, stored as Markdown.\n\n\
    Open any Markdown file as a board: headings become areas and cards, \
    \"- Manages [Erik](#erik)\" becomes an arrow, and every change is written \
    straight back to the file as plain Markdown. Run without a file to pick a name; \
    a file that does not exist yet is created."
)]
#[command(propagate_version = true)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// The Markdown file to open (created if it does not exist)
    #[arg(value_name = "FILE")]
    pub file: Option<std::path::PathBuf>,

    /// Theme for this window: sketchbook or ember (remembered from ⌘T otherwise)
    #[arg(long, value_name = "THEME")]
    pub theme: Option<String>,

    /// Stay attached to the terminal until the window closes (default: return to the prompt)
    #[arg(long)]
    pub wait: bool,

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
    /// Enable AI features for skiss
    Enable,
    /// Disable AI features for skiss
    Disable,
    /// Interactively configure AI provider and model settings
    Config,
    /// Show AI status (same as running `skiss ai` without a subcommand)
    Status,
}

/// Shells supported by `skiss completion`.
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
            // No subcommand: open the canvas (with the start screen if no file was given).
            None => {
                crate::canvas::check(self.file.as_deref(), self.theme.as_deref())?;
                // Give the prompt back: the window runs as its own process, like any app.
                let attached = self.wait
                    || std::env::var_os("SKISS_DETACHED").is_some()
                    || std::env::var_os("SKISS_SCRIPT").is_some()
                    || std::env::var_os("SKISS_SCREENSHOT").is_some();
                if !attached {
                    return crate::canvas::spawn_detached();
                }
                tokio::task::block_in_place(|| crate::canvas::run(self.file, self.theme))
            }
        }
    }
}
