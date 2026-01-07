//! Validated configuration after merging CLI and TOML sources.
//!
//! This module contains the final, validated configuration that is used
//! by the application. All validation is performed during construction.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use handlebars::Handlebars;
use http::header::{AUTHORIZATION, HeaderName, HeaderValue};
use http::{HeaderMap, Method};
use url::Url;

use crate::network::IpVersion;
use crate::network::filter::{
    CompositeFilter, ExcludeLoopbackFilter, ExcludeVirtualFilter, NameRegexFilter,
};
use crate::webhook::RetryPolicy;

use super::cli::Cli;
use super::defaults;
use super::error::{ConfigError, field};
use super::toml::TomlConfig;

/// Fully validated configuration ready for use by the application.
///
/// This struct represents a complete, validated configuration where all
/// required fields are present and all values have been validated.
///
/// # Construction
///
/// Use [`ValidatedConfig::from_raw`] to create from CLI args and optional TOML config.
/// The function validates all inputs and returns errors for invalid configurations.
#[derive(Debug)]
pub struct ValidatedConfig {
    /// IP version to monitor (required)
    pub ip_version: IpVersion,

    /// Webhook URL (required)
    pub url: Url,

    /// HTTP method for webhook requests
    pub method: Method,

    /// HTTP headers for webhook requests
    pub headers: HeaderMap,

    /// Handlebars body template (optional)
    pub body_template: Option<String>,

    /// Adapter filter configuration
    pub filter: CompositeFilter,

    /// Polling interval
    pub poll_interval: Duration,

    /// Whether to use polling only (no API events)
    pub poll_only: bool,

    /// Retry policy for failed webhook requests
    pub retry_policy: RetryPolicy,

    /// Path to state file for detecting changes across restarts.
    /// If `None`, state persistence is disabled.
    pub state_file: Option<PathBuf>,

    /// Dry-run mode (log changes without sending webhooks)
    pub dry_run: bool,

    /// Verbose logging enabled
    pub verbose: bool,
}

impl fmt::Display for ValidatedConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state_file_str = self
            .state_file
            .as_ref()
            .map_or_else(|| "none".to_string(), |p| p.display().to_string());

        write!(
            f,
            "Config {{ url: {}, ip_version: {}, method: {}, poll_interval: {}s, poll_only: {}, \
             retry: {}x/{}s, state_file: {}, dry_run: {}, filters: {} }}",
            self.url,
            self.ip_version,
            self.method,
            self.poll_interval.as_secs(),
            self.poll_only,
            self.retry_policy.max_attempts,
            self.retry_policy.initial_delay.as_secs(),
            state_file_str,
            self.dry_run,
            self.filter.len(),
        )
    }
}

impl ValidatedConfig {
    /// Creates a validated configuration from CLI arguments and optional TOML config.
    ///
    /// CLI arguments take precedence over TOML config values.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required fields are missing (`url`, `ip_version`)
    /// - URL is invalid
    /// - Regex patterns are invalid
    /// - Duration values are zero
    /// - Header format is invalid
    pub fn from_raw(cli: &Cli, toml: Option<&TomlConfig>) -> Result<Self, ConfigError> {
        // Merge and validate IP version (required)
        let ip_version = Self::resolve_ip_version(cli, toml)?;

        // Merge and validate URL (required)
        let url = Self::resolve_url(cli, toml)?;

        // Merge HTTP method (CLI default: POST)
        let method = Self::resolve_method(cli, toml)?;

        // Merge headers
        let headers = Self::resolve_headers(cli, toml)?;

        // Merge and validate body template
        let body_template = Self::resolve_body_template(cli, toml)?;

        // Build adapter filter
        let filter = Self::build_filter(cli, toml)?;

        // Merge poll interval (CLI default: 60)
        let poll_interval = Self::resolve_poll_interval(cli, toml)?;

        // Merge poll_only (CLI wins if true)
        let poll_only = cli.poll_only || toml.is_some_and(|t| t.monitor.poll_only);

        // Build retry policy
        let retry_policy = Self::build_retry_policy(cli, toml)?;

        // Resolve state file path (CLI takes precedence over TOML)
        let state_file = Self::resolve_state_file(cli, toml);

        Ok(Self {
            ip_version,
            url,
            method,
            headers,
            body_template,
            filter,
            poll_interval,
            poll_only,
            retry_policy,
            state_file,
            dry_run: cli.dry_run,
            verbose: cli.verbose,
        })
    }

    /// Loads and merges configuration from CLI and optional config file.
    ///
    /// If `cli.config` is set, loads the TOML file from that path.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config file cannot be read or parsed
    /// - The merged configuration is invalid
    pub fn load(cli: &Cli) -> Result<Self, ConfigError> {
        let toml = if let Some(ref path) = cli.config {
            Some(TomlConfig::load(path)?)
        } else {
            None
        };

        Self::from_raw(cli, toml.as_ref())
    }

