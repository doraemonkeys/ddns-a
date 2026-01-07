//! Tests for validated configuration.

use std::time::Duration;

use http::Method;

use crate::network::IpVersion;

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

mod required_fields {
    use super::*;

    #[test]
    fn missing_url_returns_error() {
        let cli = cli(&["--ip-version", "ipv4"]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(
            result,
            Err(ConfigError::MissingRequired { field: "url", .. })
        ));
    }

    #[test]
    fn missing_ip_version_returns_error() {
        let cli = cli(&["--url", "https://example.com"]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(
            result,
            Err(ConfigError::MissingRequired {
                field: "ip_version",
                ..
            })
        ));
    }

    #[test]
    fn both_required_fields_from_cli() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.url.as_str(), "https://example.com/");
        assert_eq!(config.ip_version, IpVersion::V4);
    }

    #[test]
    fn required_fields_from_toml() {
        let cli = cli(&[]);
        let toml = toml(
            r#"
            [webhook]
            url = "https://example.com/webhook"
            ip_version = "both"
        "#,
        );

        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.url.as_str(), "https://example.com/webhook");
        assert_eq!(config.ip_version, IpVersion::Both);
    }
}

mod cli_precedence {
    use super::*;

    #[test]
    fn cli_url_overrides_toml() {
        let cli = cli(&["--url", "https://cli.example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [webhook]
            url = "https://toml.example.com"
        "#,
        );

        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.url.as_str(), "https://cli.example.com/");
    }

    #[test]
    fn cli_ip_version_overrides_toml() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv6"]);
        let toml = toml(
            r#"
            [webhook]
            ip_version = "ipv4"
        "#,
        );

        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.ip_version, IpVersion::V6);
    }

    #[test]
    fn cli_exclude_virtual_wins() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--exclude-virtual",
        ]);
        let toml = toml(
            r"
            [filter]
            exclude_virtual = false
        ",
        );

        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        // Filter should include ExcludeVirtualFilter (len >= 2: loopback + virtual)
        assert!(config.filter.len() >= 2);
    }
}

mod ip_version_parsing {
    use super::*;

    #[test]
    fn parse_v4_variants() {
        for version in &["ipv4", "v4", "4"] {
            let toml_str = format!(
                r#"
                [webhook]
                url = "https://example.com"
                ip_version = "{version}"
            "#
            );
            let cli = cli(&[]);
            let toml = toml(&toml_str);

            let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();
            assert_eq!(
                config.ip_version,
                IpVersion::V4,
                "Failed for variant: {version}"
            );
        }
    }

    #[test]
    fn parse_v6_variants() {
        for version in &["ipv6", "v6", "6"] {
            let toml_str = format!(
                r#"
                [webhook]
                url = "https://example.com"
                ip_version = "{version}"
            "#
            );
            let cli = cli(&[]);
            let toml = toml(&toml_str);

            let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();
            assert_eq!(
                config.ip_version,
                IpVersion::V6,
                "Failed for variant: {version}"
            );
        }
    }

    #[test]
    fn parse_both_variants() {
        for version in &["both", "all", "dual"] {
            let toml_str = format!(
                r#"
                [webhook]
                url = "https://example.com"
                ip_version = "{version}"
            "#
            );
            let cli = cli(&[]);
            let toml = toml(&toml_str);

            let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();
            assert_eq!(
                config.ip_version,
                IpVersion::Both,
                "Failed for variant: {version}"
            );
        }
    }
}

mod url_validation {
    use super::*;

    #[test]
    fn valid_https_url() {
        let cli = cli(&[
            "--url",
            "https://api.example.com/ddns",
            "--ip-version",
            "ipv4",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.url.scheme(), "https");
        assert_eq!(config.url.host_str(), Some("api.example.com"));
    }

    #[test]
    fn valid_http_url() {
        let cli = cli(&[
            "--url",
            "http://localhost:8080/webhook",
            "--ip-version",
            "ipv4",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.url.scheme(), "http");
        assert_eq!(config.url.port(), Some(8080));
    }

    #[test]
    fn invalid_url_returns_error() {
        let cli = cli(&["--url", "not-a-valid-url", "--ip-version", "ipv4"]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidUrl { .. })));
    }
}

mod http_method {
    use super::*;

