//! Monitor layer for detecting IP address changes.
//!
//! This module provides types and functions for:
//! - Representing IP change events ([`IpChange`], [`IpChangeKind`])
//! - Detecting changes between snapshots ([`diff`])
//! - Debouncing rapid changes ([`DebouncePolicy`])
//! - Error handling ([`MonitorError`], [`ApiError`])
//! - Polling-based monitoring ([`PollingMonitor`], [`PollingStream`])
//! - API-based notifications ([`ApiListener`], [`platform`])

mod change;
mod debounce;
mod error;
mod listener;
pub mod platform;
mod poller;

#[cfg(test)]
mod poller_test;

pub use change::{IpChange, IpChangeKind, diff};
pub use debounce::DebouncePolicy;
pub use error::{ApiError, MonitorError};
pub use listener::ApiListener;
pub use poller::{PollingMonitor, PollingStream, merge_changes};
