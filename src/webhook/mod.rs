//! Webhook layer for sending HTTP requests to external services.
//!
//! This module provides types and traits for:
//! - Building HTTP requests ([`HttpRequest`])
//! - Handling HTTP responses ([`HttpResponse`])
//! - Abstracting HTTP clients ([`HttpClient`])
//! - Production HTTP client implementation ([`ReqwestClient`])
//! - Webhook sending with retries ([`WebhookSender`], [`HttpWebhook`])
//! - Retry policy configuration ([`RetryPolicy`])

mod client;
mod error;
mod http;
mod retry;
mod sender;

#[cfg(test)]
mod client_test;
#[cfg(test)]
mod http_test;
#[cfg(test)]
mod retry_tests;
#[cfg(test)]
mod sender_tests;

pub use client::ReqwestClient;
pub use error::{HttpError, RetryableError, WebhookError};
pub use http::{HttpClient, HttpRequest, HttpResponse};
pub use retry::RetryPolicy;
pub use sender::{HttpWebhook, IsRetryable, WebhookSender};