    #[test]
    fn default_is_post() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.method, Method::POST);
    }

    #[test]
    fn toml_method_overrides_default() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [webhook]
            method = "PUT"
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.method, Method::PUT);
    }

    #[test]
    fn cli_method_overrides_toml() {
        // Explicit CLI value takes precedence over TOML
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--method",
            "DELETE",
        ]);
        let toml = toml(
            r#"
            [webhook]
            method = "PUT"
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.method, Method::DELETE);
    }

    #[test]
    fn custom_method() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--method",
            "PUT",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.method, Method::PUT);
    }

    #[test]
    fn custom_method_is_accepted() {
        // RFC 7230 allows custom HTTP methods
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--method",
            "CUSTOM",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.method.as_str(), "CUSTOM");
    }

    #[test]
    fn invalid_method_returns_error() {
        // Empty or whitespace methods are invalid
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--method",
            "",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidMethod(_))));
    }
}

mod headers {
    use super::*;

    #[test]
    fn parse_equal_format() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--header",
            "X-Api-Key=secret123",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let value = config.headers.get("X-Api-Key").unwrap();
        assert_eq!(value.to_str().unwrap(), "secret123");
    }

    #[test]
    fn parse_colon_format() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--header",
            "Content-Type: application/json",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let value = config.headers.get("Content-Type").unwrap();
        assert_eq!(value.to_str().unwrap(), "application/json");
    }

    #[test]
    fn bearer_token_adds_authorization() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--bearer",
            "my-token",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let auth = config.headers.get("Authorization").unwrap();
        assert_eq!(auth.to_str().unwrap(), "Bearer my-token");
    }

    #[test]
    fn cli_headers_override_toml() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--header",
            "X-Key=cli-value",
        ]);
        let toml = toml(
            r#"
            [webhook.headers]
            X-Key = "toml-value"
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        let value = config.headers.get("X-Key").unwrap();
        assert_eq!(value.to_str().unwrap(), "cli-value");
    }

    #[test]
    fn invalid_header_format_returns_error() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--header",
            "no-separator-here",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidHeader { .. })));
    }

    #[test]
    fn header_value_containing_equals() {
        // Value containing '=' should be preserved (split on first occurrence only)
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--header",
            "X-Token=abc=def=123",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let value = config.headers.get("X-Token").unwrap();
        assert_eq!(value.to_str().unwrap(), "abc=def=123");
    }

    #[test]
    fn header_value_containing_colon() {
        // Value containing ':' should be preserved (split on first occurrence only)
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--header",
            "X-Time: 12:34:56",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let value = config.headers.get("X-Time").unwrap();
        assert_eq!(value.to_str().unwrap(), "12:34:56");
    }
}

mod filter_building {
    use super::*;

    #[test]
    fn always_excludes_loopback() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        // At minimum, ExcludeLoopbackFilter is always present
        assert!(!config.filter.is_empty());
    }

    #[test]
    fn exclude_virtual_adds_filter() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--exclude-virtual",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        // Should have loopback + virtual filters
        assert!(config.filter.len() >= 2);
    }

    #[test]
    fn include_pattern_adds_filter() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--include-adapter",
            "^eth",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        // loopback + include pattern
        assert!(config.filter.len() >= 2);
    }

    #[test]
    fn invalid_regex_returns_error() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--include-adapter",
            "[invalid",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidRegex { .. })));
    }
}

mod poll_interval {
    use super::*;

    #[test]
    fn default_is_60_seconds() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.poll_interval, Duration::from_secs(60));
    }

    #[test]
    fn custom_interval() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--poll-interval",
            "120",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.poll_interval, Duration::from_secs(120));
    }

    #[test]
    fn toml_interval_overrides_default() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r"
            [monitor]
            poll_interval = 300
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.poll_interval, Duration::from_secs(300));
    }

    #[test]
    fn cli_interval_overrides_toml() {
        // Explicit CLI value takes precedence over TOML
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--poll-interval",
            "120",
        ]);
        let toml = toml(
            r"
            [monitor]
            poll_interval = 300
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.poll_interval, Duration::from_secs(120));
    }

    #[test]
    fn zero_interval_returns_error() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--poll-interval",
            "0",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(
            result,
            Err(ConfigError::InvalidDuration {
                field: "poll_interval",
                ..
            })
        ));
    }
}

