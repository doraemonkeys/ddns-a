//! File-based state persistence implementation.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::network::AdapterSnapshot;

use super::{LoadResult, StateError, StateStore};

/// Current state file format version.
///
/// Increment this when making breaking changes to the format.
const STATE_FILE_VERSION: u32 = 1;

/// On-disk state file format.
///
/// Uses JSON for readability and debugging. The `version` field allows
/// future format migrations, though the current policy is to treat
/// incompatible versions as corrupted (no backward compatibility).
#[derive(Debug, Serialize, Deserialize)]
struct StateFile {
    /// Format version for future compatibility.
    version: u32,

    /// Unix timestamp when the state was saved.
    /// For debugging purposes only; not used in logic.
    #[serde(skip_serializing_if = "Option::is_none")]
    saved_at: Option<String>,

    /// The saved adapter snapshots.
    snapshots: Vec<AdapterSnapshot>,
}

impl StateFile {
    /// Creates a new state file with the given snapshots.
    fn new(snapshots: &[AdapterSnapshot]) -> Self {
        Self {
            version: STATE_FILE_VERSION,
            saved_at: Some(unix_timestamp_now()),
            snapshots: snapshots.to_vec(),
        }
    }
}

/// Returns the current Unix timestamp as a string.
///
/// Uses Unix timestamp for simplicity and unambiguity in debugging.
fn unix_timestamp_now() -> String {
    use std::time::SystemTime;

    let now = SystemTime::now();
    let duration = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();

    format!("{}", duration.as_secs())
}

/// File-based implementation of [`StateStore`].
///
/// Stores adapter snapshots as JSON files with atomic write semantics.
///
/// # Atomic Writes
///
/// Uses write-to-temp-then-rename pattern to prevent corruption:
/// 1. Write to `{path}.tmp`
/// 2. Rename `{path}.tmp` to `{path}`
///
/// This ensures the file is either fully written or not written at all.
#[derive(Debug, Clone)]
pub struct FileStateStore {
    path: PathBuf,
}

impl FileStateStore {
    /// Creates a new file-based state store at the given path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the path to the state file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Performs the blocking save operation.
    ///
    /// Separated out so it can be wrapped in `spawn_blocking`.
    fn save_blocking(path: &Path, state: &StateFile) -> Result<(), StateError> {
        let content = serde_json::to_string_pretty(state).map_err(StateError::Serialize)?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(StateError::Write)?;
            }
        }

        // Append .tmp instead of replacing extension to avoid conflicts
        // (e.g., state.json -> state.json.tmp, not state.tmp)
        let temp_path = PathBuf::from(format!("{}.tmp", path.display()));

        // Write to temp file
        std::fs::write(&temp_path, content).map_err(StateError::Write)?;

        // Atomic rename (on most filesystems)
        std::fs::rename(&temp_path, path).map_err(StateError::Write)?;

        Ok(())
    }
}

impl StateStore for FileStateStore {
    fn load(&self) -> LoadResult {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == ErrorKind::NotFound => return LoadResult::NotFound,
            Err(e) => {
                return LoadResult::Corrupted {
                    reason: format!("Failed to read file: {e}"),
                };
            }
        };

        match serde_json::from_str::<StateFile>(&content) {
            Ok(state) => {
                // Check version compatibility
                if state.version != STATE_FILE_VERSION {
                    return LoadResult::Corrupted {
                        reason: format!(
                            "Incompatible version: expected {STATE_FILE_VERSION}, got {}",
                            state.version
                        ),
                    };
                }
                LoadResult::Loaded(state.snapshots)
            }
            Err(e) => LoadResult::Corrupted {
                reason: format!("Invalid JSON: {e}"),
            },
        }
    }

    async fn save(&self, snapshots: &[AdapterSnapshot]) -> Result<(), StateError> {
        let path = self.path.clone();
        let state = StateFile::new(snapshots);

        // Use spawn_blocking to avoid blocking the async runtime
        tokio::task::spawn_blocking(move || Self::save_blocking(&path, &state))
            .await
            .expect("spawn_blocking task panicked")
    }
}
