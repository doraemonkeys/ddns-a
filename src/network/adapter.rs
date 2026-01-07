//! Core network types for adapter representation.

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

/// IP version to monitor (explicit specification required, no default).
///
/// # Design Decision
///
/// This enum requires explicit configuration to avoid hidden behavior.
/// Users must consciously choose which IP version(s) to monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IpVersion {
    /// Monitor IPv4 addresses only.
    V4,
    /// Monitor IPv6 addresses only.
    V6,
    /// Monitor both IPv4 and IPv6 addresses.
    Both,
}

impl IpVersion {
    /// Returns true if this version includes IPv4.
    #[must_use]
    pub const fn includes_v4(self) -> bool {
        matches!(self, Self::V4 | Self::Both)
    }

    /// Returns true if this version includes IPv6.
    #[must_use]
    pub const fn includes_v6(self) -> bool {
        matches!(self, Self::V6 | Self::Both)
    }
}

impl fmt::Display for IpVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V4 => write!(f, "IPv4"),
            Self::V6 => write!(f, "IPv6"),
            Self::Both => write!(f, "Both"),
        }
    }
}

/// Network adapter type classification.
///
/// Used for logging, filtering, and debugging. The core logic does not
/// depend on specific values, allowing platform-specific implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdapterKind {
    /// Physical Ethernet adapter.
    Ethernet,
    /// Wireless (Wi-Fi) adapter.
    Wireless,
    /// Loopback adapter (localhost).
    Loopback,
    /// Virtual adapter (`VMware`, `VirtualBox`, `Hyper-V`, WSL, etc.).
    Virtual,
    /// Unknown or other adapter type, preserving the original type code for debugging.
    Other(u32),
}

impl AdapterKind {
    /// Returns true if this is a virtual adapter.
    #[must_use]
    pub const fn is_virtual(&self) -> bool {
        matches!(self, Self::Virtual)
    }

    /// Returns true if this is a loopback adapter.
    #[must_use]
    pub const fn is_loopback(&self) -> bool {
        matches!(self, Self::Loopback)
    }
}

/// A snapshot of a single network adapter's addresses at a point in time.
///
/// # Equality
///
/// Two snapshots are equal if they have the same name, kind, and addresses.
/// Address order matters for equality comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterSnapshot {
    /// The friendly name of the adapter (e.g., "Ethernet", "Wi-Fi").
    pub name: String,
    /// The type of adapter.
    pub kind: AdapterKind,
    /// All IPv4 addresses assigned to this adapter.
    pub ipv4_addresses: Vec<Ipv4Addr>,
    /// All IPv6 addresses assigned to this adapter.
    pub ipv6_addresses: Vec<Ipv6Addr>,
}

