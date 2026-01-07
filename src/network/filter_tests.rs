//! Tests for the adapter filtering module.

use super::filter::*;
use super::{AdapterKind, AdapterSnapshot};

// ============================================================================
// Test Fixtures
// ============================================================================

fn ethernet_adapter() -> AdapterSnapshot {
    AdapterSnapshot::new(
        "Ethernet",
        AdapterKind::Ethernet,
        vec!["192.168.1.1".parse().unwrap()],
        vec![],
    )
}

fn wifi_adapter() -> AdapterSnapshot {
    AdapterSnapshot::new(
        "Wi-Fi",
        AdapterKind::Wireless,
        vec!["192.168.1.2".parse().unwrap()],
        vec![],
    )
}

fn virtual_adapter() -> AdapterSnapshot {
    AdapterSnapshot::new(
        "vEthernet (WSL)",
        AdapterKind::Virtual,
        vec!["172.17.0.1".parse().unwrap()],
        vec![],
    )
}

fn loopback_adapter() -> AdapterSnapshot {
    AdapterSnapshot::new(
        "Loopback Pseudo-Interface",
        AdapterKind::Loopback,
        vec!["127.0.0.1".parse().unwrap()],
        vec!["::1".parse().unwrap()],
    )
}

fn docker_adapter() -> AdapterSnapshot {
    AdapterSnapshot::new(
        "Docker Network Adapter",
        AdapterKind::Virtual,
        vec!["172.18.0.1".parse().unwrap()],
        vec![],
    )
}

// ============================================================================
// FilterMode Tests
// ============================================================================

mod filter_mode {
    use super::*;

    #[test]
    fn include_and_exclude_are_distinct() {
        assert_ne!(FilterMode::Include, FilterMode::Exclude);
    }

    #[test]
    fn debug_impl_works() {
        assert!(!format!("{:?}", FilterMode::Include).is_empty());
        assert!(!format!("{:?}", FilterMode::Exclude).is_empty());
    }

    #[test]
    fn clone_works() {
        let mode = FilterMode::Include;
        #[allow(clippy::clone_on_copy)]
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }
}

// ============================================================================
// NameRegexFilter Tests
// ============================================================================

mod name_regex_filter {
    use super::*;

