//! Core types and configuration for `skiss`.
//!
//! This crate holds the framework-agnostic pieces of the tool (configuration
//! and error types) so they can be reused by the CLI binary, tests, or any
//! future crate (e.g. a `-client`) without pulling in `clap`, `tokio`, or other
//! CLI-only dependencies.
//!
//! When you build a real tool from this template, put your domain logic here.

pub mod config;
pub mod error;
pub mod paths;

pub use error::{Error, Result};
