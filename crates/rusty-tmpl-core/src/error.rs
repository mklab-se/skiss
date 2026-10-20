//! Error types for `rusty-tmpl-core`.
//!
//! A small `thiserror`-based enum to start from. Add variants as your tool
//! grows; the CLI converts these into `anyhow::Error` automatically via the
//! `?` operator.

use thiserror::Error;

/// Convenient result alias used throughout the core crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced by the core crate.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O operation failed (reading/writing config, etc.).
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to (de)serialize configuration.
    #[error("config error: {0}")]
    Config(String),

    /// The platform did not expose a config directory.
    #[error("could not determine the configuration directory")]
    NoConfigDir,
}

impl From<serde_norway::Error> for Error {
    fn from(e: serde_norway::Error) -> Self {
        Error::Config(e.to_string())
    }
}
