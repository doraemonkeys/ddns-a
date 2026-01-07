//! Error types for configuration parsing and validation.

use std::path::PathBuf;

use thiserror::Error;

/// Error type for configuration operations.
///
/// Covers errors from parsing, validation, and file operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to read the configuration file.
    #[error("Failed to read config file '{}': {source}", path.display())]
    FileRead {
        /// Path to the config file
        path: PathBuf,
        /// Underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse the TOML configuration.
    #[error("Failed to parse TOML config: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Failed to write configuration file (for init command).
    #[error("Failed to write config file '{}': {source}", path.display())]
    FileWrite {
        /// Path to the config file
        path: PathBuf,
        /// Underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// Missing required field that must be provided by CLI or config file.
    #[error("Missing required field: {field}. {hint}")]
    MissingRequired {
        /// Name of the missing field
        field: &'static str,
        /// Hint for how to provide the value
        hint: &'static str,
    },

    /// Invalid URL provided.
    #[error("Invalid URL '{url}': {reason}")]
    InvalidUrl {
        /// The invalid URL string
        url: String,
        /// Reason for invalidity
        reason: String,
    },

    /// Invalid regex pattern for adapter filtering.
    #[error("Invalid regex pattern '{pattern}': {source}")]
    InvalidRegex {
        /// The invalid pattern
        pattern: String,
        /// Underlying regex error
        #[source]
        source: regex::Error,
    },

    /// Invalid duration value (zero or too large).
    #[error("Invalid duration for {field}: {reason}")]
    InvalidDuration {
        /// Name of the field
        field: &'static str,
        /// Reason for invalidity
        reason: String,
    },

    /// Invalid retry configuration.
    #[error("Invalid retry configuration: {0}")]
    InvalidRetry(String),

    /// Invalid HTTP method.
    #[error("Invalid HTTP method '{0}'")]
    InvalidMethod(String),

    /// Invalid IP version value.
    #[error("Invalid IP version '{value}': expected ipv4, ipv6, or both")]
    InvalidIpVersion {
        /// The invalid value provided
        value: String,
    },

    /// Invalid header format.
    #[error("Invalid header format '{value}': expected 'Key=Value' or 'Key: Value'")]
    InvalidHeader {
        /// The invalid header string
        value: String,
    },

    /// Invalid header name.
    #[error("Invalid header name '{name}': {reason}")]
    InvalidHeaderName {
        /// The invalid header name
        name: String,
        /// Reason for invalidity
        reason: String,
    },

    /// Invalid header value.
    #[error("Invalid header value for '{name}': {reason}")]
    InvalidHeaderValue {
        /// The header name
        name: String,
        /// Reason for invalidity
        reason: String,
    },

    /// Invalid body template (Handlebars syntax error).
    #[error("Invalid body template: {reason}")]
    InvalidTemplate {
        /// Reason for invalidity
        reason: String,
    },
}

/// Well-known field names for `MissingRequired` errors.
///
/// Use these constants for compile-time safety when matching field names.
pub mod field {
    /// The webhook URL field.
    pub const URL: &str = "url";
    /// The IP version field.
    pub const IP_VERSION: &str = "ip_version";
}

impl ConfigError {
    /// Creates a `MissingRequired` error for a required field.
    #[must_use]
    pub const fn missing(field: &'static str, hint: &'static str) -> Self {
        Self::MissingRequired { field, hint }
    }
}
