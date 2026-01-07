//! Monitor layer for detecting IP address changes.
//!
//! This module provides types and functions for:
//! - Representing IP change events ([`IpChange`], [`IpChangeKind`])
//! - Detecting changes between snapshots ([`diff`])
//! - Debouncing rapid changes ([`DebouncePolicy`])
//! - Error handling ([`MonitorError`], [`ApiError`])
//! - Polling-based monitoring ([`PollingMonitor`], [`PollingStream`])
//! - API-based notifications ([`ApiListener`], [`platform`])
//! - Hybrid monitoring ([`HybridMonitor`], [`HybridStream`])

mod change;
mod debounce;
mod error;
mod hybrid;
mod listener;
pub mod platform;
mod poller;

#[cfg(test)]
mod poller_tests;

pub use change::{IpChange, IpChangeKind, diff, filter_by_version};
pub use debounce::DebouncePolicy;
pub use error::{ApiError, MonitorError};
pub use hybrid::{HybridMonitor, HybridStream};
pub use listener::ApiListener;
pub use poller::{PollingMonitor, PollingStream, merge_changes};
