# Project Overview

> **Doc Maintenance**: Keep concise, avoid redundancy, clean up outdated content promptly to reduce AI context usage.
> **Scope**: This document reflects the current codebase state only and does not describe future plans.
> **Goal**: Help AI quickly locate relevant code by module, type, and data flow.

## Module Map

| Module | Purpose |
|--------|---------|
| `network` | `AdapterSnapshot`, `AdapterKind`, `IpVersion`; `AddressFetcher` trait for platform-agnostic adapter info retrieval; `FetchError` variants |
| `network::platform` | Platform-specific implementations; `WindowsFetcher` on Windows using `GetAdaptersAddresses` |

## Key Types

```rust
// IP version filtering
IpVersion::V4 | V6 | Both  // includes_v4(), includes_v6()
AdapterKind::Ethernet | Wireless | Loopback | Virtual | Other(u32)  // is_virtual(), is_loopback()
AdapterSnapshot { name, kind, ipv4_addresses: Vec<Ipv4Addr>, ipv6_addresses: Vec<Ipv6Addr> }
AddressFetcher trait { fetch() -> Result<Vec<AdapterSnapshot>, FetchError> }
FetchError::WindowsApi(windows::core::Error)  // #[cfg(windows)]
          | PermissionDenied { context }
          | Platform { message }

// Platform implementations
WindowsFetcher::new()  // Windows only, uses GetAdaptersAddresses API
PlatformFetcher        // Type alias for WindowsFetcher on Windows
```
