//! Polling-based IP address monitor.
//!
//! This module provides [`PollingMonitor`], which periodically fetches
//! network adapter snapshots and emits changes as a stream.

use super::DebouncePolicy;
use super::change::{IpChange, IpChangeKind, diff};
use crate::network::{AdapterSnapshot, AddressFetcher, FetchError};
use crate::time::{Clock, SystemClock};
use std::collections::HashMap;
use std::net::IpAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Interval, interval};
use tokio_stream::Stream;

/// A stream of IP address changes produced by polling.
///
/// This type is returned by [`PollingMonitor::into_stream`] and yields
/// batches of [`IpChange`] events whenever changes are detected.
pub struct PollingStream<F, C> {
    fetcher: F,
    clock: C,
    interval: Interval,
    debounce: Option<DebouncePolicy>,
    /// Previous snapshot for comparison
    prev_snapshot: Option<Vec<AdapterSnapshot>>,
    /// Debounce state: `Some(start_time)` if currently debouncing
    debounce_start: Option<tokio::time::Instant>,
    /// Snapshot taken at debounce start for final comparison
    debounce_baseline: Option<Vec<AdapterSnapshot>>,
}

impl<F, C> PollingStream<F, C>
where
    F: AddressFetcher,
    C: Clock,
{
    fn new(
        fetcher: F,
        clock: C,
        poll_interval: Duration,
        debounce: Option<DebouncePolicy>,
    ) -> Self {
        Self {
            fetcher,
            clock,
            interval: interval(poll_interval),
            debounce,
            prev_snapshot: None,
            debounce_start: None,
            debounce_baseline: None,
        }
    }

    /// Performs a single poll and returns changes if any.
    fn poll_once(&mut self) -> Result<Vec<IpChange>, FetchError> {
        let current = self.fetcher.fetch()?;
        let timestamp = self.clock.now();

        let changes = self
            .prev_snapshot
            .as_ref()
            .map_or_else(Vec::new, |prev| diff(prev, &current, timestamp));

        self.prev_snapshot = Some(current);
        Ok(changes)
    }

    /// Handles debounce logic, returning changes to emit (if any).
    ///
    /// `pre_poll_snapshot` is the snapshot state BEFORE this poll cycle,
    /// used as baseline when starting a new debounce window.
    fn process_with_debounce(
        &mut self,
        raw_changes: Vec<IpChange>,
        pre_poll_snapshot: Option<Vec<AdapterSnapshot>>,
    ) -> Option<Vec<IpChange>> {
        let Some(debounce) = &self.debounce else {
            // No debounce configured - emit immediately if non-empty
            return if raw_changes.is_empty() {
                None
            } else {
                Some(raw_changes)
            };
        };

        if raw_changes.is_empty() && self.debounce_start.is_none() {
            // No changes and not debouncing - nothing to do
            return None;
        }

        let now = tokio::time::Instant::now();

        if !raw_changes.is_empty() && self.debounce_start.is_none() {
            // Start new debounce window, save baseline (state BEFORE changes)
            self.debounce_start = Some(now);
            self.debounce_baseline = pre_poll_snapshot;
        }

        // Check if debounce window has elapsed
        if let Some(start) = self.debounce_start {
            if now.duration_since(start) >= debounce.window() {
                // Window expired - compute final changes from baseline
                return self.finalize_debounce();
            }
        }

        None
    }

    /// Finalizes debounce by computing net changes from baseline to current state.
    fn finalize_debounce(&mut self) -> Option<Vec<IpChange>> {
        let baseline = self.debounce_baseline.take()?;
        self.debounce_start = None;

        let current = self.prev_snapshot.as_ref()?;
        let timestamp = self.clock.now();

        let changes = diff(&baseline, current, timestamp);
        if changes.is_empty() {
            None
        } else {
            Some(changes)
        }
    }
}

impl<F, C> Stream for PollingStream<F, C>
where
    F: AddressFetcher + Unpin,
    C: Clock + Unpin,
{
    type Item = Vec<IpChange>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Poll the interval timer - registers waker for next tick when Pending
            if Pin::new(&mut self.interval).poll_tick(cx).is_pending() {
                return Poll::Pending;
            }

            // Capture snapshot BEFORE poll_once updates it (needed for debounce baseline)
            // Only clone when we might start debouncing (entering debounce mode)
            let pre_poll_snapshot = if self.debounce.is_some() && self.debounce_start.is_none() {
                self.prev_snapshot.clone()
            } else {
                None
            };

            // Interval ticked - perform a poll
            // Fetch errors are intentionally swallowed for resilient polling:
            // transient network/system errors should not terminate the stream.
            let Ok(changes) = self.poll_once() else {
                // Error occurred - loop back to re-register waker via poll_tick
                continue;
            };

            if let Some(result) = self.process_with_debounce(changes, pre_poll_snapshot) {
                return Poll::Ready(Some(result));
            }
            // No changes to emit - loop back to re-register waker via poll_tick
        }
    }
}

