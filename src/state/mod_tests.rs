//! Tests for state persistence module.

use std::net::{Ipv4Addr, Ipv6Addr};

use tempfile::TempDir;

use crate::network::{AdapterKind, AdapterSnapshot};
use crate::state::{FileStateStore, LoadResult, StateStore};

/// Creates a test adapter snapshot with the given IPv4 address.
fn snapshot_with_ipv4(name: &str, ip: &str) -> AdapterSnapshot {
    AdapterSnapshot::new(
        name,
        AdapterKind::Ethernet,
        vec![ip.parse::<Ipv4Addr>().unwrap()],
        vec![],
    )
}

/// Creates a test adapter snapshot with both IPv4 and IPv6 addresses.
fn snapshot_with_both(name: &str, ipv4: &str, ipv6: &str) -> AdapterSnapshot {
    AdapterSnapshot::new(
        name,
        AdapterKind::Ethernet,
        vec![ipv4.parse::<Ipv4Addr>().unwrap()],
        vec![ipv6.parse::<Ipv6Addr>().unwrap()],
    )
}

mod load_result {
    use super::*;

    #[test]
    fn into_snapshots_returns_loaded_data() {
        let snapshots = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];
        let result = LoadResult::Loaded(snapshots.clone());

        assert_eq!(result.into_snapshots(), snapshots);
    }

    #[test]
    fn into_snapshots_returns_empty_for_not_found() {
        let result = LoadResult::NotFound;
        assert!(result.into_snapshots().is_empty());
    }

    #[test]
    fn into_snapshots_returns_empty_for_corrupted() {
        let result = LoadResult::Corrupted {
            reason: "test".to_string(),
        };
        assert!(result.into_snapshots().is_empty());
    }

    #[test]
    fn is_loaded_true_for_loaded() {
        let result = LoadResult::Loaded(vec![]);
        assert!(result.is_loaded());
    }

    #[test]
    fn is_loaded_false_for_not_found() {
        let result = LoadResult::NotFound;
        assert!(!result.is_loaded());
    }

    #[test]
    fn is_loaded_false_for_corrupted() {
        let result = LoadResult::Corrupted {
            reason: "test".to_string(),
        };
        assert!(!result.is_loaded());
    }
}

mod file_state_store {
    use super::*;

    #[test]
    fn load_returns_not_found_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let store = FileStateStore::new(&path);

