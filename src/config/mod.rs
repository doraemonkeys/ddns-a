//! Configuration layer for DDNS-A.
//!
//! This module provides:
//! - CLI argument parsing ([`Cli`], [`Command`])
//! - TOML configuration file parsing ([`TomlConfig`])
//! - Validated configuration ([`ValidatedConfig`])
//! - Configuration file generation ([`write_default_config`])
//! - Default values ([`defaults`])
//!
//! # Priority
//!
//! Configuration values are resolved with the following priority (highest to lowest):
//!
//! 1. **Explicit CLI arguments** - Values explicitly passed via command line
//! 2. **TOML config file** - Values from the configuration file
//! 3. **Built-in defaults** - Hardcoded default values
//!
//! For required fields without defaults (`url`, `ip_version`), CLI takes precedence over TOML.
//!
//! For optional fields with defaults (`method`, `poll_interval`, retry settings),
//! explicit CLI values always win, then TOML, then built-in defaults.
//!
//! For filter patterns (`include_adapters`, `exclude_adapters`), CLI patterns **replace**
//! TOML patterns entirely (not merged). This is intentional "replace" semantics.
//! Note: include and exclude patterns are handled independently - if CLI has `--include-adapter`,
//! only TOML includes are replaced; TOML excludes are still used (unless CLI excludes are specified).
//!
//! # Boolean Flag Semantics
//!
//! Boolean flags (`--poll-only`, `--exclude-virtual`) use OR semantics:
//! - If set `true` in either CLI or TOML, the result is `true`.
//! - Once set `true` in TOML, CLI cannot override to `false` (flags only enable, not disable).
//! - This differs from other options where "CLI explicit > TOML".
//!
//! # CLI-Only vs TOML-Only Options
//!
//! Some retry policy options are TOML-only (not available via CLI):
//! - `retry.max_delay` (default: 60s) - Maximum retry delay
//! - `retry.multiplier` (default: 2.0) - Exponential backoff multiplier
//!
//! For full configurability, use a config file.
//!
//! # Internal Tuning Parameters
//!
//! The following parameters are intentionally not user-configurable:
//! - **Debounce window**: The monitor module uses a fixed 2-second debounce window
//!   to merge rapid IP change events. This is tuned for typical OS notification patterns
//!   and is not exposed via CLI or TOML configuration.

mod cli;
pub mod defaults;
mod error;
mod toml;
mod validated;

#[cfg(test)]
mod cli_tests;
#[cfg(test)]
mod toml_tests;
#[cfg(test)]
mod validated_tests;

pub use cli::{Cli, Command, IpVersionArg};
pub use error::ConfigError;
pub use toml::{TomlConfig, default_config_template};
pub use validated::{ValidatedConfig, write_default_config};
