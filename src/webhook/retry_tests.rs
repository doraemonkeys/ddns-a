//! Tests for `RetryPolicy`.

use super::RetryPolicy;
use std::time::Duration;

mod retry_policy_defaults {
    use super::*;

    #[test]
    fn new_creates_policy_with_defaults() {
        let policy = RetryPolicy::new();

        assert_eq!(policy.max_attempts, RetryPolicy::DEFAULT_MAX_ATTEMPTS);
        assert_eq!(policy.initial_delay, RetryPolicy::DEFAULT_INITIAL_DELAY);
        assert_eq!(policy.max_delay, RetryPolicy::DEFAULT_MAX_DELAY);
        assert!((policy.multiplier - RetryPolicy::DEFAULT_MULTIPLIER).abs() < f64::EPSILON);
    }

    #[test]
    fn default_trait_matches_new() {
        let from_new = RetryPolicy::new();
        let from_default = RetryPolicy::default();

        assert_eq!(from_new, from_default);
    }

    #[test]
    fn default_max_attempts_is_3() {
        assert_eq!(RetryPolicy::DEFAULT_MAX_ATTEMPTS, 3);
    }

    #[test]
    fn default_initial_delay_is_5_seconds() {
        assert_eq!(RetryPolicy::DEFAULT_INITIAL_DELAY, Duration::from_secs(5));
    }

    #[test]
    fn default_max_delay_is_60_seconds() {
        assert_eq!(RetryPolicy::DEFAULT_MAX_DELAY, Duration::from_secs(60));
    }

    #[test]
    fn default_multiplier_is_2() {
        assert!((RetryPolicy::DEFAULT_MULTIPLIER - 2.0).abs() < f64::EPSILON);
    }
}

mod retry_policy_builder {
    use super::*;

    #[test]
    fn with_max_attempts_sets_value() {
        let policy = RetryPolicy::new().with_max_attempts(5);
        assert_eq!(policy.max_attempts, 5);
    }

    #[test]
    #[should_panic(expected = "max_attempts must be at least 1")]
    fn with_max_attempts_zero_panics() {
        let _ = RetryPolicy::new().with_max_attempts(0);
    }

    #[test]
    fn with_initial_delay_sets_value() {
        let delay = Duration::from_millis(100);
        let policy = RetryPolicy::new().with_initial_delay(delay);
        assert_eq!(policy.initial_delay, delay);
    }

    #[test]
    fn with_max_delay_sets_value() {
        let delay = Duration::from_secs(120);
        let policy = RetryPolicy::new().with_max_delay(delay);
        assert_eq!(policy.max_delay, delay);
    }

    #[test]
    fn with_multiplier_sets_value() {
        let policy = RetryPolicy::new().with_multiplier(1.5);
        assert!((policy.multiplier - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    #[should_panic(expected = "multiplier must be positive")]
    fn with_multiplier_zero_panics() {
        let _ = RetryPolicy::new().with_multiplier(0.0);
    }

    #[test]
    #[should_panic(expected = "multiplier must be positive")]
    fn with_multiplier_negative_panics() {
        let _ = RetryPolicy::new().with_multiplier(-1.0);
    }

    #[test]
    fn builder_chains_correctly() {
        let policy = RetryPolicy::new()
            .with_max_attempts(10)
            .with_initial_delay(Duration::from_millis(500))
            .with_max_delay(Duration::from_secs(30))
            .with_multiplier(3.0);

        assert_eq!(policy.max_attempts, 10);
        assert_eq!(policy.initial_delay, Duration::from_millis(500));
        assert_eq!(policy.max_delay, Duration::from_secs(30));
        assert!((policy.multiplier - 3.0).abs() < f64::EPSILON);
    }
}

mod delay_for_retry {
    use super::*;

    #[test]
    fn first_retry_returns_initial_delay() {
        let policy = RetryPolicy::new().with_initial_delay(Duration::from_secs(5));
        let delay = policy.delay_for_retry(0);
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[test]
    fn second_retry_multiplies_delay() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_secs(5))
            .with_multiplier(2.0);

        let delay = policy.delay_for_retry(1);
        assert_eq!(delay, Duration::from_secs(10));
    }

    #[test]
    fn third_retry_multiplies_again() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_secs(5))
            .with_multiplier(2.0);

        let delay = policy.delay_for_retry(2);
        assert_eq!(delay, Duration::from_secs(20));
    }

    #[test]
    fn delay_is_capped_at_max() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_secs(10))
            .with_max_delay(Duration::from_secs(30))
            .with_multiplier(2.0);

        // Retry 2: 10 * 2^2 = 40 -> capped at 30
        let delay = policy.delay_for_retry(2);
        assert_eq!(delay, Duration::from_secs(30));
    }

    #[test]
    fn large_retry_number_caps_at_max() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(60))
            .with_multiplier(2.0);

        // Retry 10: 1 * 2^10 = 1024 -> capped at 60
        let delay = policy.delay_for_retry(10);
        assert_eq!(delay, Duration::from_secs(60));
    }

    #[test]
    fn multiplier_of_one_keeps_constant_delay() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_secs(5))
            .with_multiplier(1.0);

        for retry in 0..5 {
            assert_eq!(policy.delay_for_retry(retry), Duration::from_secs(5));
        }
    }

    #[test]
    fn fractional_multiplier_works() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_secs(4))
            .with_multiplier(1.5);

        // Retry 0: 4 * 1.5^0 = 4
        assert_eq!(policy.delay_for_retry(0), Duration::from_secs(4));
        // Retry 1: 4 * 1.5^1 = 6
        assert_eq!(policy.delay_for_retry(1), Duration::from_secs(6));
        // Retry 2: 4 * 1.5^2 = 9
        assert_eq!(policy.delay_for_retry(2), Duration::from_secs(9));
    }
}

mod should_retry {
    use super::*;

    #[test]
    fn returns_true_when_under_max_attempts() {
        let policy = RetryPolicy::new().with_max_attempts(3);

        assert!(policy.should_retry(1)); // first attempt
        assert!(policy.should_retry(2)); // second attempt
    }

    #[test]
    fn returns_false_when_at_max_attempts() {
        let policy = RetryPolicy::new().with_max_attempts(3);

        assert!(!policy.should_retry(3));
    }

    #[test]
    fn returns_false_when_over_max_attempts() {
        let policy = RetryPolicy::new().with_max_attempts(3);

        assert!(!policy.should_retry(4));
        assert!(!policy.should_retry(100));
    }

    #[test]
    fn single_attempt_never_retries() {
        let policy = RetryPolicy::new().with_max_attempts(1);

        assert!(!policy.should_retry(1));
    }
}

mod traits {
    use super::*;

    #[test]
    fn clone_creates_independent_copy() {
        let policy1 = RetryPolicy::new().with_max_attempts(5);
        let policy2 = policy1.clone();

        assert_eq!(policy1, policy2);
        assert_eq!(policy2.max_attempts, 5);
    }

    #[test]
    fn partial_eq_compares_all_fields() {
        let policy1 = RetryPolicy::new();
        let policy2 = RetryPolicy::new();
        let policy3 = RetryPolicy::new().with_max_attempts(10);

        assert_eq!(policy1, policy2);
        assert_ne!(policy1, policy3);
    }

    #[test]
    fn debug_format_is_readable() {
        let policy = RetryPolicy::new();
        let debug = format!("{policy:?}");

        assert!(debug.contains("RetryPolicy"));
        assert!(debug.contains("max_attempts"));
        assert!(debug.contains("initial_delay"));
    }
}
