//! Debounce policy for event stream processing.

use std::time::Duration;

/// Policy for debouncing IP change events.
///
/// Debouncing merges changes that occur within a time window, avoiding
/// rapid consecutive triggers (flapping) from causing duplicate notifications.
///
/// # Merge Semantics
///
/// | Scenario | Event Sequence in Window | Output | Reason |
/// |----------|--------------------------|--------|--------|
/// | Flicker | `Added(IP) → Removed(IP)` | Empty | Same IP add/remove cancel out |
/// | Reverse Flicker | `Removed(IP) → Added(IP)` | Empty | Same IP remove/add cancel out |
/// | Replacement | `Removed(old) → Added(new)` | Both events | Different IPs, independent |
/// | Duplicate Add | `Added(IP) → Added(IP)` | One Added | Idempotent merge |
///
/// # Implementation
///
/// At window end, compute net change for each (adapter, address):
/// - Net > 0: Output `Added`
/// - Net < 0: Output `Removed`
/// - Net = 0: No output (cancelled out)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebouncePolicy {
    /// The debounce window duration.
    ///
    /// Changes occurring within this window are merged before emission.
    window: Duration,
}

impl DebouncePolicy {
    /// Creates a new debounce policy with the specified window duration.
    #[must_use]
    pub const fn new(window: Duration) -> Self {
        Self { window }
    }

    /// Returns the debounce window duration.
    #[must_use]
    pub const fn window(&self) -> Duration {
        self.window
    }
}

impl Default for DebouncePolicy {
    /// Creates a default debounce policy with a 2-second window.
    ///
    /// The 2-second default balances responsiveness with protection
    /// against rapid changes during network configuration updates.
    fn default() -> Self {
        Self {
            window: Duration::from_secs(2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_is_two_seconds() {
        let policy = DebouncePolicy::default();
        assert_eq!(policy.window(), Duration::from_secs(2));
    }

    #[test]
    fn new_creates_with_specified_window() {
        let policy = DebouncePolicy::new(Duration::from_millis(500));
        assert_eq!(policy.window(), Duration::from_millis(500));
    }

    #[test]
    fn window_accessor_returns_duration() {
        let policy = DebouncePolicy::new(Duration::from_secs(5));
        assert_eq!(policy.window(), Duration::from_secs(5));
    }

    #[test]
    fn equality_based_on_window() {
        let policy1 = DebouncePolicy::new(Duration::from_secs(1));
        let policy2 = DebouncePolicy::new(Duration::from_secs(1));
        let policy3 = DebouncePolicy::new(Duration::from_secs(2));

        assert_eq!(policy1, policy2);
        assert_ne!(policy1, policy3);
    }

    #[test]
    fn clone_creates_identical_policy() {
        let original = DebouncePolicy::new(Duration::from_millis(100));
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn debug_format_includes_window() {
        let policy = DebouncePolicy::new(Duration::from_secs(3));
        let debug_str = format!("{policy:?}");

        assert!(debug_str.contains("DebouncePolicy"));
        assert!(debug_str.contains("window"));
    }
}
