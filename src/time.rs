//! Time abstraction for testability.
//!
//! This module provides a [`Clock`] trait that allows injecting mock clocks
//! in tests while using the real system clock in production.

use std::time::SystemTime;

/// Abstraction over system time for testability.
///
/// Implementations provide the current time, allowing tests to inject
/// controlled time values instead of relying on actual system time.
///
/// # Example
///
/// ```
/// use ddns_a::time::{Clock, SystemClock};
///
/// let clock = SystemClock;
/// let now = clock.now();
/// assert!(now >= std::time::SystemTime::UNIX_EPOCH);
/// ```
pub trait Clock: Send + Sync {
    /// Returns the current time.
    fn now(&self) -> SystemTime;
}

/// Production clock using actual system time.
///
/// This is the default clock implementation that delegates to
/// [`SystemTime::now()`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    /// A mock clock for testing that returns controlled time values.
    struct MockClock {
        /// Seconds since `UNIX_EPOCH`, atomically updated.
        secs: AtomicU64,
    }

    impl MockClock {
        fn new(initial_secs: u64) -> Self {
            Self {
                secs: AtomicU64::new(initial_secs),
            }
        }

        fn advance(&self, secs: u64) {
            self.secs.fetch_add(secs, Ordering::SeqCst);
        }
    }

    impl Clock for MockClock {
        fn now(&self) -> SystemTime {
            SystemTime::UNIX_EPOCH + Duration::from_secs(self.secs.load(Ordering::SeqCst))
        }
    }

    #[test]
    fn system_clock_returns_current_time() {
        let clock = SystemClock;
        let before = SystemTime::now();
        let result = clock.now();
        let after = SystemTime::now();

        assert!(result >= before);
        assert!(result <= after);
    }

    #[test]
    fn system_clock_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SystemClock>();
    }

    fn assert_default<T: Default>() {}

    #[test]
    fn system_clock_is_default() {
        assert_default::<SystemClock>();
    }

    #[test]
    fn system_clock_is_copy() {
        let clock1 = SystemClock;
        let clock2 = clock1;
        // Both are usable (Copy semantics)
        let _ = clock1.now();
        let _ = clock2.now();
    }

    #[test]
    fn mock_clock_returns_controlled_time() {
        let clock = MockClock::new(1_000_000);
        let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);

        assert_eq!(clock.now(), expected);
    }

    #[test]
    fn mock_clock_can_advance() {
        let clock = MockClock::new(0);

        assert_eq!(clock.now(), SystemTime::UNIX_EPOCH);

        clock.advance(100);
        assert_eq!(
            clock.now(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(100)
        );

        clock.advance(50);
        assert_eq!(
            clock.now(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(150)
        );
    }

    #[test]
    fn mock_clock_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockClock>();
    }
}
