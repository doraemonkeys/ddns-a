//! Tests for `PollingMonitor` configuration.

use super::*;
use crate::monitor::DebouncePolicy;
use crate::network::{AdapterSnapshot, AddressFetcher, FetchError};
use crate::time::Clock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// Mock clock for testing that returns controlled time values.
struct MockClock {
    secs: AtomicU64,
}

impl MockClock {
    fn new(initial_secs: u64) -> Self {
        Self {
            secs: AtomicU64::new(initial_secs),
        }
    }
}

impl Clock for MockClock {
    fn now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.secs.load(Ordering::SeqCst))
    }
}

/// Mock fetcher that returns predefined snapshots.
struct MockFetcher {
    results: Mutex<VecDeque<Result<Vec<AdapterSnapshot>, FetchError>>>,
}

impl MockFetcher {
    fn returning_snapshots(snapshots: Vec<Vec<AdapterSnapshot>>) -> Self {
        Self {
            results: Mutex::new(snapshots.into_iter().map(Ok).collect()),
        }
    }
}

impl AddressFetcher for MockFetcher {
    fn fetch(&self) -> Result<Vec<AdapterSnapshot>, FetchError> {
        self.results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(vec![]))
    }
}

#[test]
fn new_creates_with_system_clock() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let monitor = PollingMonitor::new(fetcher, Duration::from_secs(60));

    assert_eq!(monitor.interval(), Duration::from_secs(60));
    assert!(monitor.debounce().is_none());
}

#[test]
fn with_clock_allows_custom_clock() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let clock = MockClock::new(1000);
    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_secs(30));

    assert_eq!(monitor.interval(), Duration::from_secs(30));
}

#[test]
fn with_debounce_sets_policy() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let policy = DebouncePolicy::new(Duration::from_millis(500));
    let monitor =
        PollingMonitor::new(fetcher, Duration::from_secs(60)).with_debounce(policy.clone());

    assert_eq!(monitor.debounce(), Some(&policy));
}

#[test]
fn interval_accessor() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let monitor = PollingMonitor::new(fetcher, Duration::from_secs(120));

    assert_eq!(monitor.interval(), Duration::from_secs(120));
}
