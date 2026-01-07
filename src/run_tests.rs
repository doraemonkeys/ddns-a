//! Tests for the run module.

use super::*;

mod run_error {
    use super::*;

    #[test]
    fn stream_terminated_displays_message() {
        let error = RunError::StreamTerminated;
        assert_eq!(error.to_string(), "Monitor stream terminated unexpectedly");
    }

    #[test]
    fn api_listener_creation_displays_source() {
        let api_error = ddns_a::monitor::ApiError::Stopped;
        let error = RunError::ApiListenerCreation(api_error);
        assert!(error.to_string().contains("Failed to create API listener"));
    }

    #[test]
    fn debug_format_works() {
        let error = RunError::StreamTerminated;
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("StreamTerminated"));
    }
}

mod runtime_options {
    use super::*;
    use ddns_a::config::{Cli, ValidatedConfig};

    fn make_test_config() -> ValidatedConfig {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--url",
            "https://example.com/hook",
            "--ip-version",
            "ipv4",
            "--poll-interval",
            "120",
            "--dry-run",
            "--poll-only",
        ]);
        ValidatedConfig::from_raw(&cli, None).unwrap()
    }

    #[test]
    fn from_config_extracts_poll_interval() {
        let config = make_test_config();
        let options = RuntimeOptions::from(&config);
        assert_eq!(options.poll_interval, std::time::Duration::from_secs(120));
    }

    #[test]
    fn from_config_extracts_poll_only() {
        let config = make_test_config();
        let options = RuntimeOptions::from(&config);
        assert!(options.poll_only);
    }

    #[test]
    fn from_config_extracts_dry_run() {
        let config = make_test_config();
        let options = RuntimeOptions::from(&config);
        assert!(options.dry_run);
    }

    #[test]
    fn defaults_when_not_specified() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--url",
            "https://example.com/hook",
            "--ip-version",
            "ipv4",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();
        let options = RuntimeOptions::from(&config);

        assert!(!options.poll_only);
        assert!(!options.dry_run);
        assert_eq!(options.poll_interval, std::time::Duration::from_secs(60));
    }

    #[test]
    fn from_config_extracts_ip_version() {
        let config = make_test_config();
        let options = RuntimeOptions::from(&config);
        assert_eq!(options.ip_version, ddns_a::network::IpVersion::V4);
    }
}

mod create_webhook {
    use super::*;
    use ddns_a::config::Cli;

    #[test]
    fn creates_webhook_with_url() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--url",
            "https://example.com/webhook",
            "--ip-version",
            "ipv4",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();
        let webhook = create_webhook(&config);

        assert_eq!(webhook.url().as_str(), "https://example.com/webhook");
    }

    #[test]
    fn creates_webhook_with_method() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--url",
            "https://example.com/webhook",
            "--ip-version",
            "ipv4",
            "--method",
            "PUT",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();
        let webhook = create_webhook(&config);

        assert_eq!(webhook.method(), http::Method::PUT);
    }

    #[test]
    fn creates_webhook_with_retry_policy() {
        let cli = Cli::parse_from_iter([
            "ddns-a",
            "--url",
            "https://example.com/webhook",
            "--ip-version",
            "ipv4",
            "--retry-max",
            "5",
            "--retry-delay",
            "10",
        ]);
        let config = ValidatedConfig::from_raw(&cli, None).unwrap();
        let webhook = create_webhook(&config);

        assert_eq!(webhook.retry_policy().max_attempts, 5);
    }
}

mod detect_startup_changes {
    use super::*;
    use ddns_a::network::{AdapterKind, AdapterSnapshot, IpVersion};
    use ddns_a::state::{LoadResult, StateError, StateStore};
    use std::net::Ipv4Addr;
    use std::time::SystemTime;

    /// Local mock for testing `detect_startup_changes`.
    struct MockStateStore {
        load_result: LoadResult,
    }

    impl MockStateStore {
        fn with_loaded(snapshots: Vec<AdapterSnapshot>) -> Self {
            Self {
                load_result: LoadResult::Loaded(snapshots),
            }
        }

        fn not_found() -> Self {
            Self {
                load_result: LoadResult::NotFound,
            }
        }

        fn corrupted(reason: impl Into<String>) -> Self {
            Self {
                load_result: LoadResult::Corrupted {
                    reason: reason.into(),
                },
            }
        }
    }

    impl StateStore for MockStateStore {
        fn load(&self) -> LoadResult {
            self.load_result.clone()
        }

        async fn save(&self, _snapshots: &[AdapterSnapshot]) -> Result<(), StateError> {
            Ok(())
        }
    }

    fn snapshot_with_ipv4(name: &str, ip: &str) -> AdapterSnapshot {
        AdapterSnapshot::new(
            name,
            AdapterKind::Ethernet,
            vec![ip.parse::<Ipv4Addr>().unwrap()],
            vec![],
        )
    }

    #[test]
    fn returns_empty_when_no_previous_state() {
        let store = MockStateStore::not_found();
        let current = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];

        let changes = detect_startup_changes_with_timestamp(
            &store,
            &current,
            IpVersion::Both,
            SystemTime::UNIX_EPOCH,
        );

