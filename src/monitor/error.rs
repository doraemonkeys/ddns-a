//! Error types for the monitor layer.

use crate::network::FetchError;
use thiserror::Error;

/// Error type for API event listeners.
///
/// Represents failures in platform-specific event notification APIs.
/// These errors may be recoverable (by falling back to polling mode).
#[derive(Debug, Error)]
pub enum ApiError {
    /// Windows API call failed.
    #[cfg(windows)]
    #[error("Windows API error: {0}")]
    WindowsApi(#[from] windows::core::Error),

    /// The API listener stopped unexpectedly.
    ///
    /// This can happen when the underlying event stream terminates
    /// without explicit shutdown request.
    #[error("Listener stopped unexpectedly")]
    Stopped,
}

/// Error type for monitor operations.
///
/// Describes failures during IP address monitoring.
/// Callers decide recovery strategy based on the error variant.
#[derive(Debug, Error)]
pub enum MonitorError {
    /// Failed to fetch network adapter addresses.
    #[error("Failed to fetch addresses: {0}")]
    Fetch(#[from] FetchError),

    /// The API listener failed.
    ///
    /// This error indicates that the platform event notification API
    /// has failed. The monitor should fall back to polling-only mode.
    #[error("API listener failed: {0}")]
    ApiListenerFailed(#[source] ApiError),
}

#[cfg(test)]
mod tests {
    use super::*;

    mod api_error {
        use super::*;

        #[test]
        fn stopped_displays_message() {
            let error = ApiError::Stopped;
            assert_eq!(error.to_string(), "Listener stopped unexpectedly");
        }

        #[cfg(windows)]
        #[test]
        fn windows_api_error_preserves_source() {
            use windows::core::{Error as WinError, HRESULT};

            // Create a Windows error using a known HRESULT
            let win_error = WinError::from_hresult(HRESULT(-2_147_024_809)); // E_INVALIDARG
            let api_error: ApiError = win_error.into();

            // Verify it's the correct variant and displays properly
            assert!(api_error.to_string().contains("Windows API error"));
        }
    }

    mod monitor_error {
        use super::*;
        use std::error::Error;

        #[test]
        fn fetch_error_displays_with_context() {
            let fetch_error = FetchError::Platform {
                message: "test failure".to_string(),
            };
            let monitor_error = MonitorError::Fetch(fetch_error);

            assert!(monitor_error.to_string().contains("Failed to fetch"));
            assert!(monitor_error.to_string().contains("test failure"));
        }

        #[test]
        fn fetch_error_preserves_source_chain() {
            let fetch_error = FetchError::Platform {
                message: "inner error".to_string(),
            };
            let monitor_error = MonitorError::Fetch(fetch_error);

            let source = monitor_error.source();
            assert!(source.is_some());
            assert!(source.unwrap().to_string().contains("inner error"));
        }

        #[test]
        fn api_listener_failed_displays_with_context() {
            let api_error = ApiError::Stopped;
            let monitor_error = MonitorError::ApiListenerFailed(api_error);

            assert!(monitor_error.to_string().contains("API listener failed"));
        }

        #[test]
        fn api_listener_failed_preserves_source() {
            let api_error = ApiError::Stopped;
            let monitor_error = MonitorError::ApiListenerFailed(api_error);

            let source = monitor_error.source();
            assert!(source.is_some());
            assert!(
                source
                    .unwrap()
                    .to_string()
                    .contains("Listener stopped unexpectedly")
            );
        }

        #[test]
        fn from_fetch_error_conversion() {
            let fetch_error = FetchError::PermissionDenied {
                context: "elevated required".to_string(),
            };
            let monitor_error: MonitorError = fetch_error.into();

            assert!(matches!(monitor_error, MonitorError::Fetch(_)));
        }
    }
}
