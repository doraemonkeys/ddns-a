//! IP change detection types and functions.

use crate::network::{AdapterSnapshot, IpVersion};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::SystemTime;

/// The kind of IP address change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpChangeKind {
    /// An IP address was added to an adapter.
    Added,
    /// An IP address was removed from an adapter.
    Removed,
}

/// An IP address change event.
///
/// Represents a single IP address being added or removed from a network adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpChange {
    /// The name of the adapter where the change occurred.
    pub adapter: String,
    /// The IP address that was added or removed.
    pub address: IpAddr,
    /// The timestamp when the change was detected.
    pub timestamp: SystemTime,
    /// Whether the address was added or removed.
    pub kind: IpChangeKind,
}

impl IpChange {
    /// Creates a new IP change event.
    #[must_use]
    pub fn new(
        adapter: impl Into<String>,
        address: IpAddr,
        timestamp: SystemTime,
        kind: IpChangeKind,
    ) -> Self {
        Self {
            adapter: adapter.into(),
            address,
            timestamp,
            kind,
        }
    }

    /// Creates an "added" change event.
    #[must_use]
    pub fn added(adapter: impl Into<String>, address: IpAddr, timestamp: SystemTime) -> Self {
        Self::new(adapter, address, timestamp, IpChangeKind::Added)
    }

    /// Creates a "removed" change event.
    #[must_use]
    pub fn removed(adapter: impl Into<String>, address: IpAddr, timestamp: SystemTime) -> Self {
        Self::new(adapter, address, timestamp, IpChangeKind::Removed)
    }

    /// Returns true if this is an "added" change.
    #[must_use]
    pub const fn is_added(&self) -> bool {
        matches!(self.kind, IpChangeKind::Added)
    }

    /// Returns true if this is a "removed" change.
    #[must_use]
    pub const fn is_removed(&self) -> bool {
        matches!(self.kind, IpChangeKind::Removed)
    }

    /// Returns true if this change involves an IPv4 address.
    #[must_use]
    pub const fn is_ipv4(&self) -> bool {
        self.address.is_ipv4()
    }

    /// Returns true if this change involves an IPv6 address.
    #[must_use]
    pub const fn is_ipv6(&self) -> bool {
        self.address.is_ipv6()
    }

    /// Returns true if this change matches the specified IP version filter.
    #[must_use]
    pub const fn matches_version(&self, version: IpVersion) -> bool {
        match version {
            IpVersion::V4 => self.address.is_ipv4(),
            IpVersion::V6 => self.address.is_ipv6(),
            IpVersion::Both => true,
        }
    }
}

/// Filters IP changes by the specified IP version.
///
/// Returns only changes that match the specified version:
/// - `V4`: only IPv4 changes
/// - `V6`: only IPv6 changes
/// - `Both`: all changes (no filtering)
///
/// # Arguments
///
/// * `changes` - The changes to filter
/// * `version` - The IP version filter to apply
#[must_use]
pub fn filter_by_version(changes: Vec<IpChange>, version: IpVersion) -> Vec<IpChange> {
    match version {
        IpVersion::Both => changes,
        IpVersion::V4 | IpVersion::V6 => changes
            .into_iter()
            .filter(|c| c.matches_version(version))
            .collect(),
    }
}

