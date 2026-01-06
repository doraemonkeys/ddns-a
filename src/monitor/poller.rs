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