/// Polling-based IP address monitor.
///
/// Periodically fetches network adapter information and emits a stream
/// of [`IpChange`] events when addresses are added or removed.
///
/// # Type Parameters
///
/// * `F` - The [`AddressFetcher`] implementation for retrieving adapter snapshots
/// * `C` - The [`Clock`] implementation for timestamps (defaults to [`SystemClock`])
///
/// # Example
///
/// ```ignore
/// use ddns_a::monitor::PollingMonitor;
/// use ddns_a::time::SystemClock;
/// use std::time::Duration;
///
/// let fetcher = MyFetcher::new();
/// let monitor = PollingMonitor::new(fetcher, SystemClock, Duration::from_secs(60));
///
/// let mut stream = monitor.into_stream();
/// while let Some(changes) = stream.next().await {
///     for change in changes {
///         println!("{:?}", change);
///     }
/// }
/// ```
pub struct PollingMonitor<F, C = SystemClock> {
    fetcher: F,
    clock: C,
    interval: Duration,
    debounce: Option<DebouncePolicy>,
}

impl<F> PollingMonitor<F, SystemClock>
where
    F: AddressFetcher,
{
    /// Creates a new polling monitor with system clock.
    ///
    /// # Arguments
    ///
    /// * `fetcher` - The address fetcher to use for polling
    /// * `interval` - The interval between polls
    #[must_use]
    pub const fn new(fetcher: F, interval: Duration) -> Self {
        Self::with_clock(fetcher, SystemClock, interval)
    }
}

impl<F, C> PollingMonitor<F, C>
where
    F: AddressFetcher,
    C: Clock,
{
    /// Creates a new polling monitor with a custom clock.
    ///
    /// This constructor allows injecting a mock clock for testing.
    ///
    /// # Arguments
    ///
    /// * `fetcher` - The address fetcher to use for polling
    /// * `clock` - The clock to use for timestamps
    /// * `interval` - The interval between polls
    #[must_use]
    pub const fn with_clock(fetcher: F, clock: C, interval: Duration) -> Self {
        Self {
            fetcher,
            clock,
            interval,
            debounce: None,
        }
    }

    /// Configures debounce policy for this monitor.
    ///
    /// When debounce is enabled, rapid consecutive changes within the
    /// debounce window are merged, with cancelling changes (add then remove
    /// of the same IP) being eliminated.
    ///
    /// # Arguments
    ///
    /// * `policy` - The debounce policy to apply
    #[must_use]
    pub const fn with_debounce(mut self, policy: DebouncePolicy) -> Self {
        self.debounce = Some(policy);
        self
    }

    /// Returns the configured polling interval.
    #[must_use]
    pub const fn interval(&self) -> Duration {
        self.interval
    }

    /// Returns the configured debounce policy, if any.
    #[must_use]
    pub const fn debounce(&self) -> Option<&DebouncePolicy> {
        self.debounce.as_ref()
    }

    /// Converts this monitor into a stream of IP changes.
    ///
    /// The returned stream will poll at the configured interval and
    /// yield batches of [`IpChange`] events whenever addresses change.
    ///
    /// The stream never terminates on its own; use `take_until` with
    /// a shutdown signal to stop it gracefully.
    #[must_use]
    pub fn into_stream(self) -> PollingStream<F, C> {
        PollingStream::new(self.fetcher, self.clock, self.interval, self.debounce)
    }
}

