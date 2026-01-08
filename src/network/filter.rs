//! Adapter filtering for selective monitoring.
//!
//! This module provides traits and types for filtering network adapters
//! based on various criteria (name patterns, adapter kind, etc.).
//!
//! # Design
//!
//! - **Pure Matchers**: [`KindFilter`] and [`NameRegexFilter`] only answer
//!   "does this adapter match?" without include/exclude semantics.
//! - **Filter Chain**: [`FilterChain`] combines matchers with correct semantics:
//!   - Exclude filters: AND logic (must pass ALL excludes)
//!   - Include filters: OR logic (pass ANY include, empty = match all)
//! - **Decorator**: [`FilteredFetcher`] applies filtering transparently
//!   to any [`AddressFetcher`] implementation.

use std::collections::HashSet;

use regex::Regex;

use super::{AdapterKind, AdapterSnapshot, AddressFetcher, FetchError};

/// Trait for filtering network adapters.
///
/// Implementations determine which adapters should be included in monitoring.
/// Filters are composable via [`FilterChain`].
///
/// # Thread Safety
///
/// Filters must be `Send + Sync` to support concurrent access in async contexts.
pub trait AdapterFilter: Send + Sync {
    /// Returns `true` if the adapter should be included, `false` to filter it out.
    fn matches(&self, adapter: &AdapterSnapshot) -> bool;
}

// ============================================================================
// KindFilter - Pure matcher by adapter kind
// ============================================================================

/// Filters adapters by their kind (pure matcher, no include/exclude semantics).
///
/// This filter matches adapters whose kind is contained in the specified set.
/// Use with [`FilterChain`] to apply include/exclude logic.
///
/// # Examples
///
/// ```
/// use ddns_a::network::filter::{KindFilter, AdapterFilter};
/// use ddns_a::network::{AdapterSnapshot, AdapterKind};
///
/// // Match wireless and ethernet adapters
/// let filter = KindFilter::new([AdapterKind::Wireless, AdapterKind::Ethernet]);
///
/// let eth = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
/// let loopback = AdapterSnapshot::new("lo", AdapterKind::Loopback, vec![], vec![]);
///
/// assert!(filter.matches(&eth));
/// assert!(!filter.matches(&loopback));
/// ```
#[derive(Debug, Clone)]
pub struct KindFilter {
    kinds: HashSet<AdapterKind>,
}

impl KindFilter {
    /// Creates a kind filter matching any of the specified kinds.
    #[must_use]
    pub fn new(kinds: impl IntoIterator<Item = AdapterKind>) -> Self {
        Self {
            kinds: kinds.into_iter().collect(),
        }
    }

    /// Returns true if no kinds are configured (matches nothing).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// Returns the number of kinds in the filter.
    #[must_use]
    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    /// Returns a reference to the set of kinds.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // HashSet is not const-compatible
    pub fn kinds(&self) -> &HashSet<AdapterKind> {
        &self.kinds
    }
}

impl AdapterFilter for KindFilter {
    fn matches(&self, adapter: &AdapterSnapshot) -> bool {
        self.kinds.contains(&adapter.kind)
    }
}

// ============================================================================
// FilterChain - Include OR / Exclude AND semantics
// ============================================================================

/// Filter chain with correct include/exclude semantics.
///
/// Evaluation order:
/// 1. **Exclude filters (AND)**: Any match → reject. Adapter must pass ALL excludes.
/// 2. **Include filters (OR)**: Any match → accept. Adapter needs to pass ANY include.
///    Empty includes = match all (passthrough).
///
/// # Examples
///
/// ```
/// use ddns_a::network::filter::{FilterChain, KindFilter, AdapterFilter};
/// use ddns_a::network::{AdapterSnapshot, AdapterKind};
///
/// let chain = FilterChain::new()
///     .exclude(KindFilter::new([AdapterKind::Loopback]))
///     .include(KindFilter::new([AdapterKind::Wireless, AdapterKind::Ethernet]));
///
/// let eth = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
/// let virtual_adapter = AdapterSnapshot::new("vm0", AdapterKind::Virtual, vec![], vec![]);
/// let loopback = AdapterSnapshot::new("lo", AdapterKind::Loopback, vec![], vec![]);
///
/// assert!(chain.matches(&eth));       // Included by kind
/// assert!(!chain.matches(&virtual_adapter)); // Not in include kinds
/// assert!(!chain.matches(&loopback)); // Excluded
/// ```
#[derive(Default)]
pub struct FilterChain {
    includes: Vec<Box<dyn AdapterFilter>>,
    excludes: Vec<Box<dyn AdapterFilter>>,
}