        assert!(changes.is_empty());
    }

    #[test]
    fn returns_empty_when_state_corrupted() {
        let store = MockStateStore::corrupted("test corruption");
        let current = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];

        let changes = detect_startup_changes_with_timestamp(
            &store,
            &current,
            IpVersion::Both,
            SystemTime::UNIX_EPOCH,
        );

        assert!(changes.is_empty());
    }

    #[test]
    fn returns_empty_when_no_changes() {
        let snapshots = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];
        let store = MockStateStore::with_loaded(snapshots.clone());

        let changes = detect_startup_changes_with_timestamp(
            &store,
            &snapshots,
            IpVersion::Both,
            SystemTime::UNIX_EPOCH,
        );

        assert!(changes.is_empty());
    }

    #[test]
    fn detects_added_address() {
        let saved = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];
        let current = vec![
            snapshot_with_ipv4("eth0", "192.168.1.1"),
            snapshot_with_ipv4("eth1", "10.0.0.1"),
        ];
        let store = MockStateStore::with_loaded(saved);

        let changes = detect_startup_changes_with_timestamp(
            &store,
            &current,
            IpVersion::Both,
            SystemTime::UNIX_EPOCH,
        );

        assert_eq!(changes.len(), 1);
        assert!(changes[0].is_added());
        assert_eq!(changes[0].address.to_string(), "10.0.0.1");
    }

    #[test]
    fn detects_removed_address() {
        let saved = vec![
            snapshot_with_ipv4("eth0", "192.168.1.1"),
            snapshot_with_ipv4("eth1", "10.0.0.1"),
        ];
        let current = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];
        let store = MockStateStore::with_loaded(saved);

        let changes = detect_startup_changes_with_timestamp(
            &store,
            &current,
            IpVersion::Both,
            SystemTime::UNIX_EPOCH,
        );

        assert_eq!(changes.len(), 1);
        assert!(changes[0].is_removed());
        assert_eq!(changes[0].address.to_string(), "10.0.0.1");
    }

    #[test]
    fn filters_by_ip_version() {
        use std::net::Ipv6Addr;

        let saved = vec![];
        let current = vec![AdapterSnapshot::new(
            "eth0",
            AdapterKind::Ethernet,
            vec!["192.168.1.1".parse::<Ipv4Addr>().unwrap()],
            vec!["fe80::1".parse::<Ipv6Addr>().unwrap()],
        )];
        let store = MockStateStore::with_loaded(saved);

        // V4 only
        let changes = detect_startup_changes_with_timestamp(
            &store,
            &current,
            IpVersion::V4,
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(changes.len(), 1);
        assert!(changes[0].address.is_ipv4());

        // V6 only
        let store = MockStateStore::with_loaded(vec![]);
        let changes = detect_startup_changes_with_timestamp(
            &store,
            &current,
            IpVersion::V6,
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(changes.len(), 1);
        assert!(changes[0].address.is_ipv6());
    }
}

mod handle_changes {
    use super::*;
    use ddns_a::monitor::IpChange;
    use std::net::IpAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;

    /// Mock webhook sender for testing.
    struct MockWebhook {
        send_count: AtomicUsize,
        should_fail: bool,
    }

    impl MockWebhook {
        fn new() -> Self {
            Self {
                send_count: AtomicUsize::new(0),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                send_count: AtomicUsize::new(0),
                should_fail: true,
            }
        }

        fn send_count(&self) -> usize {
            self.send_count.load(Ordering::SeqCst)
        }
    }

    impl ddns_a::webhook::WebhookSender for MockWebhook {
        async fn send(&self, _changes: &[IpChange]) -> Result<(), ddns_a::webhook::WebhookError> {
            self.send_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail {
                Err(ddns_a::webhook::WebhookError::Retryable(
                    ddns_a::webhook::RetryableError::Http(ddns_a::webhook::HttpError::Timeout),
                ))
            } else {
                Ok(())
            }
        }
    }

    fn make_change() -> IpChange {
        IpChange::added(
            "eth0",
            "192.168.1.100".parse::<IpAddr>().unwrap(),
            SystemTime::UNIX_EPOCH,
        )
    }

    #[tokio::test]
    async fn sends_webhook_when_not_dry_run() {
        let webhook = MockWebhook::new();
        let changes = vec![make_change()];

        handle_changes(&changes, &webhook, false).await;

        assert_eq!(webhook.send_count(), 1);
    }

    #[tokio::test]
    async fn skips_webhook_in_dry_run() {
        let webhook = MockWebhook::new();
        let changes = vec![make_change()];

        handle_changes(&changes, &webhook, true).await;

        assert_eq!(webhook.send_count(), 0);
    }

    #[tokio::test]
    async fn handles_webhook_failure_gracefully() {
        let webhook = MockWebhook::failing();
        let changes = vec![make_change()];

        // Should not panic
        handle_changes(&changes, &webhook, false).await;

        assert_eq!(webhook.send_count(), 1);
    }

    #[tokio::test]
    async fn handles_multiple_changes() {
        let webhook = MockWebhook::new();
        let changes = vec![
            IpChange::added(
                "eth0",
                "192.168.1.1".parse().unwrap(),
                SystemTime::UNIX_EPOCH,
            ),
            IpChange::removed(
                "eth0",
                "192.168.1.2".parse().unwrap(),
                SystemTime::UNIX_EPOCH,
            ),
            IpChange::added("wlan0", "10.0.0.1".parse().unwrap(), SystemTime::UNIX_EPOCH),
        ];

        handle_changes(&changes, &webhook, false).await;

        // All changes sent in single batch
        assert_eq!(webhook.send_count(), 1);
    }
}