impl AdapterSnapshot {
    /// Creates a new adapter snapshot.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        kind: AdapterKind,
        ipv4_addresses: Vec<Ipv4Addr>,
        ipv6_addresses: Vec<Ipv6Addr>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            ipv4_addresses,
            ipv6_addresses,
        }
    }

    /// Returns true if this adapter has any addresses (IPv4 or IPv6).
    #[must_use]
    pub fn has_addresses(&self) -> bool {
        !self.ipv4_addresses.is_empty() || !self.ipv6_addresses.is_empty()
    }

    /// Returns the total number of addresses (IPv4 + IPv6).
    #[must_use]
    pub fn address_count(&self) -> usize {
        self.ipv4_addresses.len() + self.ipv6_addresses.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod ip_version {
        use super::*;

        #[test]
        fn v4_includes_only_v4() {
            assert!(IpVersion::V4.includes_v4());
            assert!(!IpVersion::V4.includes_v6());
        }

        #[test]
        fn v6_includes_only_v6() {
            assert!(!IpVersion::V6.includes_v4());
            assert!(IpVersion::V6.includes_v6());
        }

        #[test]
        fn both_includes_both() {
            assert!(IpVersion::Both.includes_v4());
            assert!(IpVersion::Both.includes_v6());
        }

        #[test]
        fn display_formats_correctly() {
            assert_eq!(format!("{}", IpVersion::V4), "IPv4");
            assert_eq!(format!("{}", IpVersion::V6), "IPv6");
            assert_eq!(format!("{}", IpVersion::Both), "Both");
        }
    }

    mod adapter_kind {
        use super::*;

        #[test]
        fn virtual_is_virtual() {
            assert!(AdapterKind::Virtual.is_virtual());
            assert!(!AdapterKind::Ethernet.is_virtual());
            assert!(!AdapterKind::Wireless.is_virtual());
            assert!(!AdapterKind::Loopback.is_virtual());
            assert!(!AdapterKind::Other(999).is_virtual());
        }

        #[test]
        fn loopback_is_loopback() {
            assert!(AdapterKind::Loopback.is_loopback());
            assert!(!AdapterKind::Ethernet.is_loopback());
            assert!(!AdapterKind::Virtual.is_loopback());
        }

        #[test]
        fn other_preserves_type_code() {
            let kind = AdapterKind::Other(42);
            assert_eq!(kind, AdapterKind::Other(42));
            assert_ne!(kind, AdapterKind::Other(99));
        }
    }

    mod adapter_snapshot {
        use super::*;

        fn make_snapshot() -> AdapterSnapshot {
            AdapterSnapshot::new(
                "eth0",
                AdapterKind::Ethernet,
                vec!["192.168.1.1".parse().unwrap()],
                vec!["fe80::1".parse().unwrap()],
            )
        }

        #[test]
        fn new_creates_snapshot_with_correct_fields() {
            let snapshot = make_snapshot();

            assert_eq!(snapshot.name, "eth0");
            assert_eq!(snapshot.kind, AdapterKind::Ethernet);
            assert_eq!(snapshot.ipv4_addresses.len(), 1);
            assert_eq!(snapshot.ipv6_addresses.len(), 1);
        }

        #[test]
        fn has_addresses_true_with_ipv4() {
            let snapshot = AdapterSnapshot::new(
                "eth0",
                AdapterKind::Ethernet,
                vec!["192.168.1.1".parse().unwrap()],
                vec![],
            );
            assert!(snapshot.has_addresses());
        }

        #[test]
        fn has_addresses_true_with_ipv6() {
            let snapshot = AdapterSnapshot::new(
                "eth0",
                AdapterKind::Ethernet,
                vec![],
                vec!["fe80::1".parse().unwrap()],
            );
            assert!(snapshot.has_addresses());
        }

        #[test]
        fn has_addresses_false_when_empty() {
            let snapshot = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
            assert!(!snapshot.has_addresses());
        }

        #[test]
        fn address_count_sums_both_types() {
            let snapshot = AdapterSnapshot::new(
                "eth0",
                AdapterKind::Ethernet,
                vec![
                    "192.168.1.1".parse().unwrap(),
                    "192.168.1.2".parse().unwrap(),
                ],
                vec!["fe80::1".parse().unwrap()],
            );
            assert_eq!(snapshot.address_count(), 3);
        }

        #[test]
        fn address_count_zero_when_empty() {
            let snapshot = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
            assert_eq!(snapshot.address_count(), 0);
        }

        #[test]
        fn equality_requires_same_name() {
            let snapshot1 = make_snapshot();
            let mut snapshot2 = make_snapshot();
            snapshot2.name = "eth1".to_string();

            assert_ne!(snapshot1, snapshot2);
        }

        #[test]
        fn equality_requires_same_kind() {
            let snapshot1 = make_snapshot();
            let mut snapshot2 = make_snapshot();
            snapshot2.kind = AdapterKind::Wireless;

            assert_ne!(snapshot1, snapshot2);
        }

        #[test]
        fn equality_requires_same_addresses() {
            let snapshot1 = make_snapshot();
            let mut snapshot2 = make_snapshot();
            snapshot2.ipv4_addresses.push("10.0.0.1".parse().unwrap());

            assert_ne!(snapshot1, snapshot2);
        }
    }
}
