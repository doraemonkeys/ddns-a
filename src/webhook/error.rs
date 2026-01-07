//! Error types for HTTP operations.

use thiserror::Error;

/// Error type for HTTP operations.
///
/// Describes what went wrong without dictating recovery strategy.
/// These errors are typically retryable at the caller's discretion.
#[derive(Debug, Error)]
pub enum HttpError {
    /// Network connection failed.
    ///
    /// This includes DNS resolution failures, connection refused,
    /// and other network-level errors.
    #[error("Connection error: {0}")]
    Connection(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// Request timed out.
    ///
    /// The server did not respond within the configured timeout period.
    #[error("Request timed out")]
    Timeout,

    /// The provided URL is invalid.
    ///
    /// This typically indicates a configuration error rather than
    /// a transient failure.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}
