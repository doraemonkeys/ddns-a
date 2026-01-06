//! Polling-based IP address monitor.
//!
//! This module provides:
//! - [`PollingMonitor`]: Builder/configuration for polling-based monitoring
//! - [`PollingStream`]: Stream that yields IP change events
//! - [`merge_changes`]: Utility for computing net effect of changes

mod monitor;
mod stream;

pub use monitor::PollingMonitor;
pub use stream::PollingStream;

use super::change::{IpChange, IpChangeKind};
use std::collections::HashMap;
use std::net::IpAddr;

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
mod mod_test;
#[cfg(test)]
mod monitor_test;
#[cfg(test)]
mod stream_test;
