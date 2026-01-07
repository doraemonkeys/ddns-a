//! Tests for CLI argument parsing.

use super::cli::{Cli, Command, IpVersionArg};

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

        assert_eq!(cli.method, "PUT");
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
            "--exclude-virtual",
        ]);

        assert_eq!(cli.include_adapters.len(), 2);
        assert_eq!(cli.exclude_adapters.len(), 1);
        assert!(cli.exclude_virtual);
    }

    #[test]
    fn parse_monitor_options() {
        let cli = Cli::parse_from_iter(["ddns-a", "--poll-interval", "120", "--poll-only"]);

        assert_eq!(cli.poll_interval, 120);
        assert!(cli.poll_only);
    }

    #[test]
    fn parse_retry_options() {
        let cli = Cli::parse_from_iter(["ddns-a", "--retry-max", "5", "--retry-delay", "10"]);

        assert_eq!(cli.retry_max, 5);
        assert_eq!(cli.retry_delay, 10);
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

        assert_eq!(cli.method, "POST");
        assert_eq!(cli.poll_interval, 60);
        assert_eq!(cli.retry_max, 3);
        assert_eq!(cli.retry_delay, 5);
        assert!(!cli.poll_only);
        assert!(!cli.exclude_virtual);
        assert!(!cli.dry_run);
        assert!(!cli.verbose);
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
