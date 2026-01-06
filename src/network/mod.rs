//! Network layer for fetching and representing adapter information.
//!
//! This module provides types and traits for:
//! - Representing network adapter snapshots ([`AdapterSnapshot`])
//! - IP version filtering ([`IpVersion`])
//! - Adapter type classification ([`AdapterKind`])
//! - Fetching adapter information ([`AddressFetcher`])
//! - Platform-specific implementations ([`platform`])

mod fetcher;
pub mod platform;
mod types;

pub use fetcher::{AddressFetcher, FetchError};
pub use types::{AdapterKind, AdapterSnapshot, IpVersion};
