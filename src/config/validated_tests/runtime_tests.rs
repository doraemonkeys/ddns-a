//! Tests for runtime behavior configuration: retry policy, poll interval, flags.

use std::time::Duration;

use super::*;

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