mod retry_policy {
    use super::*;

    #[test]
    fn default_values() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.retry_policy.max_attempts, 3);
        assert_eq!(config.retry_policy.initial_delay, Duration::from_secs(5));
    }

    #[test]
    fn custom_values_from_cli() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--retry-max",
            "5",
            "--retry-delay",
            "10",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.retry_policy.max_attempts, 5);
        assert_eq!(config.retry_policy.initial_delay, Duration::from_secs(10));
    }

    #[test]
    fn custom_values_from_toml() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r"
            [retry]
            max_attempts = 7
            initial_delay = 15
            max_delay = 180
            multiplier = 1.5
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.retry_policy.max_attempts, 7);
        assert_eq!(config.retry_policy.initial_delay, Duration::from_secs(15));
        assert_eq!(config.retry_policy.max_delay, Duration::from_secs(180));
        // Use approximate comparison for floats
        assert!((config.retry_policy.multiplier - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn zero_attempts_returns_error() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--retry-max",
            "0",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidRetry(_))));
    }

    #[test]
    fn zero_delay_returns_error() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--retry-delay",
            "0",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidRetry(_))));
    }
}

mod dry_run_and_verbose {
    use super::*;

    #[test]
    fn dry_run_flag() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--dry-run",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert!(config.dry_run);
    }

    #[test]
    fn verbose_flag() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--verbose",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert!(config.verbose);
    }
}

mod body_template {
    use super::*;

    #[test]
    fn cli_template() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--body-template",
            r#"{"ip":"{{address}}"}"#,
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(
            config.body_template.as_deref(),
            Some(r#"{"ip":"{{address}}"}"#)
        );
    }

    #[test]
    fn toml_template() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [webhook]
            body_template = '{"adapter": "{{adapter}}", "ip": "{{address}}"}'
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert!(config.body_template.is_some());
        assert!(
            config
                .body_template
                .as_ref()
                .unwrap()
                .contains("{{adapter}}")
        );
    }

    #[test]
    fn cli_template_overrides_toml() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--body-template",
            "cli-template",
        ]);
        let toml = toml(
            r#"
            [webhook]
            body_template = "toml-template"
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.body_template.as_deref(), Some("cli-template"));
    }

    #[test]
    fn no_template_is_none() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert!(config.body_template.is_none());
    }

    #[test]
    fn invalid_handlebars_syntax_returns_error() {
        // Unclosed handlebars expression
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--body-template",
            "{{unclosed",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidTemplate { .. })));
    }

    #[test]
    fn invalid_template_in_toml_returns_error() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [webhook]
            body_template = "{{#if}}missing condition{{/if}}"
        "#,
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(result, Err(ConfigError::InvalidTemplate { .. })));
    }

    #[test]
    fn valid_complex_template() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--body-template",
            r#"{"ip": "{{address}}", "adapter": "{{adapter}}", "time": "{{timestamp}}"}"#,
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert!(config.body_template.is_some());
    }
}

mod config_load {
    use std::io::Write;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn load_from_config_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
            [webhook]
            url = "https://example.com/webhook"
            ip_version = "ipv4"
        "#
        )
        .unwrap();

        let cli = cli(&["--config", file.path().to_str().unwrap()]);
        let config = ValidatedConfig::load(&cli).unwrap();

        assert_eq!(config.url.as_str(), "https://example.com/webhook");
        assert_eq!(config.ip_version, IpVersion::V4);
    }

    #[test]
    fn load_without_config_file() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv6"]);
        let config = ValidatedConfig::load(&cli).unwrap();

        assert_eq!(config.url.as_str(), "https://example.com/");
        assert_eq!(config.ip_version, IpVersion::V6);
    }

    #[test]
    fn load_nonexistent_config_file_returns_error() {
        let cli = cli(&["--config", "nonexistent_file_12345.toml"]);
        let result = ValidatedConfig::load(&cli);

        assert!(matches!(result, Err(ConfigError::FileRead { .. })));
    }
}

mod ip_version_cli_both {
    use super::*;

    #[test]
    fn cli_ip_version_both() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "both"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.ip_version, IpVersion::Both);
    }
}

