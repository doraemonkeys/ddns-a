//! Tests for the `merge_changes` function.

use super::*;
use std::time::{Duration, SystemTime};

fn timestamp() -> SystemTime {
    SystemTime::UNIX_EPOCH
}

#[test]
fn empty_input_returns_empty() {
    let result = merge_changes(&[], timestamp());
    assert!(result.is_empty());
}

#[test]
fn single_added_preserved() {
    let addr: IpAddr = "192.168.1.1".parse().unwrap();
    let changes = vec![IpChange::added("eth0", addr, timestamp())];

    let result = merge_changes(&changes, timestamp());

    assert_eq!(result.len(), 1);
    assert!(result[0].is_added());
    assert_eq!(result[0].address, addr);
}

#[test]
fn single_removed_preserved() {
    let addr: IpAddr = "192.168.1.1".parse().unwrap();
    let changes = vec![IpChange::removed("eth0", addr, timestamp())];

    let result = merge_changes(&changes, timestamp());

    assert_eq!(result.len(), 1);
    assert!(result[0].is_removed());
}

#[test]
fn added_then_removed_cancels_out() {
    let addr: IpAddr = "192.168.1.1".parse().unwrap();
    let changes = vec![
        IpChange::added("eth0", addr, timestamp()),
        IpChange::removed("eth0", addr, timestamp()),
    ];

    let result = merge_changes(&changes, timestamp());

    assert!(result.is_empty());
}

#[test]
fn removed_then_added_cancels_out() {
    let addr: IpAddr = "192.168.1.1".parse().unwrap();
    let changes = vec![
        IpChange::removed("eth0", addr, timestamp()),
        IpChange::added("eth0", addr, timestamp()),
    ];

    let result = merge_changes(&changes, timestamp());

    assert!(result.is_empty());
}

#[test]
fn multiple_adds_merge_to_single() {
    let addr: IpAddr = "192.168.1.1".parse().unwrap();
    let changes = vec![
        IpChange::added("eth0", addr, timestamp()),
        IpChange::added("eth0", addr, timestamp()),
        IpChange::added("eth0", addr, timestamp()),
    ];

    let result = merge_changes(&changes, timestamp());

    assert_eq!(result.len(), 1);
    assert!(result[0].is_added());
}

#[test]
fn multiple_removes_merge_to_single() {
    let addr: IpAddr = "192.168.1.1".parse().unwrap();
    let changes = vec![
        IpChange::removed("eth0", addr, timestamp()),
        IpChange::removed("eth0", addr, timestamp()),
    ];

    let result = merge_changes(&changes, timestamp());

    assert_eq!(result.len(), 1);
    assert!(result[0].is_removed());
}

#[test]
fn different_addresses_independent() {
    let addr1: IpAddr = "192.168.1.1".parse().unwrap();
    let addr2: IpAddr = "192.168.1.2".parse().unwrap();
    let changes = vec![
        IpChange::added("eth0", addr1, timestamp()),
        IpChange::removed("eth0", addr2, timestamp()),
    ];

    let result = merge_changes(&changes, timestamp());

    assert_eq!(result.len(), 2);
}

#[test]
fn different_adapters_independent() {
    let addr: IpAddr = "192.168.1.1".parse().unwrap();
    let changes = vec![
        IpChange::added("eth0", addr, timestamp()),
        IpChange::removed("eth1", addr, timestamp()),
    ];

    let result = merge_changes(&changes, timestamp());

    assert_eq!(result.len(), 2);
}

#[test]
fn complex_sequence_with_partial_cancellation() {
    let addr1: IpAddr = "192.168.1.1".parse().unwrap();
    let addr2: IpAddr = "192.168.1.2".parse().unwrap();
    let changes = vec![
        // addr1: add, remove, add = net +1 (added)
        IpChange::added("eth0", addr1, timestamp()),
        IpChange::removed("eth0", addr1, timestamp()),
        IpChange::added("eth0", addr1, timestamp()),
        // addr2: remove, add = net 0 (cancelled)
        IpChange::removed("eth0", addr2, timestamp()),
        IpChange::added("eth0", addr2, timestamp()),
    ];

    let result = merge_changes(&changes, timestamp());

    assert_eq!(result.len(), 1);
    assert!(result[0].is_added());
    assert_eq!(result[0].address, addr1);
}

#[test]
fn uses_provided_timestamp() {
    let addr: IpAddr = "192.168.1.1".parse().unwrap();
    let old_ts = SystemTime::UNIX_EPOCH;
    let new_ts = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let changes = vec![IpChange::added("eth0", addr, old_ts)];

    let result = merge_changes(&changes, new_ts);

    assert_eq!(result[0].timestamp, new_ts);
}
