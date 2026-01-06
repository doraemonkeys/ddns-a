//! Polling monitor configuration.
//!
//! This module provides [`PollingMonitor`], the builder/configuration struct
//! for creating polling-based IP address monitors.

use super::super::DebouncePolicy;
use super::stream::PollingStream;
use crate::network::AddressFetcher;
use crate::time::{Clock, SystemClock};
use std::time::Duration;

/// Polling-based IP address monitor.
///
/// Periodically fetches network adapter information and emits a stream
/// of [`super::super::IpChange`] events when addresses are added or removed.
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
/// let monitor = PollingMonitor::new(fetcher, Duration::from_secs(60));
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
    /// yield batches of [`super::super::IpChange`] events whenever addresses change.
    ///
    /// The stream never terminates on its own; use `take_until` with
    /// a shutdown signal to stop it gracefully.
    #[must_use]
    pub fn into_stream(self) -> PollingStream<F, C> {
        PollingStream::new(self.fetcher, self.clock, self.interval, self.debounce)
    }
}

