//! Hybrid stream implementation.
//!
//! This module provides [`HybridStream`], a stream that combines API event
//! notifications with periodic polling for IP address change detection.

use crate::monitor::DebouncePolicy;
use crate::monitor::change::{IpChange, diff};
use crate::monitor::error::ApiError;
use crate::network::{AdapterSnapshot, AddressFetcher, FetchError};
use crate::time::Clock;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Interval, interval};
use tokio_stream::Stream;

/// Internal state of the hybrid stream.
#[derive(Debug)]
enum StreamState<S> {
    /// Hybrid mode: API events + polling.
    Hybrid {
        /// The API notification stream.
        api_stream: S,
    },
    /// Polling-only mode: API has failed, using polling as sole source.
    PollingOnly,
}

/// What triggered the current poll iteration.
#[derive(Debug)]
enum PollTrigger {
    /// API notification received
    ApiEvent,
    /// API stream ended or errored - degrade
    ApiDegraded,
    /// Polling interval elapsed
    Interval,
    /// Nothing ready yet
    Pending,
}

impl PollTrigger {
    /// Returns a human-readable label for logging.
    const fn label(&self) -> &'static str {
        match self {
            Self::ApiEvent => "API event",
            Self::ApiDegraded => "API degradation",
            Self::Interval => "polling interval",
            Self::Pending => "pending",
        }
    }
}

/// A stream of IP address changes produced by hybrid monitoring.
///
/// This type is returned by [`super::HybridMonitor::into_stream`] and yields
/// batches of [`IpChange`] events whenever changes are detected.
///
/// The stream operates in two modes:
/// - **Hybrid**: Reacts to both API notifications and polling interval
/// - **Polling-only**: Falls back to polling if the API fails
///
/// Degradation from hybrid to polling-only is automatic and permanent
/// for the lifetime of this stream.
#[derive(Debug)]
pub struct HybridStream<F, S, C> {
    fetcher: F,
    clock: C,
    interval: Interval,
    debounce: Option<DebouncePolicy>,
    state: StreamState<S>,
    /// Previous snapshot for comparison.
    prev_snapshot: Option<Vec<AdapterSnapshot>>,
    /// Debounce state: `Some(start_time)` if currently debouncing.
    debounce_start: Option<tokio::time::Instant>,
    /// Snapshot taken at debounce start for final comparison.
    debounce_baseline: Option<Vec<AdapterSnapshot>>,
}

