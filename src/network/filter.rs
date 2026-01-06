//! Adapter filtering for selective monitoring.
//!
//! This module provides traits and types for filtering network adapters
//! based on various criteria (name patterns, adapter kind, etc.).
//!
//! # Design
//!
//! Filters use AND composition by default via [`CompositeFilter`].
//! The [`FilteredFetcher`] decorator applies filtering transparently
//! to any [`AddressFetcher`] implementation.

use super::{AdapterSnapshot, AddressFetcher, FetchError};
use regex::Regex;

/// Trait for filtering network adapters.
///
/// Implementations determine which adapters should be included in monitoring.
/// Filters are composable via [`CompositeFilter`].
///
/// # Thread Safety
///
/// Filters must be `Send + Sync` to support concurrent access in async contexts.
pub trait AdapterFilter: Send + Sync {
    /// Returns `true` if the adapter should be included, `false` to filter it out.
    fn matches(&self, adapter: &AdapterSnapshot) -> bool;
}

/// Filter mode for name-based filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    /// Adapter name must match the pattern to be included.
    Include,
    /// Adapter name must NOT match the pattern to be included.
    Exclude,
}

/// Filters adapters by name using a regex pattern.
///
/// # Examples
///
/// ```
/// use ddns_a::network::filter::{NameRegexFilter, FilterMode, AdapterFilter};
/// use ddns_a::network::{AdapterSnapshot, AdapterKind};
///
/// // Include only adapters matching "eth*"
/// let include_eth = NameRegexFilter::new(r"^eth", FilterMode::Include).unwrap();
///
/// let eth0 = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
/// let wlan0 = AdapterSnapshot::new("wlan0", AdapterKind::Wireless, vec![], vec![]);
///
/// assert!(include_eth.matches(&eth0));
/// assert!(!include_eth.matches(&wlan0));
/// ```
#[derive(Debug)]
pub struct NameRegexFilter {
    pattern: Regex,
    mode: FilterMode,
}

impl NameRegexFilter {
    /// Creates a new name filter with the given regex pattern and mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn new(pattern: &str, mode: FilterMode) -> Result<Self, regex::Error> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
            mode,
        })
    }

    /// Creates an include filter (adapter name must match).
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn include(pattern: &str) -> Result<Self, regex::Error> {
        Self::new(pattern, FilterMode::Include)
    }

    /// Creates an exclude filter (adapter name must NOT match).
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn exclude(pattern: &str) -> Result<Self, regex::Error> {
        Self::new(pattern, FilterMode::Exclude)
    }

    /// Returns the filter mode.
    #[must_use]
    pub const fn mode(&self) -> FilterMode {
        self.mode
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
        let is_match = self.pattern.is_match(&adapter.name);
        match self.mode {
            FilterMode::Include => is_match,
            FilterMode::Exclude => !is_match,
        }
    }
}

/// Filters out virtual adapters.
///
/// Use this filter to exclude `VMware`, `VirtualBox`, Hyper-V, WSL, and similar
/// virtual network interfaces.
///
/// # Examples
///
/// ```
/// use ddns_a::network::filter::{ExcludeVirtualFilter, AdapterFilter};
/// use ddns_a::network::{AdapterSnapshot, AdapterKind};
///
/// let filter = ExcludeVirtualFilter;
///
/// let physical = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
/// let virtual_adapter = AdapterSnapshot::new("vEthernet", AdapterKind::Virtual, vec![], vec![]);
///
/// assert!(filter.matches(&physical));
/// assert!(!filter.matches(&virtual_adapter));
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ExcludeVirtualFilter;

impl AdapterFilter for ExcludeVirtualFilter {
    fn matches(&self, adapter: &AdapterSnapshot) -> bool {
        !adapter.kind.is_virtual()
    }
}

/// Filters out loopback adapters.
///
/// # Examples
///
/// ```
/// use ddns_a::network::filter::{ExcludeLoopbackFilter, AdapterFilter};
/// use ddns_a::network::{AdapterSnapshot, AdapterKind};
///
/// let filter = ExcludeLoopbackFilter;
///
/// let ethernet = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
/// let loopback = AdapterSnapshot::new("lo", AdapterKind::Loopback, vec![], vec![]);
///
/// assert!(filter.matches(&ethernet));
/// assert!(!filter.matches(&loopback));
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ExcludeLoopbackFilter;

impl AdapterFilter for ExcludeLoopbackFilter {
    fn matches(&self, adapter: &AdapterSnapshot) -> bool {
        !adapter.kind.is_loopback()
    }
}

/// A composite filter that ANDs multiple filters together.
///
/// An adapter passes the composite filter only if it passes ALL contained filters.
/// An empty composite filter matches all adapters.
///
/// # Design Decision
///
/// Uses `Box<dyn AdapterFilter>` for runtime flexibility since filter combinations
/// are determined by user configuration at runtime.
///
/// # Examples
///
/// ```
/// use ddns_a::network::filter::{CompositeFilter, ExcludeVirtualFilter, NameRegexFilter, AdapterFilter};
/// use ddns_a::network::{AdapterSnapshot, AdapterKind};
///
/// let filter = CompositeFilter::new()
///     .with(ExcludeVirtualFilter)
///     .with(NameRegexFilter::exclude(r"^Docker").unwrap());
///
/// let eth = AdapterSnapshot::new("eth0", AdapterKind::Ethernet, vec![], vec![]);
/// let docker = AdapterSnapshot::new("Docker Network", AdapterKind::Virtual, vec![], vec![]);
///
/// assert!(filter.matches(&eth));
/// assert!(!filter.matches(&docker));
/// ```
#[derive(Default)]
pub struct CompositeFilter {
    filters: Vec<Box<dyn AdapterFilter>>,
}

impl CompositeFilter {
    /// Creates an empty composite filter (matches all adapters).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a filter to the composition (builder pattern).
    #[must_use]
    pub fn with<F: AdapterFilter + 'static>(mut self, filter: F) -> Self {
        self.filters.push(Box::new(filter));
        self
    }

    /// Returns the number of filters in the composition.
    #[must_use]
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    /// Returns `true` if no filters are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

impl AdapterFilter for CompositeFilter {
    fn matches(&self, adapter: &AdapterSnapshot) -> bool {
        self.filters.iter().all(|f| f.matches(adapter))
    }
}

// Manual Debug impl since Box<dyn AdapterFilter> doesn't implement Debug
impl std::fmt::Debug for CompositeFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeFilter")
            .field("filter_count", &self.filters.len())
            .finish()
    }
}

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
/// use ddns_a::network::filter::{FilteredFetcher, ExcludeVirtualFilter};
/// use ddns_a::network::platform::WindowsFetcher;
///
/// let fetcher = FilteredFetcher::new(WindowsFetcher::new(), ExcludeVirtualFilter);
/// let adapters = fetcher.fetch()?; // Only non-virtual adapters
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
