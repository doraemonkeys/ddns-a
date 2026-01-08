//! Tests for CLI argument parsing.

use super::cli::{AdapterKindArg, Cli, Command, IpVersionArg};

mod parsing {
    use super::*;

    #[test]
    fn parse_minimal_args() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--url",
            "https://example.com/webhook",
            "--ip-version",
            "ipv4",
        ]);

        assert_eq!(cli.url.as_deref(), Some("https://example.com/webhook"));
        assert_eq!(cli.ip_version, Some(IpVersionArg::V4));
    }

    #[test]
    fn parse_all_ip_versions() {
        let v4 = Cli::parse_from_iter(["ddns-a", "--ip-version", "ipv4"]);
        assert_eq!(v4.ip_version, Some(IpVersionArg::V4));

        let v6 = Cli::parse_from_iter(["ddns-a", "--ip-version", "ipv6"]);
        assert_eq!(v6.ip_version, Some(IpVersionArg::V6));

        let both = Cli::parse_from_iter(["ddns-a", "--ip-version", "both"]);
        assert_eq!(both.ip_version, Some(IpVersionArg::Both));
    }

    #[test]
    fn parse_http_options() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--url",
            "https://example.com",
            "--method",
            "PUT",
            "--header",
            "X-Api-Key=secret",
            "--header",
            "Content-Type: application/json",
            "--bearer",
            "token123",
            "--body-template",
            r#"{"ip":"{{address}}"}"#,
        ]);

        assert_eq!(cli.method.as_deref(), Some("PUT"));
        assert_eq!(cli.headers.len(), 2);
        assert_eq!(cli.headers[0], "X-Api-Key=secret");
        assert_eq!(cli.headers[1], "Content-Type: application/json");
        assert_eq!(cli.bearer.as_deref(), Some("token123"));
        assert_eq!(
            cli.body_template.as_deref(),
            Some(r#"{"ip":"{{address}}"}"#)
        );
    }

    #[test]
    fn parse_filter_options() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--include-adapter",
            "^eth",
            "--include-adapter",
            "^wlan",
            "--exclude-adapter",
            "^Docker",
            "--include-kind",
            "ethernet,wireless",
            "--exclude-kind",
            "virtual",
        ]);

        assert_eq!(cli.include_adapters.len(), 2);
        assert_eq!(cli.exclude_adapters.len(), 1);
        assert_eq!(cli.include_kinds.len(), 2);
        assert_eq!(cli.include_kinds[0], AdapterKindArg::Ethernet);
        assert_eq!(cli.include_kinds[1], AdapterKindArg::Wireless);
        assert_eq!(cli.exclude_kinds.len(), 1);
        assert_eq!(cli.exclude_kinds[0], AdapterKindArg::Virtual);
    }

    #[test]
    fn parse_monitor_options() {
        let cli = Cli::parse_from_iter(["ddns-a", "--poll-interval", "120", "--poll-only"]);

        assert_eq!(cli.poll_interval, Some(120));
        assert!(cli.poll_only);
    }

    #[test]
    fn parse_retry_options() {
        let cli = Cli::parse_from_iter(["ddns-a", "--retry-max", "5", "--retry-delay", "10"]);

        assert_eq!(cli.retry_max, Some(5));
        assert_eq!(cli.retry_delay, Some(10));
    }

    #[test]
    fn parse_misc_options() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--config",
            "/path/to/config.toml",
            "--dry-run",
            "--verbose",
        ]);

        assert_eq!(
            cli.config.as_ref().unwrap().to_str(),
            Some("/path/to/config.toml")
        );
        assert!(cli.dry_run);
        assert!(cli.verbose);
    }

    #[test]
    fn default_values() {
        let cli = Cli::parse_from_iter(["ddns-a"]);

        // Optional fields have no defaults in CLI - None when not specified
        assert!(cli.method.is_none());
        assert!(cli.poll_interval.is_none());
        assert!(cli.retry_max.is_none());
        assert!(cli.retry_delay.is_none());
        // Boolean flags default to false
        assert!(!cli.poll_only);
        assert!(!cli.dry_run);
        assert!(!cli.verbose);
        // Vec fields default to empty
        assert!(cli.include_kinds.is_empty());
        assert!(cli.exclude_kinds.is_empty());
    }
}

mod init_command {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_init_with_default_output() {
        let cli = Cli::parse_from_iter(["ddns-a", "init"]);

