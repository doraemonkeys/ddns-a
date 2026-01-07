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
mod tests {
    use super::*;
    use crate::network::AdapterKind;

    fn make_snapshot(name: &str, ipv4: Vec<&str>, ipv6: Vec<&str>) -> AdapterSnapshot {
        AdapterSnapshot::new(
            name,
            AdapterKind::Ethernet,
            ipv4.into_iter().map(|s| s.parse().unwrap()).collect(),
            ipv6.into_iter().map(|s| s.parse().unwrap()).collect(),
        )
    }

    fn timestamp() -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    mod ip_change_kind {
        use super::*;

        #[test]
        fn added_variant_exists() {
            let kind = IpChangeKind::Added;
            assert!(matches!(kind, IpChangeKind::Added));
        }

        #[test]
        fn removed_variant_exists() {
            let kind = IpChangeKind::Removed;
            assert!(matches!(kind, IpChangeKind::Removed));
        }

        #[test]
        fn equality_works() {
            assert_eq!(IpChangeKind::Added, IpChangeKind::Added);
            assert_eq!(IpChangeKind::Removed, IpChangeKind::Removed);
            assert_ne!(IpChangeKind::Added, IpChangeKind::Removed);
        }

        #[test]
        fn copy_works() {
            let kind = IpChangeKind::Added;
            let copied = kind;
            assert_eq!(kind, copied);
        }

        #[test]
        fn debug_format() {
            assert_eq!(format!("{:?}", IpChangeKind::Added), "Added");
            assert_eq!(format!("{:?}", IpChangeKind::Removed), "Removed");
        }
    }

    mod ip_change {
        use super::*;
        use std::net::{Ipv4Addr, Ipv6Addr};

        #[test]
        fn new_creates_change_with_correct_fields() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let ts = timestamp();
            let change = IpChange::new("eth0", addr, ts, IpChangeKind::Added);

            assert_eq!(change.adapter, "eth0");
            assert_eq!(change.address, addr);
            assert_eq!(change.timestamp, ts);
            assert_eq!(change.kind, IpChangeKind::Added);
        }

        #[test]
        fn added_helper_creates_added_change() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let change = IpChange::added("eth0", addr, timestamp());

