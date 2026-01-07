//! Time abstraction for testability.
//!
//! This module provides a [`Clock`] trait that allows injecting mock clocks
//! in tests while using the real system clock in production, and a [`Sleeper`]
//! trait for injectable async delays.

use std::time::{Duration, SystemTime};

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

/// Abstraction over async sleep for testability.
///
/// Implementations provide async delay functionality, allowing tests to
/// inject instant/mock sleeps instead of waiting for real time.
///
/// # Example
///
/// ```
/// use ddns_a::time::{Sleeper, TokioSleeper};
/// use std::time::Duration;
///
/// async fn example() {
///     let sleeper = TokioSleeper;
///     sleeper.sleep(Duration::from_millis(100)).await;
/// }
/// ```
pub trait Sleeper: Send + Sync {
    /// Sleeps for the specified duration.
    fn sleep(&self, duration: Duration) -> impl std::future::Future<Output = ()> + Send;
}

/// Production sleeper using tokio's sleep.
///
/// This is the default sleeper implementation that delegates to
/// [`tokio::time::sleep`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Mock sleeper that returns immediately without waiting.
///
/// Useful for testing retry logic without actual delays.
#[derive(Debug, Clone, Copy, Default)]
pub struct InstantSleeper;

impl Sleeper for InstantSleeper {
    async fn sleep(&self, _duration: Duration) {
        // Return immediately - no actual sleep
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

    // Sleeper tests

    #[tokio::test]
    async fn tokio_sleeper_completes() {
        let sleeper = TokioSleeper;
        // Very short sleep to verify it works
        sleeper.sleep(Duration::from_millis(1)).await;
    }

    #[test]
    fn tokio_sleeper_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TokioSleeper>();
    }

    #[test]
    fn tokio_sleeper_is_default() {
        assert_default::<TokioSleeper>();
    }

    #[test]
    fn tokio_sleeper_is_copy() {
        let sleeper1 = TokioSleeper;
        let sleeper2 = sleeper1;
        // Both are usable (Copy semantics)
        let _ = sleeper1;
        let _ = sleeper2;
    }

    #[tokio::test]
    async fn instant_sleeper_returns_immediately() {
        let sleeper = InstantSleeper;
        let start = std::time::Instant::now();
        sleeper.sleep(Duration::from_secs(1000)).await;
        // Should complete almost instantly
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn instant_sleeper_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<InstantSleeper>();
    }

    #[test]
    fn instant_sleeper_is_default() {
        assert_default::<InstantSleeper>();
    }

    #[test]
    fn instant_sleeper_is_copy() {
        let sleeper1 = InstantSleeper;
        let sleeper2 = sleeper1;
        // Both are usable (Copy semantics)
        let _ = sleeper1;
        let _ = sleeper2;
    }
}
