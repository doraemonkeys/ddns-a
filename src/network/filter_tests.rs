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
// KindFilter Tests
// ============================================================================

mod kind_filter {
    use super::*;

    #[test]
    fn matches_single_kind() {
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        assert!(filter.matches(&ethernet_adapter()));
        assert!(!filter.matches(&wifi_adapter()));
    }

    #[test]
    fn matches_multiple_kinds() {
        let filter = KindFilter::new([AdapterKind::Ethernet, AdapterKind::Wireless]);
        assert!(filter.matches(&ethernet_adapter()));
        assert!(filter.matches(&wifi_adapter()));
        assert!(!filter.matches(&virtual_adapter()));
    }

    #[test]
    fn empty_filter_matches_nothing() {
        let filter = KindFilter::new([]);
        assert!(!filter.matches(&ethernet_adapter()));
        assert!(!filter.matches(&wifi_adapter()));
    }

    #[test]
    fn is_empty_true_for_no_kinds() {
        let filter = KindFilter::new([]);
        assert!(filter.is_empty());
    }

    #[test]
    fn is_empty_false_when_has_kinds() {
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        assert!(!filter.is_empty());
    }

    #[test]
    fn len_returns_kind_count() {
        let filter = KindFilter::new([AdapterKind::Ethernet, AdapterKind::Wireless]);
        assert_eq!(filter.len(), 2);
    }

    #[test]
    fn kinds_accessor_returns_set() {
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        assert!(filter.kinds().contains(&AdapterKind::Ethernet));
        assert!(!filter.kinds().contains(&AdapterKind::Wireless));
    }

