//! Polling stream implementation.
//!
//! This module provides [`PollingStream`], a stream that periodically
//! fetches network adapter snapshots and yields IP address changes.

use super::super::DebouncePolicy;
use super::super::change::{IpChange, diff};
use crate::network::{AdapterSnapshot, AddressFetcher, FetchError};
use crate::time::Clock;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Interval, interval};
use tokio_stream::Stream;

/// A stream of IP address changes produced by polling.
///
/// This type is returned by [`super::PollingMonitor::into_stream`] and yields
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
    pub(super) fn new(
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

