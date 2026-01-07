//! Webhook layer for sending HTTP requests to external services.
//!
//! This module provides types and traits for:
//! - Building HTTP requests ([`HttpRequest`])
//! - Handling HTTP responses ([`HttpResponse`])
//! - Abstracting HTTP clients ([`HttpClient`])
//! - Production HTTP client implementation ([`ReqwestClient`])

mod client;
mod error;
mod http;

#[cfg(test)]
mod client_test;
#[cfg(test)]
mod http_test;

pub use client::ReqwestClient;
pub use error::HttpError;
pub use http::{HttpClient, HttpRequest, HttpResponse};
