//! Tests for `PollingStream` behavior.

use super::*;
use crate::monitor::{DebouncePolicy, IpChange};
use crate::network::{AdapterKind, AdapterSnapshot, AddressFetcher, FetchError};
use crate::time::Clock;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use tokio_stream::StreamExt;

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

    fn new(results: Vec<Result<Vec<AdapterSnapshot>, FetchError>>) -> Self {
        Self {
            results: Mutex::new(results.into()),
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

fn make_snapshot(name: &str, ipv4: Vec<&str>, ipv6: Vec<&str>) -> AdapterSnapshot {
    AdapterSnapshot::new(
        name,
        AdapterKind::Ethernet,
        ipv4.into_iter().map(|s| s.parse().unwrap()).collect(),
        ipv6.into_iter().map(|s| s.parse().unwrap()).collect(),
    )
}

#[tokio::test]
async fn emits_changes_when_addresses_change() {
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1], // First poll - baseline
        vec![snapshot2], // Second poll - change detected
    ]);
    let clock = MockClock::new(1000);

    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(10));
    let stream = monitor.into_stream();

    // Take first batch of changes (skip empty baseline)
    let changes: Vec<_> = stream.take(1).collect().await;

    assert_eq!(changes.len(), 1);
    let batch = &changes[0];
    assert_eq!(batch.len(), 2); // One removed, one added
}

#[tokio::test]
async fn no_emission_when_unchanged() {
    let snapshot = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);

    // Return same snapshot multiple times
    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot.clone()],
        vec![snapshot.clone()],
        vec![snapshot.clone()],
        vec![make_snapshot("eth0", vec!["192.168.1.2"], vec![])], // Finally a change
    ]);
    let clock = MockClock::new(0);

    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    // Should eventually get one batch of changes
    let changes: Vec<_> = stream.take(1).collect().await;
    assert_eq!(changes.len(), 1);
}

#[tokio::test]
async fn uses_clock_for_timestamps() {
    let snapshot1 = make_snapshot("eth0", vec![], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![vec![snapshot1], vec![snapshot2]]);
    let clock = MockClock::new(12345);

    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    let batch = &changes[0];

    // All changes should use the clock's timestamp
    let expected_time = SystemTime::UNIX_EPOCH + Duration::from_secs(12345);
    assert!(batch.iter().all(|c| c.timestamp == expected_time));
}

#[tokio::test]
async fn handles_adapter_appearing() {
    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![], // No adapters initially
        vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])],
    ]);
    let clock = MockClock::new(0);

    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    let batch = &changes[0];

    assert_eq!(batch.len(), 1);
    assert!(batch[0].is_added());
    assert_eq!(batch[0].adapter, "eth0");
}

#[tokio::test]
async fn handles_adapter_disappearing() {
    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])],
        vec![], // Adapter removed
    ]);
    let clock = MockClock::new(0);

    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    let batch = &changes[0];

    assert_eq!(batch.len(), 1);
    assert!(batch[0].is_removed());
}

#[tokio::test]
async fn handles_multiple_adapters() {
    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![
            make_snapshot("eth0", vec!["192.168.1.1"], vec![]),
            make_snapshot("eth1", vec!["10.0.0.1"], vec![]),
        ],
        vec![
            make_snapshot("eth0", vec!["192.168.1.2"], vec![]), // Changed
            make_snapshot("eth1", vec!["10.0.0.1"], vec![]),    // Unchanged
        ],
    ]);
    let clock = MockClock::new(0);

    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    let batch = &changes[0];

    // Only eth0 should have changes
    assert_eq!(batch.len(), 2); // One removed, one added
    assert!(batch.iter().all(|c| c.adapter == "eth0"));
}

#[tokio::test(start_paused = true)]
async fn continues_after_fetch_error() {
    let fetcher = MockFetcher::new(vec![
        Ok(vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])]),
        Err(FetchError::Platform {
            message: "transient error".to_string(),
        }),
        Ok(vec![make_snapshot("eth0", vec!["192.168.1.2"], vec![])]),
    ]);
    let clock = MockClock::new(0);

    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    // Should eventually get changes despite the error
    let changes: Vec<_> = stream.take(1).collect().await;
    assert!(!changes.is_empty());
}

