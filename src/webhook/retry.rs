//! Retry policy configuration for webhook operations.

use std::time::Duration;

/// Configuration for exponential backoff retry behavior.
///
/// Controls how many times to retry a failed operation and how long
/// to wait between attempts. Uses exponential backoff with a configurable
/// multiplier and maximum delay cap.
///
/// # Defaults
///
/// - `max_attempts`: 3
/// - `initial_delay`: 5 seconds
/// - `max_delay`: 60 seconds
/// - `multiplier`: 2.0
///
/// # Example
///
/// ```
/// use ddns_a::webhook::RetryPolicy;
/// use std::time::Duration;
///
/// // Create with defaults
/// let policy = RetryPolicy::default();
///
/// // Or customize via builder
/// let custom = RetryPolicy::new()
///     .with_max_attempts(5)
///     .with_initial_delay(Duration::from_secs(1))
///     .with_max_delay(Duration::from_secs(30))
///     .with_multiplier(1.5);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the initial attempt).
    ///
    /// A value of 1 means no retries; only the initial attempt is made.
    pub max_attempts: u32,

    /// Delay before the first retry.
    ///
    /// Subsequent delays are computed by multiplying by `multiplier`.
    pub initial_delay: Duration,

    /// Maximum delay between retries.
    ///
    /// The computed delay is capped at this value to prevent
    /// excessively long waits.
    pub max_delay: Duration,

    /// Multiplier applied to the delay after each retry.
    ///
    /// A value of 2.0 doubles the delay each time.
    pub multiplier: f64,
}

impl RetryPolicy {
    /// Default maximum attempts.
    pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

    /// Default initial delay (5 seconds).
    pub const DEFAULT_INITIAL_DELAY: Duration = Duration::from_secs(5);

    /// Default maximum delay (60 seconds).
    pub const DEFAULT_MAX_DELAY: Duration = Duration::from_secs(60);

    /// Default multiplier (2.0).
    pub const DEFAULT_MULTIPLIER: f64 = 2.0;

    /// Creates a new retry policy with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_attempts: Self::DEFAULT_MAX_ATTEMPTS,
            initial_delay: Self::DEFAULT_INITIAL_DELAY,
            max_delay: Self::DEFAULT_MAX_DELAY,
            multiplier: Self::DEFAULT_MULTIPLIER,
        }
    }

    /// Minimum value for `max_attempts`.
    pub const MIN_MAX_ATTEMPTS: u32 = 1;

    /// Sets the maximum number of attempts.
    ///
    /// # Panics
    ///
    /// Panics if `max_attempts` is less than 1.
    #[must_use]
    pub const fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        assert!(
            max_attempts >= Self::MIN_MAX_ATTEMPTS,
            "max_attempts must be at least 1"
        );
        self.max_attempts = max_attempts;
        self
    }

    /// Sets the initial delay between retries.
    ///
    /// Zero delay is supported (useful for testing with [`InstantSleeper`])
    /// but not recommended for production as it creates a tight retry loop.
    ///
    /// [`InstantSleeper`]: crate::time::InstantSleeper
    #[must_use]
    pub const fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Sets the maximum delay between retries.
    #[must_use]
    pub const fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Sets the delay multiplier.
    ///
    /// # Panics
    ///
    /// Panics if `multiplier` is not positive (must be > 0.0).
    #[must_use]
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        assert!(multiplier > 0.0, "multiplier must be positive");
        self.multiplier = multiplier;
        self
    }

    /// Computes the delay for a given retry number (0-indexed).
    ///
    /// # Arguments
    ///
    /// * `retry` - The retry number (0 = delay before first retry, 1 = delay before second retry, etc.)
    ///
    /// # Returns
    ///
    /// The delay to wait before this retry, capped at `max_delay`.
    #[must_use]
    pub fn delay_for_retry(&self, retry: u32) -> Duration {
        // Safe cast: retry values are small (typically < 20) and i32::MAX is ~2 billion
        #[allow(clippy::cast_possible_wrap)]
        let multiplier = self.multiplier.powi(retry as i32);
        let delay_secs = self.initial_delay.as_secs_f64() * multiplier;
        let capped = delay_secs.min(self.max_delay.as_secs_f64());
        Duration::from_secs_f64(capped)
    }

    /// Returns true if the given attempt number should be retried.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The attempt number (1 = first attempt, 2 = first retry, etc.)
    ///
    /// # Returns
    ///
    /// `true` if the attempt is within the allowed number of attempts.
    #[must_use]
    pub const fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}
