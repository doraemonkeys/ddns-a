//! Application startup and utilities.
//!
//! This module contains exit codes, tracing setup, and error hints
//! that support the main entry point.

use ddns_a::config::{ConfigError, field};
use tracing::Level;
use tracing_subscriber::EnvFilter;

/// Application exit codes.
pub mod exit_code {
    use std::process::ExitCode;

    /// Success (exit code 0).
    pub const SUCCESS: ExitCode = ExitCode::SUCCESS;

    /// Configuration error (exit code 1) - invalid args, missing required fields, etc.
    pub const CONFIG_ERROR: ExitCode = ExitCode::FAILURE;

    /// Runtime error (exit code 2) - network failure, API error, etc.
    ///
    /// Note: This is a function rather than a constant because `ExitCode::from()` is not `const fn`.
    pub fn runtime_error() -> ExitCode {
        ExitCode::from(2)
    }
}

/// Prints helpful hints for common configuration errors.
pub fn print_config_hint(error: &ConfigError) {
    match error {
        ConfigError::MissingRequired { field: f, .. } => {
            if *f == field::URL || *f == field::IP_VERSION {
                eprintln!("\nRun 'ddns-a init' to generate a configuration template.");
            }
        }
        ConfigError::FileRead { .. } => {
            eprintln!("\nRun 'ddns-a init' to generate a configuration template.");
        }
        _ => {}
    }
}

/// Sets up the tracing subscriber for logging.
pub fn setup_tracing(verbose: bool) {
    let level = if verbose { Level::DEBUG } else { Level::INFO };

    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}
