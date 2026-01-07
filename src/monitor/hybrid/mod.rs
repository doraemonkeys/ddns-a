//! Hybrid IP address monitor combining API events with polling.
//!
//! This module provides:
//! - [`HybridMonitor`]: Builder/configuration for hybrid monitoring
//! - [`HybridStream`]: Stream that yields IP change events from both sources

mod monitor;
mod stream;

pub use monitor::HybridMonitor;
pub use stream::HybridStream;

#[cfg(test)]
mod monitor_tests;
#[cfg(test)]
mod stream_tests;
#[cfg(test)]
mod test_fixtures;
