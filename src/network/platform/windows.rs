//! Windows-specific network adapter fetching using `GetAdaptersAddresses`.

use crate::network::{AdapterKind, AdapterSnapshot, AddressFetcher, FetchError};
use std::net::{Ipv4Addr, Ipv6Addr};
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    GetIfEntry2, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IF_TYPE_SOFTWARE_LOOPBACK,
    IP_ADAPTER_ADDRESSES_LH, MIB_IF_ROW2,
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

/// Bitmask for the `HardwareInterface` flag in `InterfaceAndOperStatusFlags`.
/// Bit 0 indicates whether the interface is a hardware (physical) interface.
const HARDWARE_INTERFACE_FLAG: u8 = 0x1;

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
    /// Prevents external construction; use `new()` or `default()`.
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

    // Classify the adapter using IF_TYPE and HardwareInterface flag.
    // SAFETY: Anonymous1.Anonymous.IfIndex is the interface index for the adapter.
    // This union field is valid when reading adapter addresses returned by GetAdaptersAddresses.
    let if_index = unsafe { adapter.Anonymous1.Anonymous.IfIndex };
    let kind = classify_adapter(adapter.IfType, if_index);

    // Collect all unicast addresses
    let (ipv4_addresses, ipv6_addresses) = collect_addresses(adapter);

    Some(AdapterSnapshot::new(
        name,
        kind,
        ipv4_addresses,
        ipv6_addresses,
    ))
}

/// Queries whether an adapter is a hardware (physical) interface.
///
/// Uses `GetIfEntry2` to check the `HardwareInterface` bit in `InterfaceAndOperStatusFlags`.
/// Returns `None` on API failure (conservative approach: don't misclassify).
///
/// # Why This Matters
///
/// Windows virtual adapters (Hyper-V, `VMware`, Mobile Hotspot) report the same `IF_TYPE`
/// as physical adapters (e.g., `IF_TYPE_ETHERNET_CSMACD`). Only the `HardwareInterface`
/// flag reliably distinguishes physical hardware from software/virtual adapters.
pub(super) fn is_hardware_interface(if_index: u32) -> Option<bool> {
    let mut row = MIB_IF_ROW2 {
        InterfaceIndex: if_index,
        ..Default::default()
    };

    // SAFETY: MIB_IF_ROW2 is properly initialized with InterfaceIndex.
    // GetIfEntry2 fills in the rest of the structure.
    let result = unsafe { GetIfEntry2(&raw mut row) };

    if result.is_ok() {
        Some(row.InterfaceAndOperStatusFlags._bitfield & HARDWARE_INTERFACE_FLAG != 0)
    } else {
        // API failure: return None to signal conservative fallback
        None
    }
}

/// Classifies an adapter with hardware interface awareness.
///
/// This function combines `IF_TYPE` classification with the `HardwareInterface` flag
/// to correctly identify virtual adapters that masquerade as physical ones.
///
/// # Classification Logic
///
/// 1. Protocol-level virtual types (Tunnel, PPP) → always `Virtual`
/// 2. Loopback → always `Loopback`
/// 3. Ethernet/Wireless with `HardwareInterface=true` → physical type
/// 4. Ethernet/Wireless with `HardwareInterface=false` → `Virtual`
/// 5. API failure → conservative fallback to `IF_TYPE`-based classification
pub(super) fn classify_adapter(if_type: u32, if_index: u32) -> AdapterKind {
    // 1. Explicit protocol-level virtual types (highest confidence)
    if matches!(if_type, IF_TYPE_TUNNEL | IF_TYPE_PPP) {
        return AdapterKind::Virtual;
    }

    // 2. Loopback is always loopback
    if if_type == IF_TYPE_SOFTWARE_LOOPBACK {
        return AdapterKind::Loopback;
    }

    // 3. For Ethernet/Wireless: check if it's actually hardware
    if matches!(if_type, IF_TYPE_ETHERNET_CSMACD | IF_TYPE_IEEE80211) {
        match is_hardware_interface(if_index) {
            Some(true) => {
                // Confirmed physical hardware
                return if if_type == IF_TYPE_ETHERNET_CSMACD {
                    AdapterKind::Ethernet
                } else {
                    AdapterKind::Wireless
                };
            }
            Some(false) => {
                // Confirmed software/virtual (Hyper-V, VMware, Mobile Hotspot, etc.)
                return AdapterKind::Virtual;
            }
            None => {
                // API failed: fall through to default mapping (conservative)
            }
        }
    }

    // 4. Default fallback by IF_TYPE (for unknown types or API failure)
    map_if_type_fallback(if_type)
}

/// Maps Windows `IF_TYPE_*` constants to [`AdapterKind`] (fallback only).
///
/// Used when `GetIfEntry2` fails or for adapter types not requiring hardware check.
pub(super) const fn map_if_type_fallback(if_type: u32) -> AdapterKind {
    match if_type {
        IF_TYPE_ETHERNET_CSMACD => AdapterKind::Ethernet,
        IF_TYPE_IEEE80211 => AdapterKind::Wireless,
        IF_TYPE_SOFTWARE_LOOPBACK => AdapterKind::Loopback,
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
