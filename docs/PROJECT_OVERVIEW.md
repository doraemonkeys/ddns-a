# Project Overview

> **Doc Maintenance**: Keep concise, avoid redundancy, clean up outdated content promptly to reduce AI context usage.
> **Scope**: This document reflects the current codebase state only and does not describe future plans.
> **Goal**: Help AI quickly locate relevant code by module, type, and data flow.

## Module Map

| Module | Purpose |
|--------|---------|
| `network` | `AdapterSnapshot`, `AdapterKind`, `IpVersion`; `AddressFetcher` trait for platform-agnostic adapter info retrieval; `FetchError` variants |
| `network::filter` | `AdapterFilter` trait for filtering adapters; `NameRegexFilter`, `ExcludeVirtualFilter`, `ExcludeLoopbackFilter` concrete filters; `CompositeFilter` for AND composition; `FilteredFetcher` decorator |
| `network::platform` | Platform-specific implementations; `WindowsFetcher` on Windows using `GetAdaptersAddresses` |
| `monitor` | `IpChange`, `IpChangeKind`, `diff()` pure function for change detection; `DebouncePolicy` for event merging; `PollingMonitor`, `PollingStream` for polling-based monitoring; `HybridMonitor`, `HybridStream` for combined API+polling monitoring; `ApiListener` trait for platform event notifications; `MonitorError`, `ApiError` for layered error handling |
| `monitor::platform` | Platform-specific listeners; `WindowsApiListener` on Windows using `NotifyIpInterfaceChange` |
| `time` | `Clock` trait for time abstraction; `SystemClock` production implementation |

## Key Types

```rust
// IP version filtering
IpVersion::V4 | V6 | Both  // includes_v4(), includes_v6()
AdapterKind::Ethernet | Wireless | Loopback | Virtual | Other(u32)  // is_virtual(), is_loopback()
AdapterSnapshot { name, kind, ipv4_addresses: Vec<Ipv4Addr>, ipv6_addresses: Vec<Ipv6Addr> }
  // Methods: new(), has_addresses(), address_count()
AddressFetcher trait { fetch() -> Result<Vec<AdapterSnapshot>, FetchError> }  // Send + Sync
FetchError::WindowsApi(windows::core::Error)  // #[cfg(windows)]
          | PermissionDenied { context }
          | Platform { message }

// Adapter filtering
AdapterFilter trait { fn matches(&self, adapter: &AdapterSnapshot) -> bool }  // Send + Sync
FilterMode::Include | Exclude  // For name-based filtering
NameRegexFilter { pattern: Regex, mode: FilterMode }  // Filter by name regex
  // Factory: include(pattern), exclude(pattern)
ExcludeVirtualFilter  // Excludes virtual adapters (VMware, VirtualBox, etc.)
ExcludeLoopbackFilter  // Excludes loopback adapters
CompositeFilter { filters: Vec<Box<dyn AdapterFilter>> }  // AND composition
  // Builder: new(), with(filter); matches all if empty
FilteredFetcher<F, A> { inner: F, filter: A }  // Decorator for AddressFetcher
  // Implements AddressFetcher, filters results via filter.matches()

// Platform implementations
WindowsFetcher::new()  // Windows only, uses GetAdaptersAddresses API; Default trait
PlatformFetcher        // Type alias for WindowsFetcher on Windows

// Monitor types
IpChangeKind::Added | Removed
IpChange { adapter, address: IpAddr, timestamp: SystemTime, kind }
  // Methods: new(), added(), removed(), is_added(), is_removed()
diff(&old, &new, timestamp) -> Vec<IpChange>  // Pure function for change detection
DebouncePolicy::new(window), window() -> Duration  // Default: 2 seconds
PollingMonitor<F, C = SystemClock>  // Builder: new(), with_clock(), with_debounce()
  // Methods: interval(), debounce(), into_stream()
PollingStream<F, C>  // Stream<Item = Vec<IpChange>>, returned by PollingMonitor::into_stream()
HybridMonitor<F, L, C = SystemClock>  // Builder: new(), with_clock(), with_debounce()
  // Methods: poll_interval(), debounce(), into_stream()
  // Combines API events (L: ApiListener) with polling fallback
HybridStream<F, S, C>  // Stream<Item = Vec<IpChange>>, auto-degrades to polling on API failure
  // Methods: is_polling_only() - check if degraded to polling-only mode
merge_changes(&[IpChange], timestamp) -> Vec<IpChange>  // Net effect merge for external consumers

// Time abstraction
Clock trait { now() -> SystemTime }  // Send + Sync
SystemClock  // Production impl; Debug, Clone, Copy, Default

// API event listeners (one-time semantics: into_stream(self) consumes self)
ApiListener trait { type Stream; fn into_stream(self) -> Self::Stream }  // Send
  // Stream yields Result<(), ApiError> - notifications, not IP data
WindowsApiListener::new() -> Result<Self, ApiError>  // Windows only, uses NotifyIpInterfaceChange
WindowsApiStream  // Stream<Item = Result<(), ApiError>>, auto-cancels on drop
PlatformListener  // Type alias for WindowsApiListener on Windows

// Monitor errors (layered)
ApiError::WindowsApi(windows::core::Error)  // #[cfg(windows)]
       | Stopped
MonitorError::Fetch(FetchError)
           | ApiListenerFailed(#[source] ApiError)
```
