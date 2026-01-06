//! Windows-specific network adapter fetching using `GetAdaptersAddresses`.

use crate::network::{AdapterKind, AdapterSnapshot, AddressFetcher, FetchError};
use std::net::{Ipv4Addr, Ipv6Addr};
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IF_TYPE_SOFTWARE_LOOPBACK, IP_ADAPTER_ADDRESSES_LH,
};
use windows::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6,
};

/// Interface type for PPP (Point-to-Point Protocol) adapters.
/// Value from Windows SDK `iptypes.h` - not exported by the `windows` crate.
const IF_TYPE_PPP: u32 = 23;

/// Interface type for tunnel adapters (VPN, etc.).
/// Value from Windows SDK `iptypes.h` - not exported by the `windows` crate.
const IF_TYPE_TUNNEL: u32 = 131;

/// Buffer size hint for `GetAdaptersAddresses`.
/// The API will tell us the actual required size if this is insufficient.
const INITIAL_BUFFER_SIZE: u32 = 16384;

/// Windows implementation of [`AddressFetcher`] using `GetAdaptersAddresses`.
///
/// This fetcher retrieves all network adapters and their IPv4/IPv6 addresses
/// from the Windows networking stack.
///
/// # Example
///
/// ```no_run
/// use ddns_a::network::{AddressFetcher, platform::WindowsFetcher};
///
/// let fetcher = WindowsFetcher::new();
/// let adapters = fetcher.fetch().expect("Failed to fetch adapters");
///
/// for adapter in adapters {
///     println!("{}: {:?}", adapter.name, adapter.ipv4_addresses);
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct WindowsFetcher {
    // Currently no configuration needed, but struct allows future extension
    _private: (),
}

impl WindowsFetcher {
    /// Creates a new Windows adapter fetcher.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl AddressFetcher for WindowsFetcher {
    fn fetch(&self) -> Result<Vec<AdapterSnapshot>, FetchError> {
        fetch_adapters()
    }
}

/// Fetches all network adapters using `GetAdaptersAddresses`.
fn fetch_adapters() -> Result<Vec<AdapterSnapshot>, FetchError> {
    let raw_adapters = get_adapter_addresses()?;

    let mut adapters = Vec::new();
    // SAFETY: GetAdaptersAddresses returns a properly aligned buffer for IP_ADAPTER_ADDRESSES_LH.
    // The Windows API guarantees alignment of the returned data structures.
    #[allow(clippy::cast_ptr_alignment)]
    let mut current = raw_adapters.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();

    // SAFETY: We iterate through a linked list returned by GetAdaptersAddresses.
    // The list is valid as long as the buffer (`raw_adapters`) is alive.
    while !current.is_null() {
        let adapter = unsafe { &*current };

        if let Some(snapshot) = parse_adapter(adapter) {
            adapters.push(snapshot);
        }

        current = adapter.Next;
    }

    Ok(adapters)
}

/// Calls `GetAdaptersAddresses` and returns the raw buffer containing adapter data.
///
/// This function handles the two-call pattern:
/// 1. First call with estimated buffer size
/// 2. Retry with exact size if buffer was too small
fn get_adapter_addresses() -> Result<Vec<u8>, FetchError> {
    // Flags to skip data we don't need (anycast, multicast, DNS servers)
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let family = u32::from(AF_UNSPEC.0); // Get both IPv4 and IPv6

    let mut buffer: Vec<u8> = vec![0u8; INITIAL_BUFFER_SIZE as usize];
    let mut size = INITIAL_BUFFER_SIZE;

    // SAFETY: We provide a valid buffer and size. The function writes adapter
    // information to the buffer and updates `size` with the required length.
    let result = unsafe {
        GetAdaptersAddresses(
            family,
            flags,
            None,
            Some(buffer.as_mut_ptr().cast()),
            &raw mut size,
        )
    };

    // Handle the result - delegate to helper for buffer overflow case
    handle_api_result(result, &mut buffer, &mut size, flags, family)?;

    Ok(buffer)
}

/// Handles the result of `GetAdaptersAddresses`, potentially retrying with a larger buffer.
///
/// # Coverage Note
///
/// This function is excluded from coverage because:
/// - Buffer overflow case requires a system with network adapter data exceeding 16KB
/// - Error paths require actual Windows API failures which cannot be mocked
#[cfg(not(tarpaulin_include))]
fn handle_api_result(
    result: u32,
    buffer: &mut Vec<u8>,
    size: &mut u32,
    flags: windows::Win32::NetworkManagement::IpHelper::GET_ADAPTERS_ADDRESSES_FLAGS,
    family: u32,
) -> Result<(), FetchError> {
    use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};

    if result == ERROR_BUFFER_OVERFLOW.0 {
        buffer.resize(*size as usize, 0);

        // SAFETY: Same as above, but with correctly sized buffer
        let result = unsafe {
            GetAdaptersAddresses(
                family,
                flags,
                None,
                Some(buffer.as_mut_ptr().cast()),
                &raw mut *size,
            )
        };

        if result != NO_ERROR.0 {
            return Err(windows::core::Error::from(WIN32_ERROR(result)).into());
        }
    } else if result != NO_ERROR.0 {
        return Err(windows::core::Error::from(WIN32_ERROR(result)).into());
    }

    Ok(())
}