impl FilterChain {
    /// Creates an empty filter chain (matches all adapters).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an include filter (OR semantics).
    ///
    /// Adapters matching ANY include filter will be accepted
    /// (after passing all exclude filters).
    #[must_use]
    pub fn include<F: AdapterFilter + 'static>(mut self, filter: F) -> Self {
        self.includes.push(Box::new(filter));
        self
    }

    /// Adds an exclude filter (AND semantics - must not match ANY).
    ///
    /// Adapters matching ANY exclude filter will be rejected,
    /// regardless of include filters.
    #[must_use]
    pub fn exclude<F: AdapterFilter + 'static>(mut self, filter: F) -> Self {
        self.excludes.push(Box::new(filter));
        self
    }

    /// Returns the number of include filters.
    #[must_use]
    pub fn include_count(&self) -> usize {
        self.includes.len()
    }

    /// Returns the number of exclude filters.
    #[must_use]
    pub fn exclude_count(&self) -> usize {
        self.excludes.len()
    }

    /// Returns true if no filters are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.includes.is_empty() && self.excludes.is_empty()
    }
}

impl AdapterFilter for FilterChain {
    fn matches(&self, adapter: &AdapterSnapshot) -> bool {
        // 1. Any exclude match → reject
        if self.excludes.iter().any(|f| f.matches(adapter)) {
            return false;
        }

        // 2. No includes = all pass; otherwise any include match → accept
        self.includes.is_empty() || self.includes.iter().any(|f| f.matches(adapter))
    }
}

impl std::fmt::Debug for FilterChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterChain")
            .field("include_count", &self.includes.len())
            .field("exclude_count", &self.excludes.len())
            .finish()
    }
}

// ============================================================================
// NameRegexFilter - Pure matcher by name pattern
// ============================================================================

/// Filters adapters by name pattern (pure matcher, no include/exclude semantics).
///
/// This filter simply checks if the adapter name matches the regex pattern.
/// Use with [`FilterChain`] to apply include/exclude logic.
///
/// # Examples
///
/// ```
/// use ddns_a::network::filter::{NameRegexFilter, AdapterFilter};
/// use ddns_a::network::{AdapterSnapshot, AdapterKind};
///
/// let filter = NameRegexFilter::new(r"^eth").unwrap();
///
/// let eth0 = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
/// let wlan0 = AdapterSnapshot::new("wlan0", AdapterKind::Wireless, vec![], vec![]);
///
/// assert!(filter.matches(&eth0));
/// assert!(!filter.matches(&wlan0));
/// ```
#[derive(Debug)]
pub struct NameRegexFilter {
    pattern: Regex,
}

impl NameRegexFilter {
    /// Creates a name filter with the given regex pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
        })
    }

    /// Returns a reference to the regex pattern.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Regex is not a const type
    pub fn pattern(&self) -> &Regex {
        &self.pattern
    }
}

impl AdapterFilter for NameRegexFilter {
    fn matches(&self, adapter: &AdapterSnapshot) -> bool {
        self.pattern.is_match(&adapter.name)
    }
}

// ============================================================================
// FilteredFetcher - Decorator for AddressFetcher
// ============================================================================

/// A fetcher decorator that applies a filter to results.
///
/// This wraps any [`AddressFetcher`] and filters the returned adapters
/// using the provided [`AdapterFilter`].
///
/// # Type Parameters
///
/// - `F`: The inner fetcher type (implements [`AddressFetcher`])
/// - `A`: The filter type (implements [`AdapterFilter`])
///
/// # Examples
///
/// ```ignore
/// use ddns_a::network::filter::{FilteredFetcher, FilterChain, KindFilter};
/// use ddns_a::network::{AdapterKind, platform::WindowsFetcher};
///
/// let filter = FilterChain::new()
///     .exclude(KindFilter::new([AdapterKind::Virtual, AdapterKind::Loopback]));
/// let fetcher = FilteredFetcher::new(WindowsFetcher::new(), filter);
/// let adapters = fetcher.fetch()?; // Only non-virtual, non-loopback adapters
/// ```
#[derive(Debug)]
pub struct FilteredFetcher<F, A> {
    inner: F,
    filter: A,
}

impl<F, A> FilteredFetcher<F, A> {
    /// Creates a new filtered fetcher.
    #[must_use]
    pub const fn new(inner: F, filter: A) -> Self {
        Self { inner, filter }
    }

    /// Returns a reference to the inner fetcher.
    pub const fn inner(&self) -> &F {
        &self.inner
    }

    /// Returns a reference to the filter.
    pub const fn filter(&self) -> &A {
        &self.filter
    }

    /// Consumes the filtered fetcher and returns the inner fetcher.
    pub fn into_inner(self) -> F {
        self.inner
    }
}

impl<F: AddressFetcher, A: AdapterFilter> AddressFetcher for FilteredFetcher<F, A> {
    fn fetch(&self) -> Result<Vec<AdapterSnapshot>, FetchError> {
        let snapshots = self.inner.fetch()?;
        Ok(snapshots
            .into_iter()
            .filter(|adapter| self.filter.matches(adapter))
            .collect())
    }
}

// Blanket implementation: any &T where T: AdapterFilter also implements AdapterFilter
impl<T: AdapterFilter + ?Sized> AdapterFilter for &T {
    fn matches(&self, adapter: &AdapterSnapshot) -> bool {
        (*self).matches(adapter)
    }
}

// Box<dyn AdapterFilter> implements AdapterFilter
impl AdapterFilter for Box<dyn AdapterFilter> {
    fn matches(&self, adapter: &AdapterSnapshot) -> bool {
        self.as_ref().matches(adapter)
    }
}