/// Merges IP changes by computing net effect per (adapter, address).
///
/// This utility is provided for external consumers who accumulate raw change
/// events and need to compute net effects (e.g., for batched notifications).
/// The internal debounce implementation uses baseline-diff comparison instead.
///
/// # Merge Semantics
///
/// - `Added` + `Removed` for same IP = cancelled (no output)
/// - `Removed` + `Added` for same IP = cancelled (no output)
/// - Multiple `Added` for same IP = single `Added`
/// - Multiple `Removed` for same IP = single `Removed`
///
/// # Arguments
///
/// * `changes` - The changes to merge
/// * `timestamp` - The timestamp to use for merged changes
///
/// # Returns
///
/// A vector of merged changes with net effect only.
#[must_use]
pub fn merge_changes(changes: &[IpChange], timestamp: std::time::SystemTime) -> Vec<IpChange> {
    // Count net changes per (adapter, address)
    let mut net_changes: HashMap<(&str, IpAddr), i32> = HashMap::new();

    for change in changes {
        let key = (change.adapter.as_str(), change.address);
        let delta = match change.kind {
            IpChangeKind::Added => 1,
            IpChangeKind::Removed => -1,
        };
        *net_changes.entry(key).or_insert(0) += delta;
    }

    // Convert net changes back to IpChange events
    let mut result = Vec::new();
    for ((adapter, address), net) in net_changes {
        match net.cmp(&0) {
            std::cmp::Ordering::Greater => {
                result.push(IpChange::added(adapter, address, timestamp));
            }
            std::cmp::Ordering::Less => {
                result.push(IpChange::removed(adapter, address, timestamp));
            }
            std::cmp::Ordering::Equal => {
                // Cancelled out - no change
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::AdapterKind;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::SystemTime;
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
        fn new(results: Vec<Result<Vec<AdapterSnapshot>, FetchError>>) -> Self {
            Self {
                results: Mutex::new(results.into()),
            }
        }

        fn returning_snapshots(snapshots: Vec<Vec<AdapterSnapshot>>) -> Self {
            Self::new(snapshots.into_iter().map(Ok).collect())
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

    fn timestamp() -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    mod merge_changes_fn {
        use super::*;

        #[test]
        fn empty_input_returns_empty() {
            let result = merge_changes(&[], timestamp());
            assert!(result.is_empty());
        }

        #[test]
        fn single_added_preserved() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let changes = vec![IpChange::added("eth0", addr, timestamp())];

            let result = merge_changes(&changes, timestamp());

            assert_eq!(result.len(), 1);
            assert!(result[0].is_added());
            assert_eq!(result[0].address, addr);
        }

        #[test]
        fn single_removed_preserved() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let changes = vec![IpChange::removed("eth0", addr, timestamp())];

            let result = merge_changes(&changes, timestamp());

            assert_eq!(result.len(), 1);
            assert!(result[0].is_removed());
        }

        #[test]
        fn added_then_removed_cancels_out() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let changes = vec![
                IpChange::added("eth0", addr, timestamp()),
                IpChange::removed("eth0", addr, timestamp()),
            ];

            let result = merge_changes(&changes, timestamp());

            assert!(result.is_empty());
        }

        #[test]
        fn removed_then_added_cancels_out() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let changes = vec![
                IpChange::removed("eth0", addr, timestamp()),
                IpChange::added("eth0", addr, timestamp()),
            ];

            let result = merge_changes(&changes, timestamp());

            assert!(result.is_empty());
        }

        #[test]
        fn multiple_adds_merge_to_single() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let changes = vec![
                IpChange::added("eth0", addr, timestamp()),
                IpChange::added("eth0", addr, timestamp()),
                IpChange::added("eth0", addr, timestamp()),
            ];

            let result = merge_changes(&changes, timestamp());

            assert_eq!(result.len(), 1);
            assert!(result[0].is_added());
        }

        #[test]
        fn multiple_removes_merge_to_single() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let changes = vec![
                IpChange::removed("eth0", addr, timestamp()),
                IpChange::removed("eth0", addr, timestamp()),
            ];

            let result = merge_changes(&changes, timestamp());

            assert_eq!(result.len(), 1);
            assert!(result[0].is_removed());
        }

        #[test]
        fn different_addresses_independent() {
            let addr1: IpAddr = "192.168.1.1".parse().unwrap();
            let addr2: IpAddr = "192.168.1.2".parse().unwrap();
            let changes = vec![
                IpChange::added("eth0", addr1, timestamp()),
                IpChange::removed("eth0", addr2, timestamp()),
            ];

            let result = merge_changes(&changes, timestamp());

            assert_eq!(result.len(), 2);
        }

        #[test]
        fn different_adapters_independent() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let changes = vec![
                IpChange::added("eth0", addr, timestamp()),
                IpChange::removed("eth1", addr, timestamp()),
            ];

            let result = merge_changes(&changes, timestamp());

            assert_eq!(result.len(), 2);
        }

        #[test]
        fn complex_sequence_with_partial_cancellation() {
            let addr1: IpAddr = "192.168.1.1".parse().unwrap();
            let addr2: IpAddr = "192.168.1.2".parse().unwrap();
            let changes = vec![
                // addr1: add, remove, add = net +1 (added)
                IpChange::added("eth0", addr1, timestamp()),
                IpChange::removed("eth0", addr1, timestamp()),
                IpChange::added("eth0", addr1, timestamp()),
                // addr2: remove, add = net 0 (cancelled)
                IpChange::removed("eth0", addr2, timestamp()),
                IpChange::added("eth0", addr2, timestamp()),
            ];

            let result = merge_changes(&changes, timestamp());

            assert_eq!(result.len(), 1);
            assert!(result[0].is_added());
            assert_eq!(result[0].address, addr1);
        }

        #[test]
        fn uses_provided_timestamp() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let old_ts = SystemTime::UNIX_EPOCH;
            let new_ts = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
            let changes = vec![IpChange::added("eth0", addr, old_ts)];

            let result = merge_changes(&changes, new_ts);

            assert_eq!(result[0].timestamp, new_ts);
        }
    }

    mod polling_monitor {
        use super::*;

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
    }

    mod polling_stream {
        use super::*;

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
    }
}