impl<F, S, C> HybridStream<F, S, C>
where
    F: AddressFetcher,
    S: Stream<Item = Result<(), ApiError>> + Unpin,
    C: Clock,
{
    pub(super) fn new(
        fetcher: F,
        api_stream: S,
        clock: C,
        poll_interval: Duration,
        debounce: Option<DebouncePolicy>,
    ) -> Self {
        Self {
            fetcher,
            clock,
            interval: interval(poll_interval),
            debounce,
            state: StreamState::Hybrid { api_stream },
            prev_snapshot: None,
            debounce_start: None,
            debounce_baseline: None,
        }
    }

    /// Returns true if currently in polling-only mode.
    #[must_use]
    pub const fn is_polling_only(&self) -> bool {
        matches!(self.state, StreamState::PollingOnly)
    }

    /// Performs a single fetch and returns changes if any.
    fn fetch_changes(&mut self) -> Result<Vec<IpChange>, FetchError> {
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
    /// `pre_fetch_snapshot` is the snapshot state BEFORE this fetch cycle,
    /// used as baseline when starting a new debounce window.
    ///
    /// `triggered_by_api`: When true, starts debounce window even if no changes
    /// detected yet. This handles Windows API timing where `NotifyIpInterfaceChange`
    /// fires before the new IP is visible in `GetAdaptersAddresses`.
    fn process_with_debounce(
        &mut self,
        raw_changes: Vec<IpChange>,
        pre_fetch_snapshot: Option<Vec<AdapterSnapshot>>,
        triggered_by_api: bool,
    ) -> Option<Vec<IpChange>> {
        let Some(debounce) = &self.debounce else {
            // No debounce configured - emit immediately if non-empty
            return if raw_changes.is_empty() {
                None
            } else {
                Some(raw_changes)
            };
        };

        let now = tokio::time::Instant::now();

        // Decide whether to start a new debounce window:
        // - Either we detected changes (normal case)
        // - Or API event fired (signal that changes are coming, even if not visible yet)
        // - But only if we have a valid baseline to compare against
        let has_valid_baseline = pre_fetch_snapshot.is_some();
        let should_start_window = self.debounce_start.is_none()
            && has_valid_baseline
            && (!raw_changes.is_empty() || triggered_by_api);

        if should_start_window {
            if triggered_by_api && raw_changes.is_empty() {
                tracing::trace!(
                    "API event triggered but no changes detected yet, starting observation window"
                );
            }
            // Start new debounce window, save baseline (state BEFORE changes)
            self.debounce_start = Some(now);
            self.debounce_baseline = pre_fetch_snapshot;
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
        // Always reset debounce_start to avoid stuck state if baseline is None
        let baseline = self.debounce_baseline.take();
        self.debounce_start = None;

        let baseline = baseline?;
        let current = self.prev_snapshot.as_ref()?;
        let timestamp = self.clock.now();

        let changes = diff(&baseline, current, timestamp);
        if changes.is_empty() {
            None
        } else {
            Some(changes)
        }
    }

    /// Transitions to polling-only mode.
    fn degrade_to_polling(&mut self) {
        self.state = StreamState::PollingOnly;
    }
}

impl<F, S, C> Stream for HybridStream<F, S, C>
where
    F: AddressFetcher + Unpin,
    S: Stream<Item = Result<(), ApiError>> + Unpin,
    C: Clock + Unpin,
{
    type Item = Vec<IpChange>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let trigger = match &mut self.state {
                StreamState::Hybrid { api_stream } => {
                    // Check API stream first (higher priority for responsiveness)
                    match Pin::new(api_stream).poll_next(cx) {
                        Poll::Ready(Some(Ok(()))) => PollTrigger::ApiEvent,
                        Poll::Ready(Some(Err(_)) | None) => {
                            // API failed or ended - will degrade
                            PollTrigger::ApiDegraded
                        }
                        Poll::Pending => {
                            // API not ready - check interval
                            if Pin::new(&mut self.interval).poll_tick(cx).is_ready() {
                                PollTrigger::Interval
                            } else {
                                PollTrigger::Pending
                            }
                        }
                    }
                }
                StreamState::PollingOnly => {
                    // Only check interval in polling-only mode
                    if Pin::new(&mut self.interval).poll_tick(cx).is_ready() {
                        PollTrigger::Interval
                    } else {
                        PollTrigger::Pending
                    }
                }
            };

            match trigger {
                PollTrigger::Pending => return Poll::Pending,
                PollTrigger::ApiDegraded => {
                    // Degrade to polling-only mode
                    self.degrade_to_polling();
                    // Continue loop to check interval
                }
                PollTrigger::ApiEvent | PollTrigger::Interval => {
                    tracing::debug!("Check triggered by {}", trigger.label());

                    // Capture snapshot BEFORE fetch (needed for debounce baseline)
                    // Only clone when we might start debouncing
                    let pre_fetch_snapshot =
                        if self.debounce.is_some() && self.debounce_start.is_none() {
                            self.prev_snapshot.clone()
                        } else {
                            None
                        };

                    // Fetch and process changes
                    let Ok(changes) = self.fetch_changes() else {
                        // Fetch error - continue waiting for next trigger
                        continue;
                    };

                    // API events start debounce even without detected changes,
                    // because Windows may notify before IP is visible
                    let triggered_by_api = matches!(trigger, PollTrigger::ApiEvent);

                    if let Some(result) =
                        self.process_with_debounce(changes, pre_fetch_snapshot, triggered_by_api)
                    {
                        tracing::debug!(
                            "Emitting {} change(s) triggered by {}",
                            result.len(),
                            trigger.label()
                        );
                        return Poll::Ready(Some(result));
                    }
                    // No changes to emit - loop back to wait for next trigger
                }
            }
        }
    }
}
