//! Platform-specific IP address change listener implementations.
//!
//! This module provides conditional compilation for platform-specific
//! implementations of the [`ApiListener`] trait.
//!
//! # Platform Support
//!
//! - **Windows**: Uses `NotifyIpInterfaceChange` API via the `windows` crate.
//! - **Linux**: Planned for future (netlink).
//! - **macOS**: Planned for future (Network.framework).

#[cfg(windows)]
mod windows;

#[cfg(all(windows, test))]
mod windows_tests;

#[cfg(windows)]
pub use windows::WindowsApiListener;

#[cfg(windows)]
pub use windows::WindowsApiStream;

// Re-export platform-specific listener as PlatformListener for convenience
#[cfg(windows)]
pub use windows::WindowsApiListener as PlatformListener;
