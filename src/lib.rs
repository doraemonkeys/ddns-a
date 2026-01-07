//! DDNS-A: Dynamic DNS Address Monitor
//!
//! A library for monitoring IP address changes on network adapters
//! and notifying external services via webhooks.

pub mod config;
pub mod monitor;
pub mod network;
pub mod state;
pub mod time;
pub mod webhook;
