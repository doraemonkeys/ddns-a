//! Tests for webhook configuration: URL, method, headers, body template, IP version, display.

use crate::network::IpVersion;

use super::*;

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

    #[test]
    fn cli_ip_version_both() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "both"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        assert_eq!(config.ip_version, IpVersion::Both);
    }

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