/// Compares two adapter snapshots and returns a list of IP changes.
///
/// This is a pure function that detects which IP addresses were added or removed
/// between two points in time. The comparison is done per-adapter by name.
///
/// # Arguments
///
/// * `old` - The previous state of network adapters
/// * `new` - The current state of network adapters
/// * `timestamp` - The timestamp to assign to all detected changes
///
/// # Returns
///
/// A vector of [`IpChange`] events. The order is not guaranteed.
///
/// # Algorithm
///
/// For each adapter (matched by name):
/// 1. Find addresses in `new` but not in `old` → `Added`
/// 2. Find addresses in `old` but not in `new` → `Removed`
///
/// Adapters that exist only in `old` have all their addresses marked as `Removed`.
/// Adapters that exist only in `new` have all their addresses marked as `Added`.
#[must_use]
pub fn diff(
    old: &[AdapterSnapshot],
    new: &[AdapterSnapshot],
    timestamp: SystemTime,
) -> Vec<IpChange> {
    let old_by_name: HashMap<&str, &AdapterSnapshot> =
        old.iter().map(|a| (a.name.as_str(), a)).collect();
    let new_by_name: HashMap<&str, &AdapterSnapshot> =
        new.iter().map(|a| (a.name.as_str(), a)).collect();

    let mut changes = Vec::new();

    // Process adapters that exist in old
    for (name, old_adapter) in &old_by_name {
        match new_by_name.get(name) {
            Some(new_adapter) => {
                // Adapter exists in both - compare addresses
                diff_adapter_addresses(&mut changes, name, old_adapter, new_adapter, timestamp);
            }
            None => {
                // Adapter removed - all addresses are removed
                add_all_addresses_as_removed(&mut changes, name, old_adapter, timestamp);
            }
        }
    }

    // Process adapters that only exist in new
    for (name, new_adapter) in &new_by_name {
        if !old_by_name.contains_key(name) {
            // New adapter - all addresses are added
            add_all_addresses_as_added(&mut changes, name, new_adapter, timestamp);
        }
    }

    changes
}

/// Compares addresses between old and new snapshots of the same adapter.
fn diff_adapter_addresses(
    changes: &mut Vec<IpChange>,
    adapter_name: &str,
    old: &AdapterSnapshot,
    new: &AdapterSnapshot,
    timestamp: SystemTime,
) {
    // Check IPv4 addresses
    for addr in &old.ipv4_addresses {
        if !new.ipv4_addresses.contains(addr) {
            changes.push(IpChange::removed(
                adapter_name,
                IpAddr::V4(*addr),
                timestamp,
            ));
        }
    }
    for addr in &new.ipv4_addresses {
        if !old.ipv4_addresses.contains(addr) {
            changes.push(IpChange::added(adapter_name, IpAddr::V4(*addr), timestamp));
        }
    }

    // Check IPv6 addresses
    for addr in &old.ipv6_addresses {
        if !new.ipv6_addresses.contains(addr) {
            changes.push(IpChange::removed(
                adapter_name,
                IpAddr::V6(*addr),
                timestamp,
            ));
        }
    }
    for addr in &new.ipv6_addresses {
        if !old.ipv6_addresses.contains(addr) {
            changes.push(IpChange::added(adapter_name, IpAddr::V6(*addr), timestamp));
        }
    }
}

/// Adds all addresses from an adapter as "removed" changes.
fn add_all_addresses_as_removed(
    changes: &mut Vec<IpChange>,
    adapter_name: &str,
    adapter: &AdapterSnapshot,
    timestamp: SystemTime,
) {
    for addr in &adapter.ipv4_addresses {
        changes.push(IpChange::removed(
            adapter_name,
            IpAddr::V4(*addr),
            timestamp,
        ));
    }
    for addr in &adapter.ipv6_addresses {
        changes.push(IpChange::removed(
            adapter_name,
            IpAddr::V6(*addr),
            timestamp,
        ));
    }
}

/// Adds all addresses from an adapter as "added" changes.
fn add_all_addresses_as_added(
    changes: &mut Vec<IpChange>,
    adapter_name: &str,
    adapter: &AdapterSnapshot,
    timestamp: SystemTime,
) {
    for addr in &adapter.ipv4_addresses {
        changes.push(IpChange::added(adapter_name, IpAddr::V4(*addr), timestamp));
    }
    for addr in &adapter.ipv6_addresses {
        changes.push(IpChange::added(adapter_name, IpAddr::V6(*addr), timestamp));
    }
}

#[cfg(test)]
#[path = "change_tests.rs"]
mod tests;