        assert!(cli.is_init());
        match cli.command {
            Some(Command::Init { output }) => {
                assert_eq!(output, PathBuf::from("ddns-a.toml"));
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn parse_init_with_custom_output() {
        let cli = Cli::parse_from_iter(["ddns-a", "init", "--output", "/custom/path/config.toml"]);

        assert!(cli.is_init());
        match cli.command {
            Some(Command::Init { output }) => {
                assert_eq!(output, PathBuf::from("/custom/path/config.toml"));
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn is_init_false_for_run_mode() {
        let cli = Cli::parse_from_iter(["ddns-a", "--url", "https://example.com"]);

        assert!(!cli.is_init());
    }
}

mod adapter_kind_arg {
    use super::*;
    use crate::network::AdapterKind;
    use clap::ValueEnum;

    #[test]
    fn parse_ethernet() {
        let kind = AdapterKindArg::from_str("ethernet", false).unwrap();
        assert_eq!(kind, AdapterKindArg::Ethernet);
    }

    #[test]
    fn parse_wireless() {
        let kind = AdapterKindArg::from_str("wireless", false).unwrap();
        assert_eq!(kind, AdapterKindArg::Wireless);
    }

    #[test]
    fn parse_virtual() {
        let kind = AdapterKindArg::from_str("virtual", false).unwrap();
        assert_eq!(kind, AdapterKindArg::Virtual);
    }

    #[test]
    fn parse_loopback() {
        let kind = AdapterKindArg::from_str("loopback", false).unwrap();
        assert_eq!(kind, AdapterKindArg::Loopback);
    }

    #[test]
    fn parse_invalid_returns_error() {
        let result = AdapterKindArg::from_str("unknown", false);
        assert!(result.is_err());
    }

    #[test]
    fn from_adapter_kind_arg_ethernet() {
        let kind: AdapterKind = AdapterKindArg::Ethernet.into();
        assert_eq!(kind, AdapterKind::Ethernet);
    }

    #[test]
    fn from_adapter_kind_arg_wireless() {
        let kind: AdapterKind = AdapterKindArg::Wireless.into();
        assert_eq!(kind, AdapterKind::Wireless);
    }

    #[test]
    fn from_adapter_kind_arg_virtual() {
        let kind: AdapterKind = AdapterKindArg::Virtual.into();
        assert_eq!(kind, AdapterKind::Virtual);
    }

    #[test]
    fn from_adapter_kind_arg_loopback() {
        let kind: AdapterKind = AdapterKindArg::Loopback.into();
        assert_eq!(kind, AdapterKind::Loopback);
    }

    #[test]
    fn debug_impl_works() {
        let debug_str = format!("{:?}", AdapterKindArg::Ethernet);
        assert!(debug_str.contains("Ethernet"));
    }

    #[test]
    fn clone_works() {
        let kind = AdapterKindArg::Wireless;
        #[allow(clippy::clone_on_copy)]
        let cloned = kind.clone();
        assert_eq!(kind, cloned);
    }

    #[test]
    fn hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(AdapterKindArg::Ethernet);
        set.insert(AdapterKindArg::Wireless);
        assert!(set.contains(&AdapterKindArg::Ethernet));
        assert!(!set.contains(&AdapterKindArg::Virtual));
    }
}

mod kind_filter_cli {
    use super::*;

    #[test]
    fn include_kind_single_value() {
        let cli = Cli::parse_from_iter(["ddns-a", "--include-kind", "ethernet"]);
        assert_eq!(cli.include_kinds.len(), 1);
        assert_eq!(cli.include_kinds[0], AdapterKindArg::Ethernet);
    }

    #[test]
    fn include_kind_comma_separated() {
        let cli = Cli::parse_from_iter(["ddns-a", "--include-kind", "ethernet,wireless,loopback"]);
        assert_eq!(cli.include_kinds.len(), 3);
        assert_eq!(cli.include_kinds[0], AdapterKindArg::Ethernet);
        assert_eq!(cli.include_kinds[1], AdapterKindArg::Wireless);
        assert_eq!(cli.include_kinds[2], AdapterKindArg::Loopback);
    }

    #[test]
    fn include_kind_multiple_flags() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--include-kind",
            "ethernet",
            "--include-kind",
            "wireless",
        ]);
        assert_eq!(cli.include_kinds.len(), 2);
        assert_eq!(cli.include_kinds[0], AdapterKindArg::Ethernet);
        assert_eq!(cli.include_kinds[1], AdapterKindArg::Wireless);
    }

    #[test]
    fn exclude_kind_single_value() {
        let cli = Cli::parse_from_iter(["ddns-a", "--exclude-kind", "virtual"]);
        assert_eq!(cli.exclude_kinds.len(), 1);
        assert_eq!(cli.exclude_kinds[0], AdapterKindArg::Virtual);
    }

    #[test]
    fn exclude_kind_comma_separated() {
        let cli = Cli::parse_from_iter(["ddns-a", "--exclude-kind", "virtual,loopback"]);
        assert_eq!(cli.exclude_kinds.len(), 2);
        assert_eq!(cli.exclude_kinds[0], AdapterKindArg::Virtual);
        assert_eq!(cli.exclude_kinds[1], AdapterKindArg::Loopback);
    }

    #[test]
    fn include_and_exclude_kinds_together() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--include-kind",
            "ethernet,wireless",
            "--exclude-kind",
            "loopback",
        ]);
        assert_eq!(cli.include_kinds.len(), 2);
        assert_eq!(cli.exclude_kinds.len(), 1);
    }
}
