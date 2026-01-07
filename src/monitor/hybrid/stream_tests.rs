//! Tests for `HybridStream` behavior.

use super::test_fixtures::{MockApiListener, MockClock, MockFetcher, make_snapshot};
use super::*;
use crate::monitor::{DebouncePolicy, IpChange};
use crate::network::FetchError;
use std::time::{Duration, SystemTime};
use tokio_stream::StreamExt;

#[tokio::test(start_paused = true)]
async fn api_event_triggers_fetch() {
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1], // First fetch - baseline
        vec![snapshot2], // Second fetch - triggered by API event
    ]);
    let clock = MockClock::new(1000);
    // API event available immediately
    let listener = MockApiListener::new(vec![Some(Ok(())), Some(Ok(()))]);

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_secs(60));
    let stream = monitor.into_stream();

    // Should get changes from API-triggered fetch
    let changes: Vec<_> = stream.take(1).collect().await;

    assert_eq!(changes.len(), 1);
    let batch = &changes[0];
    assert_eq!(batch.len(), 2); // One removed, one added
}

#[tokio::test(start_paused = true)]
async fn polling_works_when_api_pending() {
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1], // First poll - baseline
        vec![snapshot2], // Second poll - change detected
    ]);
    let clock = MockClock::new(1000);
    // API never fires (stays pending)
    let listener = MockApiListener::pending();

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(10));
    let stream = monitor.into_stream();

    // Should get changes from polling
    let changes: Vec<_> = stream.take(1).collect().await;

    assert_eq!(changes.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn degrades_to_polling_on_api_error() {
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1], // Baseline
        vec![snapshot2], // Change after degradation
    ]);
    let clock = MockClock::new(0);
    // API fails immediately
    let listener = MockApiListener::failing();

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(10));
    let mut stream = monitor.into_stream();

    // Should get changes from polling after degradation
    let changes: Vec<_> = (&mut stream).take(1).collect().await;
    assert_eq!(changes.len(), 1);

    // Verify stream is now in polling-only mode
    assert!(stream.is_polling_only());
}

#[tokio::test(start_paused = true)]
async fn degrades_on_api_stream_end() {
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1],
        vec![snapshot2], // Change after degradation
    ]);
    let clock = MockClock::new(0);
    // API stream ends (returns None)
    let listener = MockApiListener::new(vec![None]);

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(10));
    let mut stream = monitor.into_stream();

    let changes: Vec<_> = (&mut stream).take(1).collect().await;
    assert_eq!(changes.len(), 1);

    assert!(stream.is_polling_only());
}

#[tokio::test(start_paused = true)]
async fn uses_clock_for_timestamps() {
    let snapshot1 = make_snapshot("eth0", vec![], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![vec![snapshot1], vec![snapshot2]]);
    let clock = MockClock::new(99999);
    let listener = MockApiListener::new(vec![Some(Ok(()))]);

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(10));
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    let batch = &changes[0];

    // All changes should use the clock's timestamp
    let expected_time = SystemTime::UNIX_EPOCH + Duration::from_secs(99999);
    assert!(batch.iter().all(|c| c.timestamp == expected_time));
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
    let listener = MockApiListener::pending();

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    // Should eventually get changes despite the error
    let changes: Vec<_> = stream.take(1).collect().await;
    assert!(!changes.is_empty());
}

#[tokio::test(start_paused = true)]
async fn no_emission_when_unchanged() {
    let snapshot = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot.clone()],
        vec![snapshot.clone()],
        vec![snapshot.clone()],
        vec![make_snapshot("eth0", vec!["192.168.1.2"], vec![])], // Finally a change
    ]);
    let clock = MockClock::new(0);
    let listener = MockApiListener::pending();

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    // Should eventually get one batch of changes
    let changes: Vec<_> = stream.take(1).collect().await;
    assert_eq!(changes.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn debounce_emits_after_window_expires() {
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1],         // Baseline
        vec![snapshot2.clone()], // Change detected, start debounce
        vec![snapshot2],         // Unchanged, window expires
    ]);
    let clock = MockClock::new(1000);
    let listener = MockApiListener::pending();

    // Poll interval must exceed debounce window
    let debounce = DebouncePolicy::new(Duration::from_millis(50));
    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(100))
        .with_debounce(debounce);
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    assert_eq!(changes.len(), 1);
    let batch = &changes[0];
    assert_eq!(batch.len(), 2);
    assert!(batch.iter().any(IpChange::is_removed));
    assert!(batch.iter().any(IpChange::is_added));
}