        let result = store.load();
        assert!(matches!(result, LoadResult::NotFound));
    }

    #[test]
    fn load_returns_corrupted_for_invalid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, "not valid json {{{").unwrap();

        let store = FileStateStore::new(&path);
        let result = store.load();

        match result {
            LoadResult::Corrupted { reason } => {
                assert!(reason.contains("Invalid JSON"));
            }
            other => panic!("Expected Corrupted, got {other:?}"),
        }
    }

    #[test]
    fn load_returns_corrupted_for_incompatible_version() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, r#"{"version": 999, "snapshots": []}"#).unwrap();

        let store = FileStateStore::new(&path);
        let result = store.load();

        match result {
            LoadResult::Corrupted { reason } => {
                assert!(reason.contains("Incompatible version"));
                assert!(reason.contains("999"));
            }
            other => panic!("Expected Corrupted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let store = FileStateStore::new(&path);

        let snapshots = vec![
            snapshot_with_ipv4("eth0", "192.168.1.1"),
            snapshot_with_both("eth1", "10.0.0.1", "fe80::1"),
        ];

        // Save
        store.save(&snapshots).await.unwrap();

        // Verify file exists
        assert!(path.exists());

        // Load
        let result = store.load();
        match result {
            LoadResult::Loaded(loaded) => {
                assert_eq!(loaded.len(), 2);
                assert_eq!(loaded[0].name, "eth0");
                assert_eq!(loaded[0].ipv4_addresses[0].to_string(), "192.168.1.1");
                assert_eq!(loaded[1].name, "eth1");
                assert_eq!(loaded[1].ipv6_addresses[0].to_string(), "fe80::1");
            }
            other => panic!("Expected Loaded, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn save_overwrites_existing_state() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let store = FileStateStore::new(&path);

        // Save initial state
        let initial = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];
        store.save(&initial).await.unwrap();

        // Save new state (should overwrite)
        let updated = vec![snapshot_with_ipv4("eth0", "192.168.1.2")];
        store.save(&updated).await.unwrap();

        // Load should return updated state
        let result = store.load();
        match result {
            LoadResult::Loaded(loaded) => {
                assert_eq!(loaded.len(), 1);
                assert_eq!(loaded[0].ipv4_addresses[0].to_string(), "192.168.1.2");
            }
            other => panic!("Expected Loaded, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn save_empty_snapshots() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let store = FileStateStore::new(&path);

        store.save(&[]).await.unwrap();

        let result = store.load();
        match result {
            LoadResult::Loaded(loaded) => {
                assert!(loaded.is_empty());
            }
            other => panic!("Expected Loaded, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn save_preserves_all_adapter_kinds() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let store = FileStateStore::new(&path);

        let snapshots = vec![
            AdapterSnapshot::new("lo", AdapterKind::Loopback, vec![], vec![]),
            AdapterSnapshot::new("wlan0", AdapterKind::Wireless, vec![], vec![]),
            AdapterSnapshot::new("vmnet", AdapterKind::Virtual, vec![], vec![]),
            AdapterSnapshot::new("unknown", AdapterKind::Other(42), vec![], vec![]),
        ];

        store.save(&snapshots).await.unwrap();

        let result = store.load();
        match result {
            LoadResult::Loaded(loaded) => {
                assert_eq!(loaded.len(), 4);
                assert_eq!(loaded[0].kind, AdapterKind::Loopback);
                assert_eq!(loaded[1].kind, AdapterKind::Wireless);
                assert_eq!(loaded[2].kind, AdapterKind::Virtual);
                assert_eq!(loaded[3].kind, AdapterKind::Other(42));
            }
            other => panic!("Expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn path_returns_configured_path() {
        let store = FileStateStore::new("/tmp/test.json");
        assert_eq!(store.path().to_str().unwrap(), "/tmp/test.json");
    }

    #[tokio::test]
    async fn save_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let nested_path = dir.path().join("nested").join("deep").join("state.json");
        let store = FileStateStore::new(&nested_path);

        let snapshots = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];
        store.save(&snapshots).await.unwrap();

        // Verify file exists in nested directory
        assert!(nested_path.exists());

        // Verify we can load it back
        let result = store.load();
        assert!(result.is_loaded());
    }
}

mod mock_state_store {
    use super::*;
    use crate::state::mock::MockStateStore;

    #[test]
    fn with_loaded_returns_snapshots() {
        let snapshots = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];
        let store = MockStateStore::with_loaded(snapshots.clone());

        let result = store.load();
        match result {
            LoadResult::Loaded(loaded) => {
                assert_eq!(loaded, snapshots);
            }
            other => panic!("Expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn not_found_returns_not_found() {
        let store = MockStateStore::not_found();

        let result = store.load();
        assert!(matches!(result, LoadResult::NotFound));
    }

    #[test]
    fn corrupted_returns_corrupted_with_reason() {
        let store = MockStateStore::corrupted("test reason");

        let result = store.load();
        match result {
            LoadResult::Corrupted { reason } => {
                assert_eq!(reason, "test reason");
            }
            other => panic!("Expected Corrupted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn save_captures_snapshots() {
        let store = MockStateStore::not_found();
        let snapshots = vec![snapshot_with_ipv4("eth0", "192.168.1.1")];

        store.save(&snapshots).await.unwrap();

        let saved = store.saved_snapshots();
        assert_eq!(saved, Some(snapshots));
    }

    #[test]
    fn saved_snapshots_returns_none_before_save() {
        let store = MockStateStore::not_found();
        assert!(store.saved_snapshots().is_none());
    }
}
