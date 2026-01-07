//! Tests for IP change detection types and functions.

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
