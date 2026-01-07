//! Tests for adapter filtering configuration.

use super::*;

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
