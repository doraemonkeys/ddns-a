//! Error types for HTTP and webhook operations.

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

/// Error type for operations that may be retried.
///
/// Wraps both network-level failures and HTTP status errors that
/// indicate transient conditions (server overload, rate limiting, etc.).
///
/// **Note**: Despite the name, not all variants are inherently retryable.
/// Use [`IsRetryable::is_retryable()`] to determine if a specific error
/// should be retried.
#[derive(Debug, Error)]
pub enum RetryableError {
    /// Network-level error during the HTTP request.
    #[error(transparent)]
    Http(#[from] HttpError),

    /// Server returned a non-success (non-2xx) status code.
    ///
    /// **Important**: This variant is not always retryable. Whether to retry
    /// depends on the specific status code:
    /// - 5xx (server errors): typically retryable
    /// - 429 (Too Many Requests): retryable
    /// - 408 (Request Timeout): retryable
    /// - Other 4xx (client errors): not retryable (configuration/request issue)
    ///
    /// Use [`IsRetryable::is_retryable()`] to check.
    #[error("HTTP {}: {}", status.as_u16(), body.as_deref().unwrap_or("<no body>"))]
    NonSuccessStatus {
        /// HTTP status code
        status: http::StatusCode,
        /// Optional response body for diagnostics
        body: Option<String>,
    },

    /// Template rendering failed.
    ///
    /// The body template could not be rendered with the provided data.
    #[error("Template error: {0}")]
    Template(String),
}

/// High-level error type for webhook operations.
///
/// Distinguishes between errors that can be retried and terminal failures
/// where all retry attempts have been exhausted.
#[derive(Debug, Error)]
pub enum WebhookError {
    /// A single attempt failed but may be retried.
    #[error(transparent)]
    Retryable(#[from] RetryableError),

    /// All retry attempts have been exhausted.
    #[error("Failed after {attempts} attempts")]
    MaxRetriesExceeded {
        /// Number of attempts made before giving up
        attempts: u32,
        /// The last error encountered
        #[source]
        last_error: RetryableError,
    },
}
