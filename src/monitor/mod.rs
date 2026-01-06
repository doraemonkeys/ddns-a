//! Monitor layer for detecting IP address changes.
//!
//! This module provides types and functions for:
//! - Representing IP change events ([`IpChange`], [`IpChangeKind`])
//! - Detecting changes between snapshots ([`diff`])
//! - Debouncing rapid changes ([`DebouncePolicy`])
//! - Error handling ([`MonitorError`], [`ApiError`])
//! - Polling-based monitoring ([`PollingMonitor`], [`PollingStream`])

mod change;
mod debounce;
mod error;
mod poller;

pub use change::{IpChange, IpChangeKind, diff};
pub use debounce::DebouncePolicy;
pub use error::{ApiError, MonitorError};
pub use poller::{PollingMonitor, PollingStream, merge_changes};
