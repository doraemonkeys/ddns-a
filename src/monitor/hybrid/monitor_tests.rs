//! Tests for `HybridMonitor` configuration.

use super::test_fixtures::{MockApiListener, MockClock, MockFetcher};
use super::*;
use crate::monitor::DebouncePolicy;
use std::time::Duration;

#[test]
fn new_creates_with_system_clock() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let listener = MockApiListener::pending();
    let monitor = HybridMonitor::new(fetcher, listener, Duration::from_secs(60));

    assert_eq!(monitor.poll_interval(), Duration::from_secs(60));
    assert!(monitor.debounce().is_none());
}

#[test]
fn with_clock_allows_custom_clock() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let listener = MockApiListener::pending();
    let clock = MockClock::new(1000);
    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_secs(30));

    assert_eq!(monitor.poll_interval(), Duration::from_secs(30));
}

#[test]
fn with_debounce_sets_policy() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let listener = MockApiListener::pending();
    let policy = DebouncePolicy::new(Duration::from_millis(500));
    let monitor = HybridMonitor::new(fetcher, listener, Duration::from_secs(60))
        .with_debounce(policy.clone());

    assert_eq!(monitor.debounce(), Some(&policy));
}

#[test]
fn poll_interval_accessor() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let listener = MockApiListener::pending();
    let monitor = HybridMonitor::new(fetcher, listener, Duration::from_secs(120));

    assert_eq!(monitor.poll_interval(), Duration::from_secs(120));
}

#[tokio::test]
async fn into_stream_consumes_listener() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let listener = MockApiListener::new(vec![Some(Ok(())), Some(Ok(()))]);
    let monitor = HybridMonitor::new(fetcher, listener, Duration::from_secs(60));

    // This consumes the monitor and its listener
    let _stream = monitor.into_stream();
    // monitor no longer accessible - demonstrates one-time semantics
}
