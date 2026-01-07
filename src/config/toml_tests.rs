//! Tests for TOML configuration parsing.

use super::toml::{TomlConfig, default_config_template};

mod parsing {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let toml = r#"
            [webhook]
            url = "https://example.com/webhook"
            ip_version = "ipv4"
        "#;

        let config = TomlConfig::parse(toml).unwrap();
        assert_eq!(
            config.webhook.url.as_deref(),
            Some("https://example.com/webhook")
        );
        assert_eq!(config.webhook.ip_version.as_deref(), Some("ipv4"));
    }

    #[test]
    fn parse_full_webhook_section() {
        let toml = r#"
            [webhook]
            url = "https://api.example.com/ddns"
            ip_version = "both"
            method = "PUT"
            bearer = "secret-token"
            body_template = '{"ip": "{{address}}"}'

            [webhook.headers]
            X-Custom-Header = "custom-value"
            Content-Type = "application/json"
        "#;

        let config = TomlConfig::parse(toml).unwrap();
        let webhook = &config.webhook;

        assert_eq!(webhook.url.as_deref(), Some("https://api.example.com/ddns"));
        assert_eq!(webhook.ip_version.as_deref(), Some("both"));
        assert_eq!(webhook.method.as_deref(), Some("PUT"));
        assert_eq!(webhook.bearer.as_deref(), Some("secret-token"));
        assert_eq!(
            webhook.body_template.as_deref(),
            Some(r#"{"ip": "{{address}}"}"#)
        );
        assert_eq!(webhook.headers.len(), 2);
        assert_eq!(
            webhook.headers.get("X-Custom-Header").map(String::as_str),
            Some("custom-value")
        );
    }

    #[test]
    fn parse_filter_section() {
        let toml = r#"
            [filter]
            include = ["^eth", "^wlan"]
            exclude = ["^Docker", "^vEthernet"]
            exclude_virtual = true
        "#;

        let config = TomlConfig::parse(toml).unwrap();
        let filter = &config.filter;

        assert_eq!(filter.include, vec!["^eth", "^wlan"]);
        assert_eq!(filter.exclude, vec!["^Docker", "^vEthernet"]);
        assert!(filter.exclude_virtual);
    }

    #[test]
    fn parse_monitor_section() {
        let toml = r"
            [monitor]
            poll_interval = 120
            poll_only = true
        ";

        let config = TomlConfig::parse(toml).unwrap();
        let monitor = &config.monitor;

        assert_eq!(monitor.poll_interval, Some(120));
        assert!(monitor.poll_only);
    }

    #[test]
    fn parse_retry_section() {
        let toml = r"
            [retry]
            max_attempts = 5
            initial_delay = 10
            max_delay = 120
            multiplier = 1.5
        ";

        let config = TomlConfig::parse(toml).unwrap();
        let retry = &config.retry;

        assert_eq!(retry.max_attempts, Some(5));
        assert_eq!(retry.initial_delay, Some(10));
        assert_eq!(retry.max_delay, Some(120));
        assert_eq!(retry.multiplier, Some(1.5));
    }

    #[test]
    fn parse_empty_config() {
        let toml = "";
        let config = TomlConfig::parse(toml).unwrap();

        assert!(config.webhook.url.is_none());
        assert!(config.webhook.ip_version.is_none());
        assert!(config.filter.include.is_empty());
        assert!(!config.filter.exclude_virtual);
    }

    #[test]
    fn reject_unknown_fields() {
        let toml = r#"
            [webhook]
            url = "https://example.com"
            unknown_field = "value"
        "#;

        let result = TomlConfig::parse(toml);
        assert!(result.is_err());
    }

    #[test]
    fn reject_unknown_sections() {
        let toml = r#"
            [unknown_section]
            key = "value"
        "#;

        let result = TomlConfig::parse(toml);
        assert!(result.is_err());
    }
}

mod default_template {
    use super::*;

    #[test]
    fn template_is_valid_toml() {
        let template = default_config_template();
        // Template should be parseable (comments are ignored, commented-out values don't matter)
        let result = TomlConfig::parse(&template);
        assert!(
            result.is_ok(),
            "Template should be valid TOML: {:?}",
            result.err()
        );
    }

    #[test]
    fn template_contains_all_sections() {
        let template = default_config_template();

        assert!(
            template.contains("[webhook]"),
            "Template should contain webhook section"
        );
        assert!(
            template.contains("[filter]"),
            "Template should contain filter section"
        );
        assert!(
            template.contains("[monitor]"),
            "Template should contain monitor section"
        );
        assert!(
            template.contains("[retry]"),
            "Template should contain retry section"
        );
    }

    #[test]
    fn template_documents_required_fields() {
        let template = default_config_template();

        assert!(template.contains("url"), "Template should document url");
        assert!(
            template.contains("ip_version"),
            "Template should document ip_version"
        );
    }

    #[test]
    fn template_includes_examples() {
        let template = default_config_template();

        // Check for body_template example
        assert!(
            template.contains("{{address}}"),
            "Template should show template variable example"
        );
        assert!(
            template.contains("{{adapter}}"),
            "Template should show adapter variable example"
        );
    }
}

mod file_loading {
    use std::io::Write;
    use std::path::Path;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::config::ConfigError;

    #[test]
    fn load_valid_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
            [webhook]
            url = "https://example.com"
            ip_version = "ipv4"
        "#
        )
        .unwrap();

        let config = TomlConfig::load(file.path()).unwrap();
        assert_eq!(config.webhook.url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let path = Path::new("nonexistent_config_file_12345.toml");
        let result = TomlConfig::load(path);

        assert!(matches!(result, Err(ConfigError::FileRead { .. })));
    }

    #[test]
    fn load_invalid_toml_file_returns_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "this is not valid toml {{{{").unwrap();

        let result = TomlConfig::load(file.path());

        assert!(matches!(result, Err(ConfigError::TomlParse(_))));
    }
}
