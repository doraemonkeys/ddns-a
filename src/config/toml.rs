//! TOML configuration file parsing.
//!
//! Defines the structure of the configuration file with serde.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::ConfigError;

/// Root configuration structure from TOML file.
///
/// All fields are optional to allow partial configuration
/// that can be merged with CLI arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomlConfig {
    /// Webhook configuration section
    #[serde(default)]
    pub webhook: WebhookSection,

    /// Network adapter filter configuration
    #[serde(default)]
    pub filter: FilterSection,

    /// Monitoring configuration
    #[serde(default)]
    pub monitor: MonitorSection,

    /// Retry policy configuration
    #[serde(default)]
    pub retry: RetrySection,
}

/// Webhook configuration section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebhookSection {
    /// Webhook URL
    pub url: Option<String>,

    /// IP version to monitor: "ipv4", "ipv6", or "both"
    pub ip_version: Option<String>,

    /// HTTP method (default: POST)
    pub method: Option<String>,

    /// HTTP headers as key-value pairs
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Bearer token for Authorization header
    pub bearer: Option<String>,

    /// Handlebars body template
    pub body_template: Option<String>,
}

/// Adapter filter configuration section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterSection {
    /// Regex patterns for adapters to include (by name)
    #[serde(default)]
    pub include: Vec<String>,

    /// Regex patterns for adapters to exclude (by name)
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Adapter kinds to include (e.g., "ethernet", "wireless")
    #[serde(default)]
    pub include_kinds: Vec<String>,

    /// Adapter kinds to exclude (e.g., "virtual", "loopback")
    #[serde(default)]
    pub exclude_kinds: Vec<String>,
}

/// Monitoring configuration section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorSection {
    /// Polling interval in seconds
    pub poll_interval: Option<u64>,

    /// Disable API event listening, use polling only
    #[serde(default)]
    pub poll_only: bool,

    /// Path to state file for detecting changes across restarts
    pub state_file: Option<String>,
}

/// Retry policy configuration section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrySection {
    /// Maximum number of retry attempts
    pub max_attempts: Option<u32>,

    /// Initial retry delay in seconds
    pub initial_delay: Option<u64>,

    /// Maximum retry delay in seconds
    pub max_delay: Option<u64>,

    /// Backoff multiplier
    pub multiplier: Option<f64>,
}

impl TomlConfig {
    /// Loads configuration from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::FileRead {
            path: path.to_path_buf(),
            source: e,
        })?;

        Self::parse(&content)
    }

    /// Parses configuration from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the TOML is invalid.
    pub fn parse(content: &str) -> Result<Self, ConfigError> {
        toml::from_str(content).map_err(ConfigError::from)
    }
}

/// Generates a default configuration file with comments.
#[must_use]
pub fn default_config_template() -> String {
    r#"# DDNS-A Configuration File
# Documentation: https://github.com/doraemonkeys/ddns-a

[webhook]
# Webhook URL (required)
# url = "https://api.example.com/ddns"

# IP version to monitor (required)
# Accepted values: "ipv4"/"v4"/"4", "ipv6"/"v6"/"6", or "both"/"all"/"dual"
# ip_version = "both"

# HTTP method (default: POST, can be overridden by --method CLI flag)
# method = "POST"

# HTTP headers
# [webhook.headers]
# X-Custom-Header = "value"

# Bearer token for Authorization header
# bearer = "your-token-here"

# Handlebars body template
# Available variables: {{adapter}}, {{address}}, {{timestamp}}, {{kind}}
# body_template = '{"ip": "{{address}}", "adapter": "{{adapter}}"}'

[filter]
# Adapter kinds to include (empty = all kinds)
# Valid values: ethernet, wireless, virtual, loopback
# Note: CLI --include-kind REPLACES these entirely (not merged)
# include_kinds = ["ethernet", "wireless"]

# Adapter kinds to exclude
# Note: Loopback is excluded by default unless explicitly included
# Note: CLI --exclude-kind REPLACES these entirely (not merged)
# exclude_kinds = ["virtual"]

# Regex patterns for adapters to include by name (empty = all names)
# Note: CLI --include-adapter REPLACES these entirely (not merged)
# include = ["^eth", "^Ethernet"]

# Regex patterns for adapters to exclude by name
# Note: CLI --exclude-adapter REPLACES these entirely (not merged)
# exclude = ["^Docker", "^vEthernet"]

[monitor]
# Polling interval in seconds (default: 60)
poll_interval = 60

# Disable API event listening, use polling only
# poll_only = false

# Path to state file for detecting changes across restarts
# If set, the program will compare current IP addresses with the saved state
# and trigger webhooks for any changes detected during the program restart
# state_file = "ddns-a-state.json"

[retry]
# Maximum number of retry attempts (default: 3)
# max_attempts = 3

# Initial retry delay in seconds (default: 5)
# initial_delay = 5

# Maximum retry delay in seconds (default: 60)
# max_delay = 60

# Backoff multiplier (default: 2.0)
# multiplier = 2.0
"#
    .to_string()
}
