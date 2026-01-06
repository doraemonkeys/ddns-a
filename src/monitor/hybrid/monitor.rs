//! Hybrid monitor configuration.
//!
//! This module provides [`HybridMonitor`], the builder/configuration struct
//! for creating hybrid IP address monitors that combine API events with polling.

use super::super::DebouncePolicy;
use super::super::listener::ApiListener;
use super::stream::HybridStream;
use crate::network::AddressFetcher;
use crate::time::{Clock, SystemClock};
use std::time::Duration;

/// Hybrid IP address monitor combining API events with polling fallback.
///
/// The hybrid monitor uses platform API events (e.g., `NotifyIpInterfaceChange`)
/// for immediate notification of IP changes, with periodic polling as a safety net.
///
/// # Degradation Behavior
///
/// If the API listener fails (returns an error), the monitor automatically
/// degrades to polling-only mode. This degradation is permanent for the
/// lifetime of the stream - no automatic recovery is attempted.
///
/// # Type Parameters
///
/// * `F` - The [`AddressFetcher`] implementation for retrieving adapter snapshots
/// * `L` - The [`ApiListener`] implementation for platform event notifications
/// * `C` - The [`Clock`] implementation for timestamps (defaults to [`SystemClock`])
///
/// # Example
///
/// ```ignore
/// use ddns_a::monitor::{HybridMonitor, DebouncePolicy};
/// use ddns_a::monitor::platform::PlatformListener;
/// use std::time::Duration;
///
/// let fetcher = MyFetcher::new();
/// let listener = PlatformListener::new()?;
/// let monitor = HybridMonitor::new(fetcher, listener, Duration::from_secs(60))
///     .with_debounce(DebouncePolicy::default());
///
/// let mut stream = monitor.into_stream();
/// while let Some(changes) = stream.next().await {
///     for change in changes {
///         println!("{:?}", change);
///     }
/// }
/// ```
#[derive(Debug)]
pub struct HybridMonitor<F, L, C = SystemClock> {
    fetcher: F,
    api_listener: L,
    clock: C,
    poll_interval: Duration,
    debounce: Option<DebouncePolicy>,
}

impl<F, L> HybridMonitor<F, L, SystemClock>
where
    F: AddressFetcher,
    L: ApiListener,
{
    /// Creates a new hybrid monitor with system clock.
    ///
    /// # Arguments
    ///
    /// * `fetcher` - The address fetcher to use for polling
    /// * `api_listener` - The platform API listener for event notifications
    /// * `poll_interval` - The interval between polls (safety net for missed events)
    #[must_use]
    pub const fn new(fetcher: F, api_listener: L, poll_interval: Duration) -> Self {
        Self::with_clock(fetcher, api_listener, SystemClock, poll_interval)
    }
}

impl<F, L, C> HybridMonitor<F, L, C>
where
    F: AddressFetcher,
    L: ApiListener,
    C: Clock,
{
    /// Creates a new hybrid monitor with a custom clock.
    ///
    /// This constructor allows injecting a mock clock for testing.
    ///
    /// # Arguments
    ///
    /// * `fetcher` - The address fetcher to use for polling
    /// * `api_listener` - The platform API listener for event notifications
    /// * `clock` - The clock to use for timestamps
    /// * `poll_interval` - The interval between polls
    #[must_use]
    pub const fn with_clock(
        fetcher: F,
        api_listener: L,
        clock: C,
        poll_interval: Duration,
    ) -> Self {
        Self {
            fetcher,
            api_listener,
            clock,
            poll_interval,
            debounce: None,
        }
    }

    /// Configures debounce policy for this monitor.
    ///
    /// When debounce is enabled, rapid consecutive changes within the
    /// debounce window are merged, with cancelling changes (add then remove
    /// of the same IP) being eliminated.
    ///
    /// **Note**: The debounce window is fixed-duration from the first change;
    /// subsequent changes within the window do not extend the timer.
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
    pub const fn poll_interval(&self) -> Duration {
        self.poll_interval
    }

    /// Returns the configured debounce policy, if any.
    #[must_use]
    pub const fn debounce(&self) -> Option<&DebouncePolicy> {
        self.debounce.as_ref()
    }

    /// Converts this monitor into a stream of IP changes.
    ///
    /// The returned stream will:
    /// - React to API events for immediate change detection
    /// - Poll at the configured interval as a safety net
    /// - Yield batches of [`crate::monitor::IpChange`] events whenever addresses change
    ///
    /// If the API listener fails, the stream automatically degrades to
    /// polling-only mode without terminating.
    ///
    /// The stream never terminates on its own; use `take_until` with
    /// a shutdown signal to stop it gracefully.
    #[must_use]
    pub fn into_stream(self) -> HybridStream<F, L::Stream, C> {
        let api_stream = self.api_listener.into_stream();
        HybridStream::new(
            self.fetcher,
            api_stream,
            self.clock,
            self.poll_interval,
            self.debounce,
        )
    }
}
