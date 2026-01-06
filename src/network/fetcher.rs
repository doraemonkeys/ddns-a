//! Address fetching trait and error types.

use super::AdapterSnapshot;
use thiserror::Error;

/// Error type for address fetching operations.
///
/// Describes what went wrong without dictating recovery strategy.
/// Callers decide how to handle each error variant.
#[derive(Debug, Error)]
pub enum FetchError {
    /// Windows API call failed.
    #[cfg(windows)]
    #[error("Windows API error: {0}")]
    WindowsApi(#[from] windows::core::Error),

    /// Permission denied to access network information.
    #[error("Permission denied: {context}")]
    PermissionDenied {
        /// Additional context about what permission was denied.
        context: String,
    },

    /// Platform-specific error with a generic message.
    #[error("Platform error: {message}")]
    Platform {
        /// Error message describing the platform-specific failure.
        message: String,
    },
}

/// Trait for fetching network adapter address information.
///
/// # Design
///
/// - All external dependencies should implement this trait
/// - Enables dependency injection for testing with mock implementations
/// - Platform-specific implementations provided in submodules
///
/// # Example
///
/// ```ignore
/// use ddns_a::network::{AddressFetcher, AdapterSnapshot};
///
/// struct MockFetcher {
///     snapshots: Vec<Vec<AdapterSnapshot>>,
///     call_count: std::sync::atomic::AtomicUsize,
/// }
///
/// impl AddressFetcher for MockFetcher {
///     fn fetch(&self) -> Result<Vec<AdapterSnapshot>, FetchError> {
///         let idx = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
///         Ok(self.snapshots.get(idx).cloned().unwrap_or_default())
///     }
/// }
/// ```
pub trait AddressFetcher: Send + Sync {
    /// Fetches the current state of all network adapters.
    ///
    /// # Returns
    ///
    /// A vector of [`AdapterSnapshot`] representing all adapters on the system,
    /// or a [`FetchError`] if the operation fails.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError`] when:
    /// - Platform API calls fail (e.g., `FetchError::WindowsApi` on Windows)
    /// - Insufficient permissions to access network information (`FetchError::PermissionDenied`)
    /// - Other platform-specific failures (`FetchError::Platform`)
    ///
    /// # Implementation Notes
    ///
    /// - Implementations should return ALL adapters; filtering is done by the caller
    /// - Address order within each adapter should be stable across calls
    /// - This is a synchronous operation; async wrappers can be added if needed
    fn fetch(&self) -> Result<Vec<AdapterSnapshot>, FetchError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{AdapterKind, AdapterSnapshot};
    use std::sync::Mutex;

    /// A mock fetcher for testing that returns predefined snapshots.
    ///
    /// Uses `Mutex<VecDeque>` to avoid requiring `Clone` on `FetchError`.
    struct MockFetcher {
        results: Mutex<std::collections::VecDeque<Result<Vec<AdapterSnapshot>, FetchError>>>,
    }

    impl MockFetcher {
        fn new(results: Vec<Result<Vec<AdapterSnapshot>, FetchError>>) -> Self {
            Self {
                results: Mutex::new(results.into()),
            }
        }

        fn returning_snapshots(snapshots: Vec<Vec<AdapterSnapshot>>) -> Self {
            Self::new(snapshots.into_iter().map(Ok).collect())
        }
    }

    impl AddressFetcher for MockFetcher {
        fn fetch(&self) -> Result<Vec<AdapterSnapshot>, FetchError> {
            self.results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(vec![]))
        }
    }

    #[test]
    fn mock_fetcher_returns_predefined_snapshots() {
        let snapshot = AdapterSnapshot::new(
            "eth0",
            AdapterKind::Ethernet,
            vec!["192.168.1.1".parse().unwrap()],
            vec![],
        );
        let fetcher = MockFetcher::returning_snapshots(vec![vec![snapshot.clone()]]);

        let result = fetcher.fetch().unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], snapshot);
    }

    #[test]
    fn mock_fetcher_returns_different_results_on_each_call() {
        let snapshot1 = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
        let snapshot2 = AdapterSnapshot::new("eth1", AdapterKind::Wireless, vec![], vec![]);

        let fetcher = MockFetcher::returning_snapshots(vec![vec![snapshot1], vec![snapshot2]]);

        let result1 = fetcher.fetch().unwrap();
        let result2 = fetcher.fetch().unwrap();

        assert_eq!(result1[0].name, "eth0");
        assert_eq!(result2[0].name, "eth1");
    }

    #[test]
    fn mock_fetcher_returns_empty_after_exhausting_results() {
        let fetcher = MockFetcher::returning_snapshots(vec![vec![]]);

        let _ = fetcher.fetch(); // First call
        let result = fetcher.fetch().unwrap(); // Second call - should return empty

        assert!(result.is_empty());
    }

    #[test]
    fn mock_fetcher_can_return_errors() {
        let fetcher = MockFetcher::new(vec![Err(FetchError::Platform {
            message: "test error".to_string(),
        })]);

        let result = fetcher.fetch();

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("test error"));
    }

    #[test]
    fn fetch_error_permission_denied_displays_context() {
        let error = FetchError::PermissionDenied {
            context: "elevated privileges required".to_string(),
        };
        assert!(error.to_string().contains("elevated privileges required"));
    }

    #[test]
    fn fetch_error_platform_displays_message() {
        let error = FetchError::Platform {
            message: "unsupported operation".to_string(),
        };
        assert!(error.to_string().contains("unsupported operation"));
    }
}
