//! AI feature management, backed by the shared [Ailloy](https://crates.io/crates/ailloy) config.
//!
//! - `rusty-tmpl ai`          show status
//! - `rusty-tmpl ai test`     test the AI connection
//! - `rusty-tmpl ai enable`   enable AI for rusty-tmpl
//! - `rusty-tmpl ai disable`  disable AI for rusty-tmpl
//! - `rusty-tmpl ai config`   interactive AI node configuration
//!
//! Ailloy stores a shared, global config (`~/.config/ailloy/config.yaml`) so
//! every MKLab tool reuses the same providers and API keys. To actually call a
//! model from your own commands, use `ailloy::Client` (see the ailloy docs).

use anyhow::Result;

use ailloy::config::Config;
use ailloy::config_tui;

use crate::cli::AiCommands;

/// The capabilities this tool needs from Ailloy. A bare CLI just needs `chat`;
/// add more (e.g. `"image"`) as your tool grows.
const CAPABILITIES: &[&str] = &["chat"];

pub async fn run(cmd: Option<AiCommands>) -> Result<()> {
    match cmd {
        None | Some(AiCommands::Status) => config_tui::print_ai_status("rusty-tmpl", CAPABILITIES),
        Some(AiCommands::Test { message }) => {
            config_tui::run_test_chat("rusty-tmpl", message).await
        }
        Some(AiCommands::Enable) => config_tui::enable_ai("rusty-tmpl"),
        Some(AiCommands::Disable) => config_tui::disable_ai("rusty-tmpl"),
        Some(AiCommands::Config) => {
            let mut config = Config::load_global()?;
            config_tui::run_interactive_config(&mut config, CAPABILITIES).await?;
            Ok(())
        }
    }
}

/// Whether AI features are active (configured via ailloy + enabled for this tool).
#[allow(dead_code)]
pub fn is_ai_active() -> bool {
    config_tui::is_ai_active("rusty-tmpl")
}
