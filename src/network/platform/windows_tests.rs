//! Tests for Windows-specific network adapter fetching.

use super::windows::{
    WindowsFetcher, classify_adapter, is_hardware_interface, map_if_type_fallback,
};
use crate::network::{AdapterKind, AddressFetcher};
use std::net::{Ipv4Addr, Ipv6Addr};
use windows::Win32::NetworkManagement::IpHelper::{
    IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IF_TYPE_SOFTWARE_LOOPBACK,
};

/// Interface type for PPP (Point-to-Point Protocol) adapters.
const IF_TYPE_PPP: u32 = 23;

/// Interface type for tunnel adapters (VPN, etc.).
const IF_TYPE_TUNNEL: u32 = 131;

mod map_if_type_fallback_tests {
    use super::*;

    #[test]
    fn ethernet() {
        assert_eq!(
            map_if_type_fallback(IF_TYPE_ETHERNET_CSMACD),
            AdapterKind::Ethernet
        );
    }

    #[test]
    fn wireless() {
        assert_eq!(
            map_if_type_fallback(IF_TYPE_IEEE80211),
            AdapterKind::Wireless
        );
    }

    #[test]
    fn loopback() {
        assert_eq!(
            map_if_type_fallback(IF_TYPE_SOFTWARE_LOOPBACK),
            AdapterKind::Loopback
        );
    }

    #[test]
    fn tunnel_is_virtual() {
        assert_eq!(map_if_type_fallback(IF_TYPE_TUNNEL), AdapterKind::Virtual);
    }

    #[test]
    fn ppp_is_virtual() {
        assert_eq!(map_if_type_fallback(IF_TYPE_PPP), AdapterKind::Virtual);
    }

    #[test]
    fn unknown_preserves_code() {
        assert_eq!(map_if_type_fallback(999), AdapterKind::Other(999));
    }
}

mod classify_adapter_tests {
    use super::*;

    // Use if_index=0 which will cause GetIfEntry2 to fail, testing fallback behavior
    const INVALID_IF_INDEX: u32 = 0;

    #[test]
    fn tunnel_always_virtual_regardless_of_index() {
        // Tunnel types should always be classified as Virtual
        assert_eq!(
            classify_adapter(IF_TYPE_TUNNEL, INVALID_IF_INDEX),
            AdapterKind::Virtual
        );
    }

    #[test]
    fn ppp_always_virtual_regardless_of_index() {
        // PPP types should always be classified as Virtual
        assert_eq!(
            classify_adapter(IF_TYPE_PPP, INVALID_IF_INDEX),
            AdapterKind::Virtual
        );
    }

    #[test]
    fn loopback_always_loopback_regardless_of_index() {
        assert_eq!(
            classify_adapter(IF_TYPE_SOFTWARE_LOOPBACK, INVALID_IF_INDEX),
            AdapterKind::Loopback
        );
    }

    #[test]
    fn unknown_type_preserves_code() {
        assert_eq!(
            classify_adapter(999, INVALID_IF_INDEX),
            AdapterKind::Other(999)
        );
    }
}

mod is_hardware_interface_tests {
    use super::*;

    #[test]
    fn invalid_index_returns_none() {
        // Index 0 is invalid, should return None
        assert!(is_hardware_interface(0).is_none());
    }

    #[test]
    fn very_large_index_returns_none() {
        // Very large index should also fail
        assert!(is_hardware_interface(u32::MAX).is_none());
    }
}

mod windows_fetcher_tests {
    use super::*;

    #[test]
    fn new_creates_instance() {
        let _fetcher = WindowsFetcher::new();
    }

    #[test]
    fn default_creates_instance() {
        let _fetcher = WindowsFetcher::default();
    }
}

mod integration_tests {
    use super::*;

    /// Integration test: actually fetches adapters from the system.
    /// This test verifies the Windows API integration works end-to-end.
    #[test]
    fn fetch_adapters_returns_at_least_loopback() {
        let fetcher = WindowsFetcher::new();
        let result = fetcher.fetch();

        assert!(result.is_ok(), "fetch() failed: {:?}", result.err());

        let adapters = result.unwrap();

        // Every Windows system should have at least the loopback adapter
        let has_loopback_addr = adapters.iter().any(|a| {
            a.ipv4_addresses.contains(&Ipv4Addr::LOCALHOST)
                || a.ipv6_addresses.contains(&Ipv6Addr::LOCALHOST)
        });

        assert!(
            has_loopback_addr,
            "Expected at least loopback address, got adapters: {adapters:?}"
        );
    }

    #[test]
    fn fetch_adapters_names_are_not_empty() {
        let fetcher = WindowsFetcher::new();
        let adapters = fetcher.fetch().expect("fetch() failed");

        for adapter in &adapters {
            assert!(
                !adapter.name.is_empty(),
                "Adapter name should not be empty: {adapter:?}"
            );
        }
    }

    /// Verifies that loopback adapter is classified as Loopback kind.
    #[test]
    fn loopback_adapter_has_loopback_kind() {
        let fetcher = WindowsFetcher::new();
        let adapters = fetcher.fetch().expect("fetch() failed");

        let loopback = adapters.iter().find(|a| {
            a.ipv4_addresses.contains(&Ipv4Addr::LOCALHOST)
                || a.ipv6_addresses.contains(&Ipv6Addr::LOCALHOST)
        });

        assert!(loopback.is_some(), "No loopback adapter found");
        assert_eq!(
            loopback.unwrap().kind,
            AdapterKind::Loopback,
            "Loopback adapter should have Loopback kind"
        );
    }

    /// Verifies that virtual adapters (if any) are classified as Virtual.
    /// This test helps verify the `HardwareInterface` detection works.
    #[test]
    fn virtual_adapters_are_not_ethernet_or_wireless() {
        let fetcher = WindowsFetcher::new();
        let adapters = fetcher.fetch().expect("fetch() failed");

        // Check adapters with "vEthernet" or "本地连接*" in name
        for adapter in &adapters {
            let name_lower = adapter.name.to_lowercase();
            let is_likely_virtual =
                name_lower.contains("vethernet") || adapter.name.starts_with("本地连接*");

            if is_likely_virtual {
                assert!(
                    adapter.kind == AdapterKind::Virtual,
                    "Adapter '{}' looks like a virtual adapter but is classified as {:?}",
                    adapter.name,
                    adapter.kind
                );
            }
        }
    }
}