/// Parses a single `IP_ADAPTER_ADDRESSES_LH` structure into an [`AdapterSnapshot`].
///
/// Returns `None` if the adapter name cannot be read.
fn parse_adapter(adapter: &IP_ADAPTER_ADDRESSES_LH) -> Option<AdapterSnapshot> {
    // Get the friendly name (wide string)
    let name = unsafe { adapter.FriendlyName.to_string().ok()? };

    // Map the adapter type
    let kind = map_adapter_type(adapter.IfType);

    // Collect all unicast addresses
    let (ipv4_addresses, ipv6_addresses) = collect_addresses(adapter);

    Some(AdapterSnapshot::new(
        name,
        kind,
        ipv4_addresses,
        ipv6_addresses,
    ))
}

/// Maps Windows `IF_TYPE_*` constants to [`AdapterKind`].
const fn map_adapter_type(if_type: u32) -> AdapterKind {
    match if_type {
        IF_TYPE_ETHERNET_CSMACD => AdapterKind::Ethernet,
        IF_TYPE_IEEE80211 => AdapterKind::Wireless,
        IF_TYPE_SOFTWARE_LOOPBACK => AdapterKind::Loopback,
        // Common virtual adapter types (tunnel, PPP, etc.)
        IF_TYPE_TUNNEL | IF_TYPE_PPP => AdapterKind::Virtual,
        other => AdapterKind::Other(other),
    }
}

/// Collects IPv4 and IPv6 unicast addresses from an adapter.
///
/// # Safety Note
///
/// The pointer casts to `SOCKADDR_IN` and `SOCKADDR_IN6` are allowed despite alignment
/// concerns because Windows guarantees proper alignment of these structures when returned
/// from the networking APIs.
#[allow(clippy::cast_ptr_alignment)]
fn collect_addresses(adapter: &IP_ADAPTER_ADDRESSES_LH) -> (Vec<Ipv4Addr>, Vec<Ipv6Addr>) {
    let mut ipv4_addresses = Vec::new();
    let mut ipv6_addresses = Vec::new();

    let mut unicast = adapter.FirstUnicastAddress;

    // SAFETY: We iterate through a linked list of unicast addresses.
    // Each address is valid as long as the parent adapter buffer is alive.
    while !unicast.is_null() {
        let addr_entry = unsafe { &*unicast };

        // SAFETY: The Address field contains a valid SOCKET_ADDRESS structure
        // pointing to either SOCKADDR_IN (IPv4) or SOCKADDR_IN6 (IPv6).
        if let Some(sockaddr) = unsafe { addr_entry.Address.lpSockaddr.as_ref() } {
            match sockaddr.sa_family {
                f if f == AF_INET => {
                    // SAFETY: We verified the family is AF_INET, so this is a valid cast.
                    let sockaddr_in =
                        unsafe { &*(std::ptr::from_ref(sockaddr).cast::<SOCKADDR_IN>()) };
                    // SAFETY: sin_addr contains the IPv4 address bytes in network order.
                    let octets = unsafe { sockaddr_in.sin_addr.S_un.S_un_b };
                    let addr = Ipv4Addr::new(octets.s_b1, octets.s_b2, octets.s_b3, octets.s_b4);
                    ipv4_addresses.push(addr);
                }
                f if f == AF_INET6 => {
                    // SAFETY: We verified the family is AF_INET6, so this is a valid cast.
                    let sockaddr_in6 =
                        unsafe { &*(std::ptr::from_ref(sockaddr).cast::<SOCKADDR_IN6>()) };
                    // SAFETY: We verified this is an IPv6 address, so the union field is valid.
                    let octets = unsafe { sockaddr_in6.sin6_addr.u.Byte };
                    let addr = Ipv6Addr::from(octets);
                    ipv6_addresses.push(addr);
                }
                // Unknown address family, skip - Windows typically only returns
                // AF_INET or AF_INET6 for unicast addresses
                _ => {}
            }
        }

        unicast = unsafe { (*unicast).Next };
    }

    (ipv4_addresses, ipv6_addresses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_adapter_type_ethernet() {
        assert_eq!(
            map_adapter_type(IF_TYPE_ETHERNET_CSMACD),
            AdapterKind::Ethernet
        );
    }

    #[test]
    fn map_adapter_type_wireless() {
        assert_eq!(map_adapter_type(IF_TYPE_IEEE80211), AdapterKind::Wireless);
    }

    #[test]
    fn map_adapter_type_loopback() {
        assert_eq!(
            map_adapter_type(IF_TYPE_SOFTWARE_LOOPBACK),
            AdapterKind::Loopback
        );
    }

    #[test]
    fn map_adapter_type_tunnel_is_virtual() {
        assert_eq!(map_adapter_type(IF_TYPE_TUNNEL), AdapterKind::Virtual);
    }

    #[test]
    fn map_adapter_type_ppp_is_virtual() {
        assert_eq!(map_adapter_type(IF_TYPE_PPP), AdapterKind::Virtual);
    }

    #[test]
    fn map_adapter_type_unknown_preserves_code() {
        assert_eq!(map_adapter_type(999), AdapterKind::Other(999));
    }

    #[test]
    fn windows_fetcher_new_creates_instance() {
        let _fetcher = WindowsFetcher::new();
        // Just verify it compiles and runs
    }

    #[test]
    fn windows_fetcher_default_creates_instance() {
        let _fetcher = WindowsFetcher::default();
        // Just verify it compiles and runs
    }

    // Integration test: actually fetches adapters from the system
    // This test verifies the Windows API integration works end-to-end
    #[test]
    fn fetch_adapters_returns_at_least_loopback() {
        let fetcher = WindowsFetcher::new();
        let result = fetcher.fetch();

        // Should succeed on any Windows system
        assert!(result.is_ok(), "fetch() failed: {:?}", result.err());

        let adapters = result.unwrap();

        // Every Windows system should have at least the loopback adapter
        // with address 127.0.0.1 or ::1
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
}
