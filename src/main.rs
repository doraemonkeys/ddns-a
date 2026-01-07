//! DDNS-A: Dynamic DNS Address Monitor
//!
//! Entry point for the ddns-a application.

use ddns_a::config::{Cli, Command, ValidatedConfig, write_default_config};
use std::process::ExitCode;

mod app;
mod run;

use app::{exit_code, print_config_hint, setup_tracing};

/// Main entry point.
///
/// Excluded from coverage as it's the thin wrapper around testable components.
#[cfg(not(tarpaulin_include))]
fn main() -> ExitCode {
    let cli = Cli::parse_args();

    // Handle init subcommand
    if let Some(Command::Init { output }) = &cli.command {
        return handle_init(output);
    }

    // Load and validate configuration
    let config = match ValidatedConfig::load(&cli) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            print_config_hint(&e);
            return exit_code::CONFIG_ERROR;
        }
    };

    // Setup logging and run
    setup_tracing(config.verbose);
    tracing::info!("{config}");

    run_application(config)
}

/// Handles the `init` subcommand.
fn handle_init(output: &std::path::Path) -> ExitCode {
    match write_default_config(output) {
        Ok(()) => {
            println!("Configuration template written to: {}", output.display());
            exit_code::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            exit_code::CONFIG_ERROR
        }
    }
}

/// Runs the main application with the given configuration.
///
/// Excluded from coverage - requires async runtime.
#[cfg(not(tarpaulin_include))]
fn run_application(config: ValidatedConfig) -> ExitCode {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    match runtime.block_on(run::execute(config)) {
        Ok(()) => exit_code::SUCCESS,
        Err(e) => {
            tracing::error!("Application error: {e}");
            exit_code::runtime_error()
        }
    }
}