#[tokio::test(start_paused = true)]
async fn debounce_cancels_flapping() {
    // Add IP then remove it -> net change from baseline is 0
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.1", "192.168.1.2"], vec![]);
    // Back to original state
    let snapshot3 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    // Real change
    let snapshot4 = make_snapshot("eth0", vec!["10.0.0.1"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1],         // Baseline
        vec![snapshot2],         // Add .2, starts debounce
        vec![snapshot3.clone()], // Back to [.1], window expires, net=0
        vec![snapshot3],         // No change
        vec![snapshot4.clone()], // Real change
        vec![snapshot4],         // Hold for debounce emission
    ]);
    let clock = MockClock::new(0);
    let listener = MockApiListener::pending();

    let debounce = DebouncePolicy::new(Duration::from_millis(50));
    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(100))
        .with_debounce(debounce);
    let stream = monitor.into_stream();

    // First emission should be the real change
    let changes: Vec<_> = stream.take(1).collect().await;
    assert_eq!(changes.len(), 1);
    let batch = &changes[0];
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
async fn is_polling_only_initially_false() {
    let fetcher = MockFetcher::returning_snapshots(vec![]);
    let listener = MockApiListener::pending();
    let clock = MockClock::new(0);

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_secs(60));
    let stream = monitor.into_stream();

    assert!(!stream.is_polling_only());
}

#[tokio::test(start_paused = true)]
async fn handles_adapter_appearing() {
    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![], // No adapters initially
        vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])],
    ]);
    let clock = MockClock::new(0);
    let listener = MockApiListener::pending();

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    let batch = &changes[0];

    assert_eq!(batch.len(), 1);
    assert!(batch[0].is_added());
    assert_eq!(batch[0].adapter, "eth0");
}

#[tokio::test(start_paused = true)]
async fn handles_adapter_disappearing() {
    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])],
        vec![], // Adapter removed
    ]);
    let clock = MockClock::new(0);
    let listener = MockApiListener::pending();

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    let batch = &changes[0];

    assert_eq!(batch.len(), 1);
    assert!(batch[0].is_removed());
}

#[tokio::test(start_paused = true)]
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
    let listener = MockApiListener::pending();

    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(5));
    let stream = monitor.into_stream();

    let changes: Vec<_> = stream.take(1).collect().await;
    let batch = &changes[0];

    // Only eth0 should have changes
    assert_eq!(batch.len(), 2);
    assert!(batch.iter().all(|c| c.adapter == "eth0"));
}

#[tokio::test(start_paused = true)]
async fn debounce_handles_rapid_api_events() {
    // Multiple API events within debounce window should result in single emission
    // after window expires.
    let snapshot1 = make_snapshot("eth0", vec!["192.168.1.1"], vec![]);
    let snapshot2 = make_snapshot("eth0", vec!["192.168.1.2"], vec![]);
    let snapshot3 = make_snapshot("eth0", vec!["192.168.1.3"], vec![]);

    let fetcher = MockFetcher::returning_snapshots(vec![
        vec![snapshot1],         // Baseline
        vec![snapshot2],         // First API event triggers fetch, change detected
        vec![snapshot3.clone()], // Second API event triggers fetch within debounce window
        vec![snapshot3.clone()], // Third API event triggers fetch
        vec![snapshot3],         // Poll after debounce window expires
    ]);
    let clock = MockClock::new(1000);
    // Multiple API events fire rapidly
    let listener = MockApiListener::new(vec![Some(Ok(())), Some(Ok(())), Some(Ok(()))]);

    let debounce = DebouncePolicy::new(Duration::from_millis(50));
    let monitor = HybridMonitor::with_clock(fetcher, listener, clock, Duration::from_millis(100))
        .with_debounce(debounce);
    let stream = monitor.into_stream();

    // Should get single merged emission after debounce window expires
    let changes: Vec<_> = stream.take(1).collect().await;
    assert_eq!(changes.len(), 1);
    let batch = &changes[0];

    // Net effect: removed .1, added .3 (intermediate .2 is cancelled out)
    assert_eq!(batch.len(), 2);
    assert!(
        batch
            .iter()
            .any(|c| c.is_removed() && c.address.to_string() == "192.168.1.1")
    );
    assert!(
        batch
            .iter()
            .any(|c| c.is_added() && c.address.to_string() == "192.168.1.3")
    );
}
