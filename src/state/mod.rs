//! IP state persistence for detecting changes across restarts.
//!
//! This module provides abstractions for storing and retrieving
//! adapter snapshot state between program executions.

mod file;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

pub use file::FileStateStore;

use std::io;

use thiserror::Error;

use crate::network::AdapterSnapshot;

/// Result of loading state from persistent storage.
///
/// Explicitly models all valid states to avoid ambiguity:
/// - Successfully loaded previous state
/// - No previous state exists (first run)
/// - State exists but is corrupted/unreadable
#[derive(Debug, Clone)]
pub enum LoadResult {
    /// Successfully loaded previously saved snapshots.
    Loaded(Vec<AdapterSnapshot>),

    /// No state file exists (first run or explicitly deleted).
    NotFound,

    /// State file exists but could not be parsed.
    /// Program should continue with fresh state and overwrite on next save.
    Corrupted {
        /// Reason for corruption (for logging/debugging).
        reason: String,
    },
}

impl LoadResult {
    /// Returns the loaded snapshots, or an empty vec for `NotFound`/`Corrupted`.
    #[must_use]
    pub fn into_snapshots(self) -> Vec<AdapterSnapshot> {
        match self {
            Self::Loaded(snapshots) => snapshots,
            Self::NotFound | Self::Corrupted { .. } => Vec::new(),
        }
    }

    /// Returns `true` if state was successfully loaded.
    #[must_use]
    pub const fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded(_))
    }
}

/// Errors that can occur during state persistence operations.
///
/// Only covers write-side errors; read-side issues are modeled
/// as [`LoadResult`] variants to allow graceful degradation.
#[derive(Debug, Error)]
pub enum StateError {
    /// Failed to write the state file.
    #[error("Failed to write state file: {0}")]
    Write(#[source] io::Error),

    /// Failed to serialize state to JSON.
    #[error("Failed to serialize state: {0}")]
    Serialize(#[source] serde_json::Error),
}

/// Abstraction for persisting adapter state between program runs.
///
/// Implementations should:
/// - Use atomic writes to prevent corruption from crashes
/// - Handle missing files gracefully (return `LoadResult::NotFound`)
/// - Degrade gracefully on read errors (return `LoadResult::Corrupted`)
///
/// # Testing
///
/// Use [`MockStateStore`] in tests to avoid filesystem dependencies.
pub trait StateStore: Send + Sync {
    /// Loads previously saved state.
    ///
    /// Returns one of:
    /// - `LoadResult::Loaded` - State was successfully loaded
    /// - `LoadResult::NotFound` - No state file exists
    /// - `LoadResult::Corrupted` - State file exists but is invalid
    fn load(&self) -> LoadResult;

    /// Saves current adapter state for future reference.
    ///
    /// Implementations should use atomic write semantics (write to temp file,
    /// then rename) to prevent corruption if the program crashes mid-write.
    ///
    /// # Errors
    ///
    /// Returns an error if the state cannot be written.
    fn save(
        &self,
        snapshots: &[AdapterSnapshot],
    ) -> impl std::future::Future<Output = Result<(), StateError>> + Send;
}

/// Mock state store for testing.
///
/// Allows tests to inject specific load results and capture saved state.
#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::RwLock;

    /// A mock implementation of [`StateStore`] for testing.
    #[derive(Debug)]
    pub struct MockStateStore {
        load_result: LoadResult,
        saved: RwLock<Option<Vec<AdapterSnapshot>>>,
    }

    impl MockStateStore {
        /// Creates a mock that returns `LoadResult::Loaded` with the given snapshots.
        #[must_use]
        pub fn with_loaded(snapshots: Vec<AdapterSnapshot>) -> Self {
            Self {
                load_result: LoadResult::Loaded(snapshots),
                saved: RwLock::new(None),
            }
        }

        /// Creates a mock that returns `LoadResult::NotFound`.
        #[must_use]
        pub fn not_found() -> Self {
            Self {
                load_result: LoadResult::NotFound,
                saved: RwLock::new(None),
            }
        }

        /// Creates a mock that returns `LoadResult::Corrupted`.
        #[must_use]
        pub fn corrupted(reason: impl Into<String>) -> Self {
            Self {
                load_result: LoadResult::Corrupted {
                    reason: reason.into(),
                },
                saved: RwLock::new(None),
            }
        }

        /// Returns the last saved snapshots, if any.
        ///
        /// # Panics
        ///
        /// Panics if the internal lock is poisoned (only in test code).
        #[must_use]
        pub fn saved_snapshots(&self) -> Option<Vec<AdapterSnapshot>> {
            self.saved.read().unwrap().clone()
        }
    }

    impl StateStore for MockStateStore {
        fn load(&self) -> LoadResult {
            self.load_result.clone()
        }

        async fn save(&self, snapshots: &[AdapterSnapshot]) -> Result<(), StateError> {
            *self.saved.write().unwrap() = Some(snapshots.to_vec());
            Ok(())
        }
    }
}
