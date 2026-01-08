//! Tests for adapter filtering configuration.

use crate::network::filter::AdapterFilter;
use crate::network::{AdapterKind, AdapterSnapshot};

use super::*;

mod filter_building {
    use super::*;

    #[test]
    fn always_excludes_loopback() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        // Default: loopback is excluded
        assert!(config.filter.exclude_count() >= 1);

        // Verify loopback adapter is rejected
        let loopback = AdapterSnapshot::new("lo", AdapterKind::Loopback, vec![], vec![]);
        assert!(!config.filter.matches(&loopback));
    }

    #[test]
    fn exclude_kind_virtual_adds_filter() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--exclude-kind",
            "virtual",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        // Should have loopback + virtual excludes
        assert!(config.filter.exclude_count() >= 2);

        // Verify virtual adapter is rejected
        let virtual_adapter =
            AdapterSnapshot::new("vEthernet", AdapterKind::Virtual, vec![], vec![]);
        assert!(!config.filter.matches(&virtual_adapter));
    }

    #[test]
    fn include_kind_accepts_specified_types() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--include-kind",
            "ethernet,wireless",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        let ethernet = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
        let wireless = AdapterSnapshot::new("wlan0", AdapterKind::Wireless, vec![], vec![]);
        let virtual_adapter =
            AdapterSnapshot::new("vEthernet", AdapterKind::Virtual, vec![], vec![]);

        assert!(config.filter.matches(&ethernet));
        assert!(config.filter.matches(&wireless));
        assert!(!config.filter.matches(&virtual_adapter)); // Not in include list
    }

    #[test]
    fn include_loopback_overrides_default_exclude() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--include-kind",
            "loopback",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        // Loopback should NOT be excluded when explicitly included
        let loopback = AdapterSnapshot::new("lo", AdapterKind::Loopback, vec![], vec![]);
        assert!(config.filter.matches(&loopback));
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

        // loopback exclude + include pattern
        assert!(config.filter.exclude_count() >= 1);
        assert!(config.filter.include_count() >= 1);
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

        // 2 include patterns
        assert_eq!(config.filter.include_count(), 2);
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
        assert_eq!(config.filter.exclude_count(), 3);
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
        assert_eq!(config.filter.exclude_count(), 3);
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

mod filter_toml_kinds {
    use super::*;

    #[test]
    fn include_kinds_from_toml() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [filter]
            include_kinds = ["ethernet", "wireless"]
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        // 1 include filter (KindFilter for ethernet+wireless)
        assert!(config.filter.include_count() >= 1);

        let ethernet = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
        let virtual_adapter = AdapterSnapshot::new("vm0", AdapterKind::Virtual, vec![], vec![]);

        assert!(config.filter.matches(&ethernet));
        assert!(!config.filter.matches(&virtual_adapter));
    }

    #[test]
    fn exclude_kinds_from_toml() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [filter]
            exclude_kinds = ["virtual"]
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        // loopback + virtual excludes
        assert!(config.filter.exclude_count() >= 2);

        let virtual_adapter = AdapterSnapshot::new("vm0", AdapterKind::Virtual, vec![], vec![]);
        assert!(!config.filter.matches(&virtual_adapter));
    }

    #[test]
    fn cli_kinds_replace_toml_kinds() {
        let cli = cli(&[
            "--url",
            "https://example.com",
            "--ip-version",
            "ipv4",
            "--include-kind",
            "wireless",
        ]);
        let toml = toml(
            r#"
            [filter]
            include_kinds = ["ethernet"]
        "#,
        );
        let config = ValidatedConfig::from_raw(&cli, Some(&toml)).unwrap();

        let ethernet = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
        let wireless = AdapterSnapshot::new("wlan0", AdapterKind::Wireless, vec![], vec![]);

        // CLI replaces TOML, so only wireless is included
        assert!(!config.filter.matches(&ethernet));
        assert!(config.filter.matches(&wireless));
    }

    #[test]
    fn invalid_kind_in_toml_returns_error() {
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let toml = toml(
            r#"
            [filter]
            include_kinds = ["invalid_kind"]
        "#,
        );
        let result = ValidatedConfig::from_raw(&cli, Some(&toml));

        assert!(matches!(
            result,
            Err(ConfigError::InvalidAdapterKind { .. })
        ));
    }

    #[test]
    fn empty_kinds_when_toml_absent_and_cli_empty() {
        // Edge case: TOML is completely absent (not just with empty include_kinds),
        // and CLI doesn't specify any kinds. Result should be empty set (no kind filters).
        let cli = cli(&["--url", "https://example.com", "--ip-version", "ipv4"]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();

        // No include kind filter added (only default loopback exclude)
        assert_eq!(config.filter.include_count(), 0);

        // All non-loopback kinds should match (no include restriction)
        let ethernet = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
        let wireless = AdapterSnapshot::new("wlan0", AdapterKind::Wireless, vec![], vec![]);
        let virtual_adapter = AdapterSnapshot::new("vm0", AdapterKind::Virtual, vec![], vec![]);

        assert!(config.filter.matches(&ethernet));
        assert!(config.filter.matches(&wireless));
        assert!(config.filter.matches(&virtual_adapter));
    }
}
