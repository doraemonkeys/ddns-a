//! Default values for configuration options.
//!
//! Centralized constants to avoid magic numbers scattered across the codebase.

use std::time::Duration;

/// Default HTTP method for webhook requests.
pub const METHOD: &str = "POST";

/// Default polling interval in seconds.
pub const POLL_INTERVAL_SECS: u64 = 60;

/// Default maximum number of retry attempts.
pub const RETRY_MAX_ATTEMPTS: u32 = 3;

/// Default initial retry delay in seconds.
pub const RETRY_INITIAL_DELAY_SECS: u64 = 5;

/// Default maximum retry delay in seconds.
pub const RETRY_MAX_DELAY_SECS: u64 = 60;

/// Default retry backoff multiplier.
pub const RETRY_MULTIPLIER: f64 = 2.0;

/// Default polling interval as Duration.
#[must_use]
pub const fn poll_interval() -> Duration {
    Duration::from_secs(POLL_INTERVAL_SECS)
}

/// Default initial retry delay as Duration.
#[must_use]
pub const fn retry_initial_delay() -> Duration {
    Duration::from_secs(RETRY_INITIAL_DELAY_SECS)
}

/// Default maximum retry delay as Duration.
#[must_use]
pub const fn retry_max_delay() -> Duration {
    Duration::from_secs(RETRY_MAX_DELAY_SECS)
}
