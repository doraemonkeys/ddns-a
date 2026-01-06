//! API listener trait for platform event notifications.
//!
//! This module provides the [`ApiListener`] trait that abstracts platform-specific
//! event notification mechanisms for IP address changes.

use super::ApiError;
use tokio_stream::Stream;

/// Trait for platform-specific IP address change notification APIs.
///
/// Implementations wrap platform APIs like Windows `NotifyIpInterfaceChange`
/// to provide async event streams that signal when IP addresses may have changed.
///
/// # One-time Semantics
///
/// The `into_stream` method consumes `self`, enforcing one-time use.
/// If the underlying API fails, callers should fall back to polling
/// rather than attempting to recreate the listener.
///
/// # Stream Items
///
/// The stream yields `Result<(), ApiError>`:
/// - `Ok(())` - A notification event occurred; caller should re-fetch addresses
/// - `Err(ApiError)` - The listener failed; caller should degrade to polling-only
///
/// # Example
///
/// ```ignore
/// use ddns_a::monitor::{ApiListener, ApiError};
///
/// async fn handle_events<L: ApiListener>(listener: L) {
///     let mut stream = listener.into_stream();
///     while let Some(result) = stream.next().await {
///         match result {
///             Ok(()) => println!("IP change notification received"),
///             Err(e) => {
///                 eprintln!("Listener failed: {e}");
///                 break; // Fall back to polling
///             }
///         }
///     }
/// }
/// ```
pub trait ApiListener: Send {
    /// The stream type returned by `into_stream`.
    type Stream: Stream<Item = Result<(), ApiError>> + Send + Unpin;

    /// Converts this listener into a notification stream.
    ///
    /// Consumes `self` to enforce one-time semantics.
    /// See trait-level documentation for error handling semantics.
    fn into_stream(self) -> Self::Stream;
}