    #[test]
    fn matches_virtual_kind() {
        let filter = KindFilter::new([AdapterKind::Virtual]);
        assert!(filter.matches(&virtual_adapter()));
        assert!(filter.matches(&docker_adapter()));
        assert!(!filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn matches_loopback_kind() {
        let filter = KindFilter::new([AdapterKind::Loopback]);
        assert!(filter.matches(&loopback_adapter()));
        assert!(!filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn debug_impl_works() {
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let debug_str = format!("{filter:?}");
        assert!(debug_str.contains("KindFilter"));
    }

    #[test]
    fn clone_works() {
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        #[allow(clippy::redundant_clone)]
        let cloned = filter.clone();
        assert!(cloned.matches(&ethernet_adapter()));
    }
}

// ============================================================================
// FilterChain Tests
// ============================================================================

mod filter_chain {
    use super::*;

    #[test]
    fn empty_chain_matches_all() {
        let chain = FilterChain::new();
        assert!(chain.matches(&ethernet_adapter()));
        assert!(chain.matches(&virtual_adapter()));
        assert!(chain.matches(&loopback_adapter()));
    }

    #[test]
    fn exclude_rejects_matching_adapters() {
        let chain = FilterChain::new().exclude(KindFilter::new([AdapterKind::Virtual]));
        assert!(chain.matches(&ethernet_adapter()));
        assert!(!chain.matches(&virtual_adapter()));
    }

    #[test]
    fn include_accepts_matching_adapters() {
        let chain = FilterChain::new().include(KindFilter::new([AdapterKind::Ethernet]));
        assert!(chain.matches(&ethernet_adapter()));
        assert!(!chain.matches(&wifi_adapter()));
    }

    #[test]
    fn exclude_takes_priority_over_include() {
        // Both include and exclude Virtual - exclude wins
        let chain = FilterChain::new()
            .include(KindFilter::new([AdapterKind::Virtual]))
            .exclude(KindFilter::new([AdapterKind::Virtual]));
        assert!(!chain.matches(&virtual_adapter()));
    }

    #[test]
    fn multiple_includes_use_or_semantics() {
        // Include Ethernet OR Wireless - either matches
        let chain = FilterChain::new()
            .include(KindFilter::new([AdapterKind::Ethernet]))
            .include(KindFilter::new([AdapterKind::Wireless]));
        assert!(chain.matches(&ethernet_adapter()));
        assert!(chain.matches(&wifi_adapter()));
        assert!(!chain.matches(&virtual_adapter()));
    }

    #[test]
    fn multiple_excludes_use_and_semantics() {
        // Exclude Virtual AND Loopback - both are excluded
        let chain = FilterChain::new()
            .exclude(KindFilter::new([AdapterKind::Virtual]))
            .exclude(KindFilter::new([AdapterKind::Loopback]));
        assert!(chain.matches(&ethernet_adapter()));
        assert!(!chain.matches(&virtual_adapter()));
        assert!(!chain.matches(&loopback_adapter()));
    }

    #[test]
    fn include_count_returns_correct_value() {
        let chain = FilterChain::new()
            .include(KindFilter::new([AdapterKind::Ethernet]))
            .include(KindFilter::new([AdapterKind::Wireless]));
        assert_eq!(chain.include_count(), 2);
    }

    #[test]
    fn exclude_count_returns_correct_value() {
        let chain = FilterChain::new()
            .exclude(KindFilter::new([AdapterKind::Virtual]))
            .exclude(KindFilter::new([AdapterKind::Loopback]));
        assert_eq!(chain.exclude_count(), 2);
    }

    #[test]
    fn is_empty_true_when_no_filters() {
        let chain = FilterChain::new();
        assert!(chain.is_empty());
    }

    #[test]
    fn is_empty_false_with_include() {
        let chain = FilterChain::new().include(KindFilter::new([AdapterKind::Ethernet]));
        assert!(!chain.is_empty());
    }

    #[test]
    fn is_empty_false_with_exclude() {
        let chain = FilterChain::new().exclude(KindFilter::new([AdapterKind::Virtual]));
        assert!(!chain.is_empty());
    }

    #[test]
    fn default_creates_empty_chain() {
        let chain = FilterChain::default();
        assert!(chain.is_empty());
    }

    #[test]
    fn debug_impl_shows_counts() {
        let chain = FilterChain::new()
            .include(KindFilter::new([AdapterKind::Ethernet]))
            .exclude(KindFilter::new([AdapterKind::Virtual]));
        let debug_str = format!("{chain:?}");
        assert!(debug_str.contains("FilterChain"));
        assert!(debug_str.contains("include_count"));
        assert!(debug_str.contains("exclude_count"));
    }

    #[test]
    fn complex_real_world_scenario() {
        // Include physical adapters (Ethernet/Wireless), exclude loopback
        let chain = FilterChain::new()
            .exclude(KindFilter::new([AdapterKind::Loopback]))
            .include(KindFilter::new([
                AdapterKind::Ethernet,
                AdapterKind::Wireless,
            ]));

        assert!(chain.matches(&ethernet_adapter()));
        assert!(chain.matches(&wifi_adapter()));
        assert!(!chain.matches(&virtual_adapter())); // Not in include list
        assert!(!chain.matches(&loopback_adapter())); // Excluded
    }

    #[test]
    fn with_name_regex_filter() {
        // Exclude adapters with "Docker" in name
        let chain = FilterChain::new().exclude(NameRegexFilter::new(r"(?i)docker").unwrap());

        assert!(chain.matches(&ethernet_adapter()));
        assert!(!chain.matches(&docker_adapter()));
    }

    #[test]
    fn combined_kind_and_name_filters() {
        // Include Ethernet/Wireless by kind, but exclude "Docker" by name
        let chain = FilterChain::new()
            .exclude(NameRegexFilter::new(r"(?i)docker").unwrap())
            .include(KindFilter::new([
                AdapterKind::Ethernet,
                AdapterKind::Wireless,
            ]));

        assert!(chain.matches(&ethernet_adapter()));
        assert!(chain.matches(&wifi_adapter()));
        assert!(!chain.matches(&docker_adapter())); // Excluded by name
        assert!(!chain.matches(&loopback_adapter())); // Not included by kind
    }
}

// ============================================================================
// NameRegexFilter Tests (New Pure Matcher API)
// ============================================================================

mod name_regex_filter {
    use super::*;

    #[test]
    fn matches_when_pattern_matches() {
        let filter = NameRegexFilter::new(r"^Ethernet").unwrap();
        assert!(filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn does_not_match_when_pattern_differs() {
        let filter = NameRegexFilter::new(r"^Ethernet").unwrap();
        assert!(!filter.matches(&wifi_adapter()));
    }

    #[test]
    fn partial_match_works() {
        let filter = NameRegexFilter::new(r"Wi").unwrap();
        assert!(filter.matches(&wifi_adapter()));
    }

    #[test]
    fn case_sensitive_by_default() {
        let filter = NameRegexFilter::new(r"ethernet").unwrap();
        assert!(!filter.matches(&ethernet_adapter())); // "Ethernet" has capital E
    }

    #[test]
    fn case_insensitive_with_flag() {
        let filter = NameRegexFilter::new(r"(?i)ethernet").unwrap();
        assert!(filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn invalid_regex_returns_error() {
        let result = NameRegexFilter::new(r"[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn pattern_accessor_returns_regex() {
        let filter = NameRegexFilter::new(r"^eth\d+").unwrap();
        assert!(filter.pattern().is_match("eth0"));
        assert!(!filter.pattern().is_match("wlan0"));
    }

    #[test]
    fn debug_impl_works() {
        let filter = NameRegexFilter::new(r"test").unwrap();
        let debug_str = format!("{filter:?}");
        assert!(debug_str.contains("NameRegexFilter"));
    }

    #[test]
    fn matches_docker_pattern() {
        let filter = NameRegexFilter::new(r"(?i)docker").unwrap();
        assert!(filter.matches(&docker_adapter()));
        assert!(!filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn complex_pattern_with_alternation() {
        let filter = NameRegexFilter::new(r"(?i)(docker|vmware|virtualbox)").unwrap();
        assert!(filter.matches(&docker_adapter()));
        assert!(!filter.matches(&ethernet_adapter()));
    }

    #[test]
    fn use_with_filter_chain_include() {
        let chain = FilterChain::new().include(NameRegexFilter::new(r"^Eth").unwrap());
        assert!(chain.matches(&ethernet_adapter()));
        assert!(!chain.matches(&wifi_adapter()));
    }

    #[test]
    fn use_with_filter_chain_exclude() {
        let chain = FilterChain::new().exclude(NameRegexFilter::new(r"^vEthernet").unwrap());
        assert!(chain.matches(&ethernet_adapter()));
        assert!(!chain.matches(&virtual_adapter()));
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
    fn filters_adapters_with_kind_filter() {
        let all_adapters = vec![ethernet_adapter(), virtual_adapter(), wifi_adapter()];
        let filter = KindFilter::new([AdapterKind::Ethernet, AdapterKind::Wireless]);
        let fetcher = FilteredFetcher::new(MockFetcher::returning(all_adapters), filter);

        let result = fetcher.fetch().unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|a| !a.kind.is_virtual()));
    }

    #[test]
    fn filters_adapters_with_filter_chain() {
        let adapters = vec![
            ethernet_adapter(),
            virtual_adapter(),
            loopback_adapter(),
            wifi_adapter(),
        ];

        let chain = FilterChain::new().exclude(KindFilter::new([
            AdapterKind::Virtual,
            AdapterKind::Loopback,
        ]));

        let fetcher = FilteredFetcher::new(MockFetcher::returning(adapters), chain);
        let result = fetcher.fetch().unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "Ethernet");
        assert_eq!(result[1].name, "Wi-Fi");
    }

    #[test]
    fn propagates_errors_from_inner_fetcher() {
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let fetcher = FilteredFetcher::new(
            MockFetcher::new(vec![Err(FetchError::Platform {
                message: "test error".to_string(),
            })]),
            filter,
        );

        let result = fetcher.fetch();
        assert!(result.is_err());
    }

    #[test]
    fn empty_result_when_all_filtered() {
        let all_virtual = vec![virtual_adapter(), docker_adapter()];
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let fetcher = FilteredFetcher::new(MockFetcher::returning(all_virtual), filter);

        let result = fetcher.fetch().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn inner_accessor_returns_reference() {
        let mock = MockFetcher::returning(vec![]);
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let fetcher = FilteredFetcher::new(mock, filter);

        let _ = fetcher.inner();
    }

    #[test]
    fn filter_accessor_returns_reference() {
        let mock = MockFetcher::returning(vec![]);
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let fetcher = FilteredFetcher::new(mock, filter);

        let _ = fetcher.filter();
    }

    #[test]
    fn into_inner_returns_owned_fetcher() {
        let mock = MockFetcher::returning(vec![ethernet_adapter()]);
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let fetcher = FilteredFetcher::new(mock, filter);

        let inner = fetcher.into_inner();
        let result = inner.fetch().unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn debug_impl_works() {
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let fetcher = FilteredFetcher::new(MockFetcher::returning(vec![]), filter);
        let debug_str = format!("{fetcher:?}");
        assert!(debug_str.contains("FilteredFetcher"));
    }

    #[test]
    fn implements_address_fetcher_trait() {
        fn assert_fetcher<F: AddressFetcher>(_: &F) {}

        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let fetcher = FilteredFetcher::new(MockFetcher::returning(vec![]), filter);
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
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let filter_ref: &dyn AdapterFilter = &filter;

        assert!(filter_ref.matches(&ethernet_adapter()));
        assert!(!filter_ref.matches(&virtual_adapter()));
    }

    #[test]
    fn boxed_filter_implements_trait() {
        let filter: Box<dyn AdapterFilter> = Box::new(KindFilter::new([AdapterKind::Ethernet]));

        assert!(filter.matches(&ethernet_adapter()));
        assert!(!filter.matches(&virtual_adapter()));
    }

    #[test]
    fn double_reference_works() {
        let filter = KindFilter::new([AdapterKind::Ethernet]);
        let filter_ref = &filter;
        let filter_ref_ref = &filter_ref;

        assert!(filter_ref_ref.matches(&ethernet_adapter()));
    }
}
