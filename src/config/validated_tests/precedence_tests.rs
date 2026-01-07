//! Tests for CLI vs TOML precedence rules.

use std::time::Duration;

use crate::network::IpVersion;

use super::*;

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
