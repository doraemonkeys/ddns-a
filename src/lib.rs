//! DDNS-A: Dynamic DNS Address Monitor
//!
//! A library for monitoring IP address changes on network adapters
//! and notifying external services via webhooks.

pub mod monitor;
pub mod network;
pub mod time;
pub mod webhook;
