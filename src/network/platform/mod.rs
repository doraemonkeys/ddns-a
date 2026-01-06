//! Platform-specific network adapter fetcher implementations.
//!
//! This module provides conditional compilation for platform-specific
//! implementations of the [`AddressFetcher`] trait.
//!
//! # Platform Support
//!
//! - **Windows**: Uses `GetAdaptersAddresses` API via the `windows` crate.
//! - **Linux**: Planned for future (netlink).
//! - **macOS**: Planned for future (getifaddrs).

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::WindowsFetcher;

// Re-export platform-specific fetcher as PlatformFetcher for convenience
#[cfg(windows)]
pub use windows::WindowsFetcher as PlatformFetcher;