mod filter_toml_patterns {
    use super::*;

    #[test]
    fn include_patterns_from_toml_when_cli_empty() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [filter]
            include = ["^eth", "^wlan"]
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        // loopback + 2 include patterns
        assert!(config.filter.len() >= 3);
    }

    #[test]
    fn exclude_patterns_from_cli() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--exclude-adapter",
            "^Docker",
            "--exclude-adapter",
            "^vEthernet",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        // loopback + 2 exclude patterns
        assert!(config.filter.len() >= 3);
    }

    #[test]
    fn exclude_patterns_from_toml_when_cli_empty() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [filter]
            exclude = ["^Docker", "^vEthernet"]
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        // loopback + 2 exclude patterns
        assert!(config.filter.len() >= 3);
    }

    #[test]
    fn invalid_include_regex_in_toml() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [filter]
            include = ["[invalid"]
        "#,
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(result, Err(ConfigError::InvalidRegex { .. })));
    }

    #[test]
    fn invalid_exclude_regex_from_cli() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--exclude-adapter",
            "[invalid",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidRegex { .. })));
    }

    #[test]
    fn invalid_exclude_regex_in_toml() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [filter]
            exclude = ["[invalid"]
        "#,
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(result, Err(ConfigError::InvalidRegex { .. })));
    }
}

mod retry_policy_validation {
    use super::*;

    #[test]
    fn zero_multiplier_returns_error() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r"
            [retry]
            multiplier = 0.0
        ",
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(result, Err(ConfigError::InvalidRetry(_))));
    }

    #[test]
    fn negative_multiplier_returns_error() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r"
            [retry]
            multiplier = -1.5
        ",
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(result, Err(ConfigError::InvalidRetry(_))));
    }

    #[test]
    fn nan_multiplier_returns_error() {
        // NaN values must be rejected
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        // Cannot specify NaN directly in TOML; test via manual construction
        let mut toml_config = TomlConfig::parse("[webhook]").unwrap();
        // Use a special value that would pass <= 0.0 check but is not finite
        toml_config.retry.multiplier = Some(f64::NAN);

        let result = ValidatedConfig::from_raw(&cli, Some(&toml_config));

        assert!(matches!(result, Err(ConfigError::InvalidRetry(_))));
    }

    #[test]
    fn infinity_multiplier_returns_error() {
        // Infinity values must be rejected
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let mut toml_config = TomlConfig::parse("[webhook]").unwrap();
        toml_config.retry.multiplier = Some(f64::INFINITY);

        let result = ValidatedConfig::from_raw(&cli, Some(&toml_config));

        assert!(matches!(result, Err(ConfigError::InvalidRetry(_))));
    }

    #[test]
    fn neg_infinity_multiplier_returns_error() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let mut toml_config = TomlConfig::parse("[webhook]").unwrap();
        toml_config.retry.multiplier = Some(f64::NEG_INFINITY);

        let result = ValidatedConfig::from_raw(&cli, Some(&toml_config));

        assert!(matches!(result, Err(ConfigError::InvalidRetry(_))));
    }
}

mod retry_delay_validation {
    use super::*;

    #[test]
    fn max_delay_less_than_initial_delay_returns_error() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r"
            [retry]
            initial_delay = 30
            max_delay = 10
        ",
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(result, Err(ConfigError::InvalidRetry(msg)) if msg.contains("max_delay")));
    }

    #[test]
    fn max_delay_equal_to_initial_delay_is_valid() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r"
            [retry]
            initial_delay = 30
            max_delay = 30
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.retry_policy.initial_delay, Duration::from_secs(30));
        assert_eq!(config.retry_policy.max_delay, Duration::from_secs(30));
    }

    #[test]
    fn max_delay_greater_than_initial_delay_is_valid() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r"
            [retry]
            initial_delay = 5
            max_delay = 120
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.retry_policy.initial_delay, Duration::from_secs(5));
        assert_eq!(config.retry_policy.max_delay, Duration::from_secs(120));
    }
}

mod retry_cli_overrides_toml {
    use super::*;