#[tokio::test(start_paused = true)]
async fn debounce_emits_after_window_expires() {
    // Setup: baseline -> change -> wait for debounce window -> emit
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1],         // Poll 1: baseline (no prev, no emit)
        vec![snapshot2.clone()], // Poll 2: change detected, start debounce
        vec![snapshot2],         // Poll 3: unchanged, window expires
    ]);
    let clock = MockClock::new(1000);

    // Poll interval must exceed debounce window so window expires on next poll
    let debounce = DebouncePolicy::new(Duration::from_millis(50));
    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(100))
        .with_debounce(debounce);
    let stream = monitor.into_stream();

    // Should get changes after debounce window
    let changes: Vec<_> = stream.take(1).collect().await;
    assert_eq!(changes.len(), 1);
    let batch = &changes[0];
    // From [.1] -> [.2]: one removed (.1), one added (.2)
    assert_eq!(batch.len(), 2);
    assert!(batch.iter().any(IpChange::is_removed));
    assert!(batch.iter().any(IpChange::is_added));
}

#[tokio::test(start_paused = true)]
async fn debounce_cancels_add_then_remove() {
    // Setup: add IP then remove it -> net change from baseline is 0
    // Then a real change that should be emitted
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.1", "192.168.1.2"], vec![]);
    // Back to original state (net change = 0 from snapshot1)
    let snapshot3 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    // Real change after debounce
    let snapshot4 = make_snapshot("eth0", vec!["10.0.0.1"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1],         // Poll 1: baseline established
        vec![snapshot2],         // Poll 2: add .2, starts debounce with baseline=[.1]
        vec![snapshot3.clone()], // Poll 3: back to [.1], window expires, net=0
        vec![snapshot3],         // Poll 4: no change, no debounce active
        vec![snapshot4.clone()], // Poll 5: real change, starts new debounce
        vec![snapshot4],         // Poll 6: hold final state for debounce emission
    ]);
    let clock = MockClock::new(0);

    // Use 50ms debounce window - tokio time control handles timing
    let debounce = DebouncePolicy::new(Duration::from_millis(50));
    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(100))
        .with_debounce(debounce);
    let stream = monitor.into_stream();

    // First emission should be the real change (cancelled ones produce no emit)
    let changes: Vec<_> = stream.take(1).collect().await;
    assert_eq!(changes.len(), 1);
    let batch = &changes[0];
    // Should see removal of .1 and addition of 10.0.0.1
    assert!(
        batch
            .iter()
            .any(|c| c.is_removed() && c.address.to_string() == "192.168.1.1")
    );
    assert!(
        batch
            .iter()
            .any(|c| c.is_added() && c.address.to_string() == "10.0.0.1")
    );
}

#[tokio::test(start_paused = true)]
async fn debounce_no_changes_no_emit() {
    // When debounce starts but final state equals baseline -> no emit
    let snapshot = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot_changed = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);
    // Final change that actually differs from baseline
    let snapshot_final = make_snapshot("eth0", vec!["10.0.0.1"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot.clone()],       // Poll 1: baseline [.1]
        vec![snapshot_changed],       // Poll 2: change to [.2], debounce starts
        vec![snapshot.clone()],       // Poll 3: back to [.1], window expires, net=0
        vec![snapshot],               // Poll 4: no change
        vec![snapshot_final.clone()], // Poll 5: real change to [10.0.0.1]
        vec![snapshot_final],         // Poll 6: hold final state for debounce emission
    ]);
    let clock = MockClock::new(0);

    // Use 50ms debounce window - tokio time control handles timing
    let debounce = DebouncePolicy::new(Duration::from_millis(50));
    let monitor = PollingMonitor::with_clock(fetcher, clock, Duration::from_millis(100))
        .with_debounce(debounce);
    let stream = monitor.into_stream();

    // Should skip the cancelled changes and emit the real one
    let changes: Vec<_> = stream.take(1).collect().await;
    assert_eq!(changes.len(), 1);
    assert!(
        changes[0]
            .iter()
            .any(|c| c.address.to_string() == "10.0.0.1")
    );
}