    fn resolve_ip_version(cli: &Cli, toml: Option<&TomlConfig>) -> Result<IpVersion, ConfigError> {
        // CLI takes precedence
        if let Some(version) = cli.ip_version {
            return Ok(version.into());
        }

        // Fall back to TOML
        if let Some(toml) = toml {
            if let Some(ref version_str) = toml.webhook.ip_version {
                return parse_ip_version(version_str);
            }
        }

        Err(ConfigError::missing(
            field::IP_VERSION,
            "Use --ip-version or set webhook.ip_version in config file",
        ))
    }

    fn resolve_url(cli: &Cli, toml: Option<&TomlConfig>) -> Result<Url, ConfigError> {
        // CLI takes precedence
        let url_str = cli
            .url
            .as_deref()
            .or_else(|| toml.and_then(|t| t.webhook.url.as_deref()))
            .ok_or_else(|| {
                ConfigError::missing(field::URL, "Use --url or set webhook.url in config file")
            })?;

        Url::parse(url_str).map_err(|e| ConfigError::InvalidUrl {
            url: url_str.to_string(),
            reason: e.to_string(),
        })
    }

    fn resolve_method(cli: &Cli, toml: Option<&TomlConfig>) -> Result<Method, ConfigError> {
        // Priority: CLI explicit > TOML > default
        let method_str = cli
            .method
            .as_deref()
            .or_else(|| toml.and_then(|t| t.webhook.method.as_deref()))
            .unwrap_or(defaults::METHOD);

        method_str
            .parse::<Method>()
            .map_err(|_| ConfigError::InvalidMethod(method_str.to_string()))
    }

    fn resolve_headers(cli: &Cli, toml: Option<&TomlConfig>) -> Result<HeaderMap, ConfigError> {
        let mut headers = HeaderMap::new();

        // Add TOML headers first (CLI can override)
        if let Some(toml) = toml {
            for (name, value) in &toml.webhook.headers {
                let header_name = parse_header_name(name)?;
                let header_value = parse_header_value(name, value)?;
                headers.insert(header_name, header_value);
            }
        }

        // Add CLI headers (override TOML)
        for header_str in &cli.headers {
            let (name, value) = parse_header_string(header_str)?;
            let header_name = parse_header_name(&name)?;
            let header_value = parse_header_value(&name, &value)?;
            headers.insert(header_name, header_value);
        }

        // Handle bearer token (CLI wins, then TOML)
        let bearer = cli
            .bearer
            .as_deref()
            .or_else(|| toml.and_then(|t| t.webhook.bearer.as_deref()));

        if let Some(token) = bearer {
            let auth_value = format!("Bearer {token}");
            let header_value = parse_header_value("Authorization", &auth_value)?;
            headers.insert(AUTHORIZATION, header_value);
        }

        Ok(headers)
    }

    fn resolve_body_template(
        cli: &Cli,
        toml: Option<&TomlConfig>,
    ) -> Result<Option<String>, ConfigError> {
        let template = cli
            .body_template
            .clone()
            .or_else(|| toml.and_then(|t| t.webhook.body_template.clone()));

        // Validate Handlebars syntax if template is provided
        if let Some(ref tmpl) = template {
            Self::validate_template(tmpl)?;
        }

        Ok(template)
    }

    fn validate_template(template: &str) -> Result<(), ConfigError> {
        let hbs = Handlebars::new();
        // Compile-check only; render with empty context to validate syntax
        hbs.render_template(template, &serde_json::json!({}))
            .map_err(|e| ConfigError::InvalidTemplate {
                reason: e.to_string(),
            })?;
        Ok(())
    }

    fn build_filter(cli: &Cli, toml: Option<&TomlConfig>) -> Result<CompositeFilter, ConfigError> {
        let mut filter = CompositeFilter::new();

        // Always exclude loopback
        filter = filter.with(ExcludeLoopbackFilter);

        // Exclude virtual if CLI flag or TOML setting
        let exclude_virtual = cli.exclude_virtual || toml.is_some_and(|t| t.filter.exclude_virtual);

        if exclude_virtual {
            filter = filter.with(ExcludeVirtualFilter);
        }

        // Add include patterns from CLI
        for pattern in &cli.include_adapters {
            let regex_filter =
                NameRegexFilter::include(pattern).map_err(|e| ConfigError::InvalidRegex {
                    pattern: pattern.clone(),
                    source: e,
                })?;
            filter = filter.with(regex_filter);
        }

        // Add include patterns from TOML (if CLI didn't provide any)
        if cli.include_adapters.is_empty() {
            if let Some(toml) = toml {
                for pattern in &toml.filter.include {
                    let regex_filter = NameRegexFilter::include(pattern).map_err(|e| {
                        ConfigError::InvalidRegex {
                            pattern: pattern.clone(),
                            source: e,
                        }
                    })?;
                    filter = filter.with(regex_filter);
                }
            }
        }

        // Add exclude patterns from CLI
        for pattern in &cli.exclude_adapters {
            let regex_filter =
                NameRegexFilter::exclude(pattern).map_err(|e| ConfigError::InvalidRegex {
                    pattern: pattern.clone(),
                    source: e,
                })?;
            filter = filter.with(regex_filter);
        }

        // Add exclude patterns from TOML (if CLI didn't provide any)
        if cli.exclude_adapters.is_empty() {
            if let Some(toml) = toml {
                for pattern in &toml.filter.exclude {
                    let regex_filter = NameRegexFilter::exclude(pattern).map_err(|e| {
                        ConfigError::InvalidRegex {
                            pattern: pattern.clone(),
                            source: e,
                        }
                    })?;
                    filter = filter.with(regex_filter);
                }
            }
        }

        Ok(filter)
    }