            assert_eq!(change.kind, IpChangeKind::Added);
            assert!(change.is_added());
            assert!(!change.is_removed());
        }

        #[test]
        fn removed_helper_creates_removed_change() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let change = IpChange::removed("eth0", addr, timestamp());

            assert_eq!(change.kind, IpChangeKind::Removed);
            assert!(change.is_removed());
            assert!(!change.is_added());
        }

        #[test]
        fn works_with_ipv4() {
            let addr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
            let change = IpChange::added("eth0", addr, timestamp());

            assert!(change.address.is_ipv4());
        }

        #[test]
        fn works_with_ipv6() {
            let addr = IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1));
            let change = IpChange::added("eth0", addr, timestamp());

            assert!(change.address.is_ipv6());
        }

        #[test]
        fn equality_requires_all_fields() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let ts = timestamp();

            let change1 = IpChange::new("eth0", addr, ts, IpChangeKind::Added);
            let change2 = IpChange::new("eth0", addr, ts, IpChangeKind::Added);
            let change3 = IpChange::new("eth1", addr, ts, IpChangeKind::Added);
            let change4 = IpChange::new("eth0", addr, ts, IpChangeKind::Removed);

            assert_eq!(change1, change2);
            assert_ne!(change1, change3); // different adapter
            assert_ne!(change1, change4); // different kind
        }

        #[test]
        fn clone_creates_identical_change() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let change = IpChange::added("eth0", addr, timestamp());
            let cloned = change.clone();

            assert_eq!(change, cloned);
        }
    }

    mod diff_function {
        use super::*;

        #[test]
        fn empty_to_empty_returns_no_changes() {
            let changes = diff(&[], &[], timestamp());
            assert!(changes.is_empty());
        }

        #[test]
        fn detects_added_ipv4_on_new_adapter() {
            let old: Vec<AdapterSnapshot> = vec![];
            let new = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].adapter, "eth0");
            assert_eq!(changes[0].address.to_string(), "192.168.1.1");
            assert!(changes[0].is_added());
        }

        #[test]
        fn detects_added_ipv6_on_new_adapter() {
            let old: Vec<AdapterSnapshot> = vec![];
            let new = vec![make_snapshot("eth0", vec![], vec!["fe80::1"])];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].adapter, "eth0");
            assert_eq!(changes[0].address.to_string(), "fe80::1");
            assert!(changes[0].is_added());
        }

        #[test]
        fn detects_removed_ipv4_on_deleted_adapter() {
            let old = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])];
            let new: Vec<AdapterSnapshot> = vec![];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].adapter, "eth0");
            assert_eq!(changes[0].address.to_string(), "192.168.1.1");
            assert!(changes[0].is_removed());
        }

        #[test]
        fn detects_removed_ipv6_on_deleted_adapter() {
            let old = vec![make_snapshot("eth0", vec![], vec!["fe80::1"])];
            let new: Vec<AdapterSnapshot> = vec![];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 1);
            assert!(changes[0].is_removed());
        }

        #[test]
        fn detects_added_address_on_existing_adapter() {
            let old = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])];
            let new = vec![make_snapshot(
                "eth0",
                vec!["192.168.1.1", "192.168.1.2"],
                vec![],
            )];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].address.to_string(), "192.168.1.2");
            assert!(changes[0].is_added());
        }

        #[test]
        fn detects_removed_address_on_existing_adapter() {
            let old = vec![make_snapshot(
                "eth0",
                vec!["192.168.1.1", "192.168.1.2"],
                vec![],
            )];
            let new = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].address.to_string(), "192.168.1.2");
            assert!(changes[0].is_removed());
        }

        #[test]
        fn detects_address_replacement() {
            let old = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])];
            let new = vec![make_snapshot("eth0", vec!["192.168.1.2"], vec![])];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 2);

            let removed = changes.iter().find(|c| c.is_removed()).unwrap();
            let added = changes.iter().find(|c| c.is_added()).unwrap();

            assert_eq!(removed.address.to_string(), "192.168.1.1");
            assert_eq!(added.address.to_string(), "192.168.1.2");
        }

        #[test]
        fn no_changes_when_identical() {
            let snapshot = make_snapshot("eth0", vec!["192.168.1.1"], vec!["fe80::1"]);
            let old = vec![snapshot.clone()];
            let new = vec![snapshot];

            let changes = diff(&old, &new, timestamp());

            assert!(changes.is_empty());
        }

        #[test]
        fn handles_multiple_adapters() {
            let old = vec![
                make_snapshot("eth0", vec!["192.168.1.1"], vec![]),
                make_snapshot("eth1", vec!["10.0.0.1"], vec![]),
            ];
            let new = vec![
                make_snapshot("eth0", vec!["192.168.1.1", "192.168.1.2"], vec![]),
                make_snapshot("eth1", vec![], vec![]), // eth1 lost its address
            ];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 2);

            let eth0_change = changes.iter().find(|c| c.adapter == "eth0").unwrap();
            let eth1_change = changes.iter().find(|c| c.adapter == "eth1").unwrap();

            assert!(eth0_change.is_added());
            assert_eq!(eth0_change.address.to_string(), "192.168.1.2");

            assert!(eth1_change.is_removed());
            assert_eq!(eth1_change.address.to_string(), "10.0.0.1");
        }

        #[test]
        fn handles_mixed_ipv4_and_ipv6_changes() {
            let old = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec!["fe80::1"])];
            let new = vec![make_snapshot("eth0", vec!["192.168.1.2"], vec!["fe80::2"])];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 4); // 2 removed + 2 added

            let removed_count = changes.iter().filter(|c| c.is_removed()).count();
            let added_count = changes.iter().filter(|c| c.is_added()).count();

            assert_eq!(removed_count, 2);
            assert_eq!(added_count, 2);
        }

        #[test]
        fn timestamp_is_assigned_to_all_changes() {
            let ts = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_234_567_890);
            let old: Vec<AdapterSnapshot> = vec![];
            let new = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec!["fe80::1"])];

            let changes = diff(&old, &new, ts);

            assert!(changes.iter().all(|c| c.timestamp == ts));
        }

        #[test]
        fn adapter_with_no_addresses_to_having_addresses() {
            let old = vec![make_snapshot("eth0", vec![], vec![])];
            let new = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 1);
            assert!(changes[0].is_added());
        }

        #[test]
        fn adapter_with_addresses_to_no_addresses() {
            let old = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])];
            let new = vec![make_snapshot("eth0", vec![], vec![])];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 1);
            assert!(changes[0].is_removed());
        }

        #[test]
        fn handles_adapter_appearing_and_another_disappearing() {
            let old = vec![make_snapshot("eth0", vec!["192.168.1.1"], vec![])];
            let new = vec![make_snapshot("eth1", vec!["10.0.0.1"], vec![])];

            let changes = diff(&old, &new, timestamp());

            assert_eq!(changes.len(), 2);

            let eth0_change = changes.iter().find(|c| c.adapter == "eth0").unwrap();
            let eth1_change = changes.iter().find(|c| c.adapter == "eth1").unwrap();

            assert!(eth0_change.is_removed());
            assert!(eth1_change.is_added());
        }

        #[test]
        fn handles_multiple_addresses_on_single_adapter() {
            let old = vec![make_snapshot(
                "eth0",
                vec!["192.168.1.1", "192.168.1.2", "192.168.1.3"],
                vec![],
            )];
            let new = vec![make_snapshot(
                "eth0",
                vec!["192.168.1.2", "192.168.1.4", "192.168.1.5"],
                vec![],
            )];

            let changes = diff(&old, &new, timestamp());

            // Removed: .1, .3
            // Added: .4, .5
            assert_eq!(changes.len(), 4);

            let removed_count = changes.iter().filter(|c| c.is_removed()).count();
            let added_count = changes.iter().filter(|c| c.is_added()).count();

            assert_eq!(removed_count, 2);
            assert_eq!(added_count, 2);
        }
    }

    mod filter_by_version_function {
        use super::*;

        fn make_ipv4_change(addr: &str) -> IpChange {
            let address: IpAddr = addr.parse().unwrap();
            debug_assert!(address.is_ipv4(), "Expected IPv4 address");
            IpChange::added("eth0", address, timestamp())
        }

        fn make_ipv6_change(addr: &str) -> IpChange {
            let address: IpAddr = addr.parse().unwrap();
            debug_assert!(address.is_ipv6(), "Expected IPv6 address");
            IpChange::added("eth0", address, timestamp())
        }

        #[test]
        fn v4_filter_keeps_only_ipv4() {
            let changes = vec![
                make_ipv4_change("192.168.1.1"),
                make_ipv6_change("fe80::1"),
                make_ipv4_change("10.0.0.1"),
            ];

            let filtered = filter_by_version(changes, IpVersion::V4);

            assert_eq!(filtered.len(), 2);
            assert!(filtered.iter().all(IpChange::is_ipv4));
        }

        #[test]
        fn v6_filter_keeps_only_ipv6() {
            let changes = vec![
                make_ipv4_change("192.168.1.1"),
                make_ipv6_change("fe80::1"),
                make_ipv6_change("2001:db8::1"),
            ];

            let filtered = filter_by_version(changes, IpVersion::V6);

            assert_eq!(filtered.len(), 2);
            assert!(filtered.iter().all(IpChange::is_ipv6));
        }

        #[test]
        fn both_filter_keeps_all() {
            let changes = vec![
                make_ipv4_change("192.168.1.1"),
                make_ipv6_change("fe80::1"),
                make_ipv4_change("10.0.0.1"),
            ];

            let filtered = filter_by_version(changes, IpVersion::Both);

            assert_eq!(filtered.len(), 3);
        }

        #[test]
        fn empty_input_returns_empty() {
            let changes: Vec<IpChange> = vec![];

            let filtered = filter_by_version(changes, IpVersion::V4);

            assert!(filtered.is_empty());
        }

        #[test]
        fn v4_filter_on_all_ipv6_returns_empty() {
            let changes = vec![make_ipv6_change("fe80::1"), make_ipv6_change("2001:db8::1")];

            let filtered = filter_by_version(changes, IpVersion::V4);

            assert!(filtered.is_empty());
        }

        #[test]
        fn v6_filter_on_all_ipv4_returns_empty() {
            let changes = vec![
                make_ipv4_change("192.168.1.1"),
                make_ipv4_change("10.0.0.1"),
            ];

            let filtered = filter_by_version(changes, IpVersion::V6);

            assert!(filtered.is_empty());
        }
    }

    mod ip_change_version_methods {
        use super::*;

        #[test]
        fn is_ipv4_returns_true_for_ipv4() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let change = IpChange::added("eth0", addr, timestamp());

            assert!(change.is_ipv4());
            assert!(!change.is_ipv6());
        }

        #[test]
        fn is_ipv6_returns_true_for_ipv6() {
            let addr: IpAddr = "fe80::1".parse().unwrap();
            let change = IpChange::added("eth0", addr, timestamp());

            assert!(change.is_ipv6());
            assert!(!change.is_ipv4());
        }

        #[test]
        fn matches_version_v4_for_ipv4() {
            let addr: IpAddr = "192.168.1.1".parse().unwrap();
            let change = IpChange::added("eth0", addr, timestamp());

            assert!(change.matches_version(IpVersion::V4));
            assert!(!change.matches_version(IpVersion::V6));
            assert!(change.matches_version(IpVersion::Both));
        }

        #[test]
        fn matches_version_v6_for_ipv6() {
            let addr: IpAddr = "fe80::1".parse().unwrap();
            let change = IpChange::added("eth0", addr, timestamp());

            assert!(!change.matches_version(IpVersion::V4));
            assert!(change.matches_version(IpVersion::V6));
            assert!(change.matches_version(IpVersion::Both));
        }
    }
}