    #[test]
    fn include_mode_matches_when_pattern_matches() {
        let filter = NameRegexFilter::include(r"^Ethernet").unwrap();
        assert!(filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn include_mode_rejects_when_pattern_does_not_match() {
        let filter = NameRegexFilter::include(r"^Ethernet").unwrap();
        assert!(!filter.matches(&wifi_adapter()));
    }

    #[test]
    fn exclude_mode_rejects_when_pattern_matches() {
        let filter = NameRegexFilter::exclude(r"^vEthernet").unwrap();
        assert!(!filter.matches(&virtual_adapter()));
    }

    #[test]
    fn exclude_mode_matches_when_pattern_does_not_match() {
        let filter = NameRegexFilter::exclude(r"^vEthernet").unwrap();
        assert!(filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn partial_match_works() {
        let filter = NameRegexFilter::include(r"Wi").unwrap();
        assert!(filter.matches(&wifi_adapter()));
    }

    #[test]
    fn case_sensitive_by_default() {
        let filter = NameRegexFilter::include(r"ethernet").unwrap();
        assert!(!filter.matches(&ethernet_adapter())); // "Ethernet" has capital E
    }

    #[test]
    fn case_insensitive_with_flag() {
        let filter = NameRegexFilter::include(r"(?i)ethernet").unwrap();
        assert!(filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn invalid_regex_returns_error() {
        let result = NameRegexFilter::include(r"[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn new_with_include_mode() {
        let filter = NameRegexFilter::new(r"test", FilterMode::Include).unwrap();
        assert_eq!(filter.mode(), FilterMode::Include);
    }

    #[test]
    fn new_with_exclude_mode() {
        let filter = NameRegexFilter::new(r"test", FilterMode::Exclude).unwrap();
        assert_eq!(filter.mode(), FilterMode::Exclude);
    }

    #[test]
    fn pattern_accessor_returns_regex() {
        let filter = NameRegexFilter::include(r"^eth\d+").unwrap();
        assert!(filter.pattern().is_match("eth0"));
        assert!(!filter.pattern().is_match("wlan0"));
    }

    #[test]
    fn debug_impl_works() {
        let filter = NameRegexFilter::include(r"test").unwrap();
        let debug_str = format!("{filter:?}");
        assert!(debug_str.contains("NameRegexFilter"));
    }

    #[test]
    fn exclude_docker_adapters() {
        let filter = NameRegexFilter::exclude(r"(?i)docker").unwrap();
        assert!(!filter.matches(&docker_adapter()));
        assert!(filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn complex_pattern_with_alternation() {
        let filter = NameRegexFilter::exclude(r"(?i)(docker|vmware|virtualbox)").unwrap();
        assert!(!filter.matches(&docker_adapter()));
        assert!(filter.matches(&ethernet_adapter()));
        assert!(filter.matches(&wifi_adapter()));
    }
}

// ============================================================================
// ExcludeVirtualFilter Tests
// ============================================================================

mod exclude_virtual_filter {
    use super::*;

    #[test]
    fn excludes_virtual_adapters() {
        let filter = ExcludeVirtualFilter;
        assert!(!filter.matches(&virtual_adapter()));
    }

    #[test]
    fn includes_ethernet_adapters() {
        let filter = ExcludeVirtualFilter;
        assert!(filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn includes_wireless_adapters() {
        let filter = ExcludeVirtualFilter;
        assert!(filter.matches(&wifi_adapter()));
    }

    #[test]
    fn includes_loopback_adapters() {
        // Loopback is not virtual
        let filter = ExcludeVirtualFilter;
        assert!(filter.matches(&loopback_adapter()));
    }

    #[test]
    fn default_trait_implemented() {
        fn assert_default<T: Default>() {}
        assert_default::<ExcludeVirtualFilter>();
    }

    #[test]
    fn debug_impl_works() {
        let filter = ExcludeVirtualFilter;
        assert!(!format!("{filter:?}").is_empty());
    }

    #[test]
    fn clone_works() {
        let filter = ExcludeVirtualFilter;
        #[allow(clippy::clone_on_copy)]
        let cloned = filter.clone();
        assert!(cloned.matches(&ethernet_adapter()));
    }
}

// ============================================================================
// ExcludeLoopbackFilter Tests
// ============================================================================

mod exclude_loopback_filter {
    use super::*;

    #[test]
    fn excludes_loopback_adapters() {
        let filter = ExcludeLoopbackFilter;
        assert!(!filter.matches(&loopback_adapter()));
    }

    #[test]
    fn includes_ethernet_adapters() {
        let filter = ExcludeLoopbackFilter;
        assert!(filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn includes_virtual_adapters() {
        let filter = ExcludeLoopbackFilter;
        assert!(filter.matches(&virtual_adapter()));
    }

    #[test]
    fn default_trait_implemented() {
        fn assert_default<T: Default>() {}
        assert_default::<ExcludeLoopbackFilter>();
    }

    #[test]
    fn debug_impl_works() {
        let filter = ExcludeLoopbackFilter;
        assert!(!format!("{filter:?}").is_empty());
    }
}

// ============================================================================
// CompositeFilter Tests
// ============================================================================

mod composite_filter {
    use super::*;

    #[test]
    fn empty_filter_matches_all() {
        let filter = CompositeFilter::new();
        assert!(filter.matches(&ethernet_adapter()));
        assert!(filter.matches(&virtual_adapter()));
        assert!(filter.matches(&loopback_adapter()));
    }

    #[test]
    fn single_filter_works() {
        let filter = CompositeFilter::new().with(ExcludeVirtualFilter);
        assert!(filter.matches(&ethernet_adapter()));
        assert!(!filter.matches(&virtual_adapter()));
    }

    #[test]
    fn multiple_filters_and_together() {
        let filter = CompositeFilter::new()
            .with(ExcludeVirtualFilter)
            .with(ExcludeLoopbackFilter);

        assert!(filter.matches(&ethernet_adapter()));
        assert!(!filter.matches(&virtual_adapter()));
        assert!(!filter.matches(&loopback_adapter()));
    }

    #[test]
    fn all_filters_must_pass() {
        // Include "Ethernet" AND exclude virtual
        // Virtual adapter named "vEthernet" should fail even though name contains "Ethernet"
        let filter = CompositeFilter::new()
            .with(NameRegexFilter::include(r"Ethernet").unwrap())
            .with(ExcludeVirtualFilter);

        assert!(filter.matches(&ethernet_adapter())); // Name matches, not virtual
        assert!(!filter.matches(&virtual_adapter())); // Name contains Ethernet, but is virtual
        assert!(!filter.matches(&wifi_adapter())); // Name doesn't match
    }

    #[test]
    fn len_returns_filter_count() {
        let filter = CompositeFilter::new()
            .with(ExcludeVirtualFilter)
            .with(ExcludeLoopbackFilter);
        assert_eq!(filter.len(), 2);
    }

    #[test]
    fn is_empty_true_when_no_filters() {
        let filter = CompositeFilter::new();
        assert!(filter.is_empty());
    }

    #[test]
    fn is_empty_false_when_has_filters() {
        let filter = CompositeFilter::new().with(ExcludeVirtualFilter);
        assert!(!filter.is_empty());
    }

    #[test]
    fn default_creates_empty_filter() {
        let filter = CompositeFilter::default();
        assert!(filter.is_empty());
    }

    #[test]
    fn debug_impl_shows_filter_count() {
        let filter = CompositeFilter::new()
            .with(ExcludeVirtualFilter)
            .with(ExcludeLoopbackFilter);
        let debug_str = format!("{filter:?}");
        assert!(debug_str.contains("CompositeFilter"));
        assert!(debug_str.contains('2')); // filter_count
    }

    #[test]
    fn complex_real_world_scenario() {
        // Real-world filter: exclude virtual, loopback, and Docker adapters
        let filter = CompositeFilter::new()
            .with(ExcludeVirtualFilter)
            .with(ExcludeLoopbackFilter)
            .with(NameRegexFilter::exclude(r"(?i)docker").unwrap());

        assert!(filter.matches(&ethernet_adapter()));
        assert!(filter.matches(&wifi_adapter()));
        assert!(!filter.matches(&virtual_adapter()));
        assert!(!filter.matches(&loopback_adapter()));
        assert!(!filter.matches(&docker_adapter()));
    }
}

// ============================================================================
// FilteredFetcher Tests
// ============================================================================

mod filtered_fetcher {
    use super::*;
    use crate::network::{AddressFetcher, FetchError};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// A mock fetcher for testing.
    #[derive(Debug)]
    struct MockFetcher {
        results: Mutex<VecDeque<Result<Vec<AdapterSnapshot>, FetchError>>>,
    }

    impl MockFetcher {
        fn new(results: Vec<Result<Vec<AdapterSnapshot>, FetchError>>) -> Self {
            Self {
                results: Mutex::new(results.into()),
            }
        }

        fn returning(snapshots: Vec<AdapterSnapshot>) -> Self {
            Self::new(vec![Ok(snapshots)])
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
    fn filters_adapters_from_inner_fetcher() {
        let all_adapters = vec![ethernet_adapter(), virtual_adapter(), wifi_adapter()];
        let fetcher =
            FilteredFetcher::new(MockFetcher::returning(all_adapters), ExcludeVirtualFilter);

        let result = fetcher.fetch().unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|a| !a.kind.is_virtual()));
    }

    #[test]
    fn propagates_errors_from_inner_fetcher() {
        let fetcher = FilteredFetcher::new(
            MockFetcher::new(vec![Err(FetchError::Platform {
                message: "test error".to_string(),
            })]),
            ExcludeVirtualFilter,
        );

        let result = fetcher.fetch();

        assert!(result.is_err());
    }

    #[test]
    fn empty_result_when_all_filtered() {
        let all_virtual = vec![virtual_adapter(), docker_adapter()];
        let fetcher =
            FilteredFetcher::new(MockFetcher::returning(all_virtual), ExcludeVirtualFilter);

        let result = fetcher.fetch().unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn inner_accessor_returns_reference() {
        let mock = MockFetcher::returning(vec![]);
        let fetcher = FilteredFetcher::new(mock, ExcludeVirtualFilter);

        // Can access inner fetcher
        let _ = fetcher.inner();
    }

    #[test]
    fn filter_accessor_returns_reference() {
        let mock = MockFetcher::returning(vec![]);
        let fetcher = FilteredFetcher::new(mock, ExcludeVirtualFilter);

        // Can access filter
        let _ = fetcher.filter();
    }

    #[test]
    fn into_inner_returns_owned_fetcher() {
        let mock = MockFetcher::returning(vec![ethernet_adapter()]);
        let fetcher = FilteredFetcher::new(mock, ExcludeVirtualFilter);

        let inner = fetcher.into_inner();
        let result = inner.fetch().unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn works_with_composite_filter() {
        let adapters = vec![
            ethernet_adapter(),
            virtual_adapter(),
            loopback_adapter(),
            wifi_adapter(),
        ];

        let filter = CompositeFilter::new()
            .with(ExcludeVirtualFilter)
            .with(ExcludeLoopbackFilter);

        let fetcher = FilteredFetcher::new(MockFetcher::returning(adapters), filter);
        let result = fetcher.fetch().unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "Ethernet");
        assert_eq!(result[1].name, "Wi-Fi");
    }

    #[test]
    fn debug_impl_works() {
        let fetcher = FilteredFetcher::new(MockFetcher::returning(vec![]), ExcludeVirtualFilter);
        let debug_str = format!("{fetcher:?}");
        assert!(debug_str.contains("FilteredFetcher"));
    }

    #[test]
    fn implements_address_fetcher_trait() {
        // Verify FilteredFetcher implements AddressFetcher via type constraint
        fn assert_fetcher<F: AddressFetcher>(_: &F) {}

        let fetcher = FilteredFetcher::new(MockFetcher::returning(vec![]), ExcludeVirtualFilter);
        assert_fetcher(&fetcher);
    }
}

// ============================================================================
// Blanket Implementation Tests
// ============================================================================

mod blanket_impl {
    use super::*;

    #[test]
    fn reference_to_filter_implements_trait() {
        let filter = ExcludeVirtualFilter;
        let filter_ref: &dyn AdapterFilter = &filter;

        assert!(filter_ref.matches(&ethernet_adapter()));
        assert!(!filter_ref.matches(&virtual_adapter()));
    }

    #[test]
    fn boxed_filter_implements_trait() {
        let filter: Box<dyn AdapterFilter> = Box::new(ExcludeVirtualFilter);

        assert!(filter.matches(&ethernet_adapter()));
        assert!(!filter.matches(&virtual_adapter()));
    }

    #[test]
    fn double_reference_works() {
        let filter = ExcludeVirtualFilter;
        let filter_ref = &filter;
        let filter_ref_ref = &filter_ref;

        // Should compile and work due to blanket impl
        assert!(filter_ref_ref.matches(&ethernet_adapter()));
    }
}
