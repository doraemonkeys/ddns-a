//! Tests for validated configuration.

use http::Method;

use super::ConfigError;
use super::cli::Cli;
use super::toml::TomlConfig;
use super::validated::ValidatedConfig;

/// Helper to create CLI args from a slice
fn cli(args: &[&str]) -> Cli {
    let mut full_args = vec!["ddns-a"];
    full_args.extend(args);
    Cli::parse_from_iter(full_args)
}

/// Helper to parse TOML config
fn toml(content: &str) -> TomlConfig {
    TomlConfig::parse(content).unwrap()
}

mod filter_tests;
mod loading_tests;
mod precedence_tests;
mod runtime_tests;
mod webhook_tests;