    #[test]
    fn cli_retry_max_overrides_toml() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--retry-max",
            "10",
        ]);
        let toml = toml(
            r"
            [retry]
            max_attempts = 5
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.retry_policy.max_attempts, 10);
    }

    #[test]
    fn cli_retry_delay_overrides_toml() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--retry-delay",
            "30",
        ]);
        let toml = toml(
            r"
            [retry]
            initial_delay = 15
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert_eq!(config.retry_policy.initial_delay, Duration::from_secs(30));
    }
}

mod write_config {
    use std::fs;
    use tempfile::tempdir;

    use super::super::validated::write_default_config;
    use super::*;

    #[test]
    fn write_default_config_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test-config.toml");

        write_default_config(&path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("[webhook]"));
        assert!(content.contains("[filter]"));
        assert!(content.contains("[monitor]"));
        assert!(content.contains("[retry]"));
    }

    #[test]
    fn write_default_config_to_invalid_path_returns_error() {
        use std::path::Path;
        // Try writing to an invalid path (directory that doesn't exist)
        let path = Path::new("/nonexistent_dir_12345/config.toml");
        let result = write_default_config(path);

        assert!(matches!(result, Err(ConfigError::FileWrite { .. })));
    }
}

mod ip_version_invalid {
    use super::*;

    #[test]
    fn invalid_ip_version_string_from_toml() {
        let cli = cli(&["--url", "https://example.com"]);
        let toml = toml(
            r#"
            [webhook]
            ip_version = "invalid_version"
        "#,
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(
            result,
            Err(ConfigError::InvalidIpVersion { value }) if value == "invalid_version"
        ));
    }
}

mod header_validation {
    use super::*;

    #[test]
    fn invalid_header_name_returns_error() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--header",
            "Invalid Header Name=value",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(result, Err(ConfigError::InvalidHeaderName { .. })));
    }

    #[test]
    fn invalid_header_value_returns_error() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--header",
            "X-Header=\x00invalid",
        ]);
        let result = ValidatedConfig::from_raw(&cli, None);

        assert!(matches!(
            result,
            Err(ConfigError::InvalidHeaderValue { .. })
        ));
    }

    #[test]
    fn invalid_header_name_in_toml() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [webhook.headers]
            "Invalid Header Name" = "value"
        "#,
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(result, Err(ConfigError::InvalidHeaderName { .. })));
    }

    #[test]
    fn invalid_header_value_in_toml() {
        // Header values with null bytes are invalid
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        // Use a header with non-visible ASCII (control characters) which is invalid
        let toml_config = TomlConfig::parse(
            r"
            [webhook]
        ",
        )
        .unwrap();
        // Manually construct a config with invalid header value
        let mut toml_config = toml_config;
        toml_config
            .webhook
            .headers
            .insert("X-Header".to_string(), "\x01\x02\x03".to_string());

        let result = ValidatedConfig::from_raw(&cli, Some(&toml_config));

        assert!(matches!(
            result,
            Err(ConfigError::InvalidHeaderValue { .. })
        ));
    }
}

mod display_impl {
    use super::*;

    #[test]
    fn display_shows_key_config() {
        let cli = cli(&[
            "--url",
            "https://api.example.com/webhook",
            "--ip-version",
            "both",
            "--poll-interval",
            "120",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let display = format!("{config}");

        assert!(display.contains("https://api.example.com/webhook"));
        assert!(display.contains("Both"));
        assert!(display.contains("POST"));
        assert!(display.contains("120s"));
    }

    #[test]
    fn display_does_not_leak_bearer_token() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--bearer",
            "super-secret-token-12345",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let display = format!("{config}");

        // Bearer token should NOT appear in display output
        assert!(!display.contains("super-secret-token"));
        assert!(!display.contains("12345"));
    }

    #[test]
    fn display_shows_retry_config() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--retry-max",
            "5",
            "--retry-delay",
            "10",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let display = format!("{config}");

        // Should show retry attempts and delay
        assert!(display.contains("5x"));
        assert!(display.contains("10s"));
    }
}

mod poll_only {
    use super::*;

    #[test]
    fn poll_only_from_toml() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r"
            [monitor]
            poll_only = true
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert!(config.poll_only);
    }

    #[test]
    fn poll_only_from_cli_overrides_toml() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--poll-only",
        ]);
        let toml = toml(
            r"
            [monitor]
            poll_only = false
        ",
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        assert!(config.poll_only);
    }
}