    fn resolve_poll_interval(
        cli: &Cli,
        toml: Option<&TomlConfig>,
    ) -> Result<Duration, ConfigError> {
        // Priority: CLI explicit > TOML > default
        let seconds = cli
            .poll_interval
            .or_else(|| toml.and_then(|t| t.monitor.poll_interval))
            .unwrap_or(defaults::POLL_INTERVAL_SECS);

        if seconds == 0 {
            return Err(ConfigError::InvalidDuration {
                field: "poll_interval",
                reason: "must be greater than 0".to_string(),
            });
        }

        Ok(Duration::from_secs(seconds))
    }

    fn build_retry_policy(
        cli: &Cli,
        toml: Option<&TomlConfig>,
    ) -> Result<RetryPolicy, ConfigError> {
        let retry = toml.map(|t| &t.retry);

        // Priority: CLI explicit > TOML > default
        let max_attempts = cli
            .retry_max
            .or_else(|| retry.and_then(|r| r.max_attempts))
            .unwrap_or(defaults::RETRY_MAX_ATTEMPTS);

        let initial_delay_secs = cli
            .retry_delay
            .or_else(|| retry.and_then(|r| r.initial_delay))
            .unwrap_or(defaults::RETRY_INITIAL_DELAY_SECS);

        let max_delay_secs = retry
            .and_then(|r| r.max_delay)
            .unwrap_or(defaults::RETRY_MAX_DELAY_SECS);

        let multiplier = retry
            .and_then(|r| r.multiplier)
            .unwrap_or(defaults::RETRY_MULTIPLIER);

        if max_attempts == 0 {
            return Err(ConfigError::InvalidRetry(
                "max_attempts must be greater than 0".to_string(),
            ));
        }

        if initial_delay_secs == 0 {
            return Err(ConfigError::InvalidRetry(
                "initial_delay must be greater than 0".to_string(),
            ));
        }

        if multiplier <= 0.0 || !multiplier.is_finite() {
            return Err(ConfigError::InvalidRetry(
                "multiplier must be a positive finite number".to_string(),
            ));
        }

        if max_delay_secs < initial_delay_secs {
            return Err(ConfigError::InvalidRetry(format!(
                "max_delay ({max_delay_secs}s) must be >= initial_delay ({initial_delay_secs}s)"
            )));
        }

        Ok(RetryPolicy::new()
            .with_max_attempts(max_attempts)
            .with_initial_delay(Duration::from_secs(initial_delay_secs))
            .with_max_delay(Duration::from_secs(max_delay_secs))
            .with_multiplier(multiplier))
    }

    fn resolve_state_file(cli: &Cli, toml: Option<&TomlConfig>) -> Option<PathBuf> {
        // CLI takes precedence
        if let Some(ref path) = cli.state_file {
            return Some(path.clone());
        }

        // Fall back to TOML
        toml.and_then(|t| t.monitor.state_file.as_ref().map(PathBuf::from))
    }
}

/// Writes the default configuration template to a file.
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_default_config(path: &Path) -> Result<(), ConfigError> {
    let template = super::toml::default_config_template();
    std::fs::write(path, template).map_err(|e| ConfigError::FileWrite {
        path: path.to_path_buf(),
        source: e,
    })
}

// Helper functions

fn parse_ip_version(s: &str) -> Result<IpVersion, ConfigError> {
    match s.to_lowercase().as_str() {
        "ipv4" | "v4" | "4" => Ok(IpVersion::V4),
        "ipv6" | "v6" | "6" => Ok(IpVersion::V6),
        "both" | "all" | "dual" => Ok(IpVersion::Both),
        _ => Err(ConfigError::InvalidIpVersion {
            value: s.to_string(),
        }),
    }
}

fn parse_header_string(s: &str) -> Result<(String, String), ConfigError> {
    // Try "Key=Value" format first
    if let Some((name, value)) = s.split_once('=') {
        return Ok((name.trim().to_string(), value.trim().to_string()));
    }

    // Try "Key: Value" format
    if let Some((name, value)) = s.split_once(':') {
        return Ok((name.trim().to_string(), value.trim().to_string()));
    }

    Err(ConfigError::InvalidHeader {
        value: s.to_string(),
    })
}

fn parse_header_name(name: &str) -> Result<HeaderName, ConfigError> {
    name.parse::<HeaderName>()
        .map_err(|e| ConfigError::InvalidHeaderName {
            name: name.to_string(),
            reason: e.to_string(),
        })
}

fn parse_header_value(name: &str, value: &str) -> Result<HeaderValue, ConfigError> {
    HeaderValue::from_str(value).map_err(|e| ConfigError::InvalidHeaderValue {
        name: name.to_string(),
        reason: e.to_string(),
    })
}
