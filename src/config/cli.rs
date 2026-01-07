//! CLI argument parsing using clap.
//!
//! Defines the command-line interface with all options and subcommands.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// DDNS-A: Dynamic DNS Address Monitor
///
/// Monitors IP address changes on network adapters and notifies
/// external services via webhooks.
#[derive(Debug, Parser)]
#[command(name = "ddns-a")]
#[command(version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)] // CLI flags are naturally boolean
pub struct Cli {
    /// Subcommand to run
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Webhook URL (required for run mode)
    #[arg(long, global = true)]
    pub url: Option<String>,

    /// IP version to monitor (required for run mode)
    #[arg(long = "ip-version", value_enum, global = true)]
    pub ip_version: Option<IpVersionArg>,

    /// HTTP method for webhook requests
    #[arg(long)]
    pub method: Option<String>,

    /// HTTP headers in 'Key=Value' or 'Key: Value' format (can be specified multiple times)
    #[arg(long = "header", value_name = "K=V")]
    pub headers: Vec<String>,

    /// Bearer token for Authorization header
    #[arg(long)]
    pub bearer: Option<String>,

    /// Handlebars body template for webhook requests
    #[arg(long = "body-template")]
    pub body_template: Option<String>,

    /// Regex pattern for adapters to include (can be specified multiple times)
    #[arg(long = "include-adapter", value_name = "PATTERN")]
    pub include_adapters: Vec<String>,

    /// Regex pattern for adapters to exclude (can be specified multiple times)
    #[arg(long = "exclude-adapter", value_name = "PATTERN")]
    pub exclude_adapters: Vec<String>,

    /// Exclude virtual adapters (`VMware`, `VirtualBox`, `Hyper-V`, etc.)
    #[arg(long = "exclude-virtual")]
    pub exclude_virtual: bool,

    /// Polling interval in seconds
    #[arg(long = "poll-interval")]
    pub poll_interval: Option<u64>,

    /// Disable API event listening, use polling only
    #[arg(long = "poll-only")]
    pub poll_only: bool,

    /// Maximum number of retry attempts
    #[arg(long = "retry-max")]
    pub retry_max: Option<u32>,

    /// Initial retry delay in seconds
    #[arg(long = "retry-delay")]
    pub retry_delay: Option<u64>,

    /// Path to configuration file
    #[arg(long, short)]
    pub config: Option<PathBuf>,

    /// Path to state file for detecting changes across restarts
    #[arg(long = "state-file")]
    pub state_file: Option<PathBuf>,

    /// Test mode - log changes without sending webhooks
    #[arg(long)]
    pub dry_run: bool,

    /// Enable verbose logging
    #[arg(long, short)]
    pub verbose: bool,
}

/// Subcommands for ddns-a
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate a default configuration file
    Init {
        /// Output path for the configuration file
        #[arg(long, short, default_value = "ddns-a.toml")]
        output: PathBuf,
    },
}

/// IP version argument for CLI parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum IpVersionArg {
    /// Monitor IPv4 addresses only
    #[value(name = "ipv4")]
    V4,
    /// Monitor IPv6 addresses only
    #[value(name = "ipv6")]
    V6,
    /// Monitor both IPv4 and IPv6 addresses
    #[value(name = "both")]
    Both,
}

impl From<IpVersionArg> for crate::network::IpVersion {
    fn from(arg: IpVersionArg) -> Self {
        match arg {
            IpVersionArg::V4 => Self::V4,
            IpVersionArg::V6 => Self::V6,
            IpVersionArg::Both => Self::Both,
        }
    }
}

impl Cli {
    /// Parses CLI arguments from the command line.
    #[must_use]
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Parses CLI arguments from an iterator (useful for testing).
    pub fn parse_from_iter<I, T>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::parse_from(iter)
    }

    /// Returns true if this is the init command.
    #[must_use]
    pub const fn is_init(&self) -> bool {
        matches!(self.command, Some(Command::Init { .. }))
    }
}
