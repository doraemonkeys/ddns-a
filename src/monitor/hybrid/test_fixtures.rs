//! Shared test fixtures for hybrid monitor tests.

use crate::monitor::ApiError;
use crate::monitor::listener::ApiListener;
use crate::network::{AdapterKind, AdapterSnapshot, AddressFetcher, FetchError};
use crate::time::Clock;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};
use tokio_stream::Stream;

/// Mock clock for testing that returns controlled time values.
pub struct MockClock {
    secs: AtomicU64,
}

impl MockClock {
    pub fn new(initial_secs: u64) -> Self {
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
pub struct MockFetcher {
    results: Mutex<VecDeque<Result<Vec<AdapterSnapshot>, FetchError>>>,
}

impl MockFetcher {
    pub fn returning_snapshots(snapshots: Vec<Vec<AdapterSnapshot>>) -> Self {
        Self {
            results: Mutex::new(snapshots.into_iter().map(Ok).collect()),
        }
    }

    pub fn new(results: Vec<Result<Vec<AdapterSnapshot>, FetchError>>) -> Self {
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

/// Mock API stream with controllable behavior for testing.
///
/// This implementation is designed to work correctly with tokio's polling model:
/// - `Pending` results return `Poll::Pending` (requires waker registration)
/// - `Ready` results return `Poll::Ready` immediately
/// - Empty queue returns `Poll::Pending` (stream stays open)
/// - `None` in queue explicitly terminates the stream
pub struct MockApiStream {
    /// Queue of events to return. None means stream ends.
    events: Mutex<VecDeque<Option<Result<(), ApiError>>>>,
}

impl MockApiStream {
    pub fn new(events: Vec<Option<Result<(), ApiError>>>) -> Self {
        Self {
            events: Mutex::new(events.into()),
        }
    }
}

impl Stream for MockApiStream {
    type Item = Result<(), ApiError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Note: We don't register wakers - tests rely on interval timer for wakeups.
        // This is intentional for test simplicity and works because HybridStream
        // always has polling as a fallback trigger.
        let mut events = self.events.lock().unwrap();
        match events.pop_front() {
            Some(Some(result)) => Poll::Ready(Some(result)),
            Some(None) => Poll::Ready(None), // Stream ended
            None => Poll::Pending,           // No more events, stay pending
        }
    }
}

/// Mock API listener that produces a controllable stream.
pub struct MockApiListener {
    events: Vec<Option<Result<(), ApiError>>>,
}

impl MockApiListener {
    /// Create listener with specific events. Use None to signal stream end.
    pub fn new(events: Vec<Option<Result<(), ApiError>>>) -> Self {
        Self { events }
    }

    /// Listener that immediately degrades (API error).
    pub fn failing() -> Self {
        Self {
            events: vec![Some(Err(ApiError::Stopped))],
        }
    }

    /// Listener that stays pending (for polling-only tests).
    pub fn pending() -> Self {
        Self { events: vec![] }
    }
}

impl ApiListener for MockApiListener {
    type Stream = MockApiStream;

    fn into_stream(self) -> Self::Stream {
        MockApiStream::new(self.events)
    }
}

/// Helper function to create test adapter snapshots.
pub fn make_snapshot(name: &str, ipv4: Vec<&str>, ipv6: Vec<&str>) -> AdapterSnapshot {
    AdapterSnapshot::new(
        name,
        AdapterKind::Ethernet,
        ipv4.into_iter().map(|s| s.parse().unwrap()).collect(),
        ipv6.into_iter().map(|s| s.parse().unwrap()).collect(),
    )
}
