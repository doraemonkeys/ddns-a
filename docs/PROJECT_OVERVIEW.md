# Project Overview

> **Doc Maintenance**: Keep concise, avoid redundancy, clean up outdated content promptly to reduce AI context usage.
> **Scope**: This document reflects the current codebase state only and does not describe future plans.
> **Goal**: Help AI quickly locate relevant code by module, type, and data flow.

## Module Map

| Module | Purpose |
|--------|---------|
| `config` | CLI argument parsing with clap (`Cli`, `Command`); TOML config parsing (`TomlConfig`); validated merged config (`ValidatedConfig`); `ConfigError` for configuration errors; `write_default_config()` for init command; `defaults` submodule for centralized default values |
| `network` | `AdapterSnapshot`, `AdapterKind`, `IpVersion`; `AddressFetcher` trait for platform-agnostic adapter info retrieval; `FetchError` variants |
| `network::filter` | `AdapterFilter` trait for filtering adapters; `NameRegexFilter`, `ExcludeVirtualFilter`, `ExcludeLoopbackFilter` concrete filters; `CompositeFilter` for AND composition; `FilteredFetcher` decorator |
| `network::platform` | Platform-specific implementations; `WindowsFetcher` on Windows using `GetAdaptersAddresses` |
| `monitor` | `IpChange`, `IpChangeKind`, `diff()` pure function for change detection; `DebouncePolicy` for event merging; `PollingMonitor`, `PollingStream` for polling-based monitoring; `HybridMonitor`, `HybridStream` for combined API+polling monitoring; `ApiListener` trait for platform event notifications; `MonitorError`, `ApiError` for layered error handling |
| `monitor::platform` | Platform-specific listeners; `WindowsApiListener` on Windows using `NotifyIpInterfaceChange` |
| `webhook` | `HttpRequest`, `HttpResponse` value types; `HttpClient` trait for HTTP abstraction; `HttpError`, `RetryableError`, `WebhookError` for layered error handling; `ReqwestClient` production HTTP client; `RetryPolicy` for exponential backoff; `WebhookSender` trait and `HttpWebhook` for sending IP changes with retries |
| `time` | `Clock` trait for time abstraction; `SystemClock` production implementation |

## Key Types

```rust
// IP version filtering
IpVersion::V4 | V6 | Both  // includes_v4(), includes_v6(); Display impl formats as "IPv4"/"IPv6"/"Both"
AdapterKind::Ethernet | Wireless | Loopback | Virtual | Other(u32)  // is_virtual(), is_loopback()
AdapterSnapshot { name, kind, ipv4_addresses: Vec<Ipv4Addr>, ipv6_addresses: Vec<Ipv6Addr> }
  // Methods: new(), has_addresses(), address_count()
AddressFetcher trait { fetch() -> Result<Vec<AdapterSnapshot>, FetchError> }  // Send + Sync
FetchError::WindowsApi(windows::core::Error)  // #[cfg(windows)]
          | PermissionDenied { context }
          | Platform { message }

// Adapter filtering
AdapterFilter trait { fn matches(&self, adapter: &AdapterSnapshot) -> bool }  // Send + Sync
FilterMode::Include | Exclude  // For name-based filtering
NameRegexFilter { pattern: Regex, mode: FilterMode }  // Filter by name regex
  // Factory: include(pattern), exclude(pattern)
ExcludeVirtualFilter  // Excludes virtual adapters (VMware, VirtualBox, etc.)
ExcludeLoopbackFilter  // Excludes loopback adapters
CompositeFilter { filters: Vec<Box<dyn AdapterFilter>> }  // AND composition
  // Builder: new(), with(filter); matches all if empty
FilteredFetcher<F, A> { inner: F, filter: A }  // Decorator for AddressFetcher
  // Implements AddressFetcher, filters results via filter.matches()

// Platform implementations
WindowsFetcher::new()  // Windows only, uses GetAdaptersAddresses API; Default trait
PlatformFetcher        // Type alias for WindowsFetcher on Windows

// Monitor types
IpChangeKind::Added | Removed
IpChange { adapter, address: IpAddr, timestamp: SystemTime, kind }
  // Methods: new(), added(), removed(), is_added(), is_removed()
diff(&old, &new, timestamp) -> Vec<IpChange>  // Pure function for change detection
DebouncePolicy::new(window), window() -> Duration  // Default: 2 seconds
PollingMonitor<F, C = SystemClock>  // Builder: new(), with_clock(), with_debounce()
  // Methods: interval(), debounce(), into_stream()
PollingStream<F, C>  // Stream<Item = Vec<IpChange>>, returned by PollingMonitor::into_stream()
HybridMonitor<F, L, C = SystemClock>  // Builder: new(), with_clock(), with_debounce()
  // Methods: poll_interval(), debounce(), into_stream()
  // Combines API events (L: ApiListener) with polling fallback
HybridStream<F, S, C>  // Stream<Item = Vec<IpChange>>, auto-degrades to polling on API failure
  // Methods: is_polling_only() - check if degraded to polling-only mode
merge_changes(&[IpChange], timestamp) -> Vec<IpChange>  // Net effect merge for external consumers

// Time abstraction
Clock trait { now() -> SystemTime }  // Send + Sync
SystemClock  // Production impl; Debug, Clone, Copy, Default
Sleeper trait { async fn sleep(&self, Duration) }  // Send + Sync, for testable delays
TokioSleeper  // Production impl using tokio::time::sleep; Debug, Clone, Copy, Default
InstantSleeper  // Mock impl that returns immediately; Debug, Clone, Copy, Default

// API event listeners (one-time semantics: into_stream(self) consumes self)
ApiListener trait { type Stream; fn into_stream(self) -> Self::Stream }  // Send
  // Stream yields Result<(), ApiError> - notifications, not IP data
WindowsApiListener::new() -> Result<Self, ApiError>  // Windows only, uses NotifyIpInterfaceChange
WindowsApiStream  // Stream<Item = Result<(), ApiError>>, auto-cancels on drop
PlatformListener  // Type alias for WindowsApiListener on Windows

// Monitor errors (layered)
ApiError::WindowsApi(windows::core::Error)  // #[cfg(windows)]
       | Stopped
MonitorError::Fetch(FetchError)
           | ApiListenerFailed(#[source] ApiError)

// HTTP client abstraction
HttpRequest { method: http::Method, url: url::Url, headers: http::HeaderMap, body: Option<Vec<u8>> }
  // Constructors: new(method, url), get(url), post(url)
  // Builder: with_body(Vec<u8>), with_header(name, value)
HttpResponse { status: http::StatusCode, headers: http::HeaderMap, body: Vec<u8> }
  // Methods: new(), is_success(), body_text() -> Option<&str>
HttpClient trait { async fn request(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> }  // Send + Sync
HttpError::Connection(Box<dyn Error + Send + Sync>)  // Network failures
        | Timeout                                    // Request timed out
        | InvalidUrl(String)                         // Configuration error
ReqwestClient { inner: reqwest::Client }  // Production impl
  // Factory: new(), default(), from_client(reqwest::Client)

// Webhook sending with retries
RetryPolicy { max_attempts: u32 (pub), initial_delay: Duration, max_delay: Duration, multiplier: f64 }
  // Invariants: max_attempts >= 1, multiplier > 0.0 (enforced in builders, panic on violation)
  // Defaults: 3 attempts, 5s initial, 60s max, 2.0 multiplier
  // Builder: new(), with_max_attempts(n), with_initial_delay(), with_max_delay(), with_multiplier()
  // Methods: delay_for_retry(retry) -> Duration, should_retry(attempt) -> bool
RetryableError::Http(HttpError) | NonSuccessStatus { status, body } | Template(String)
  // Note: NonSuccessStatus is not always retryable; use IsRetryable::is_retryable() to check
WebhookError::Retryable(RetryableError) | MaxRetriesExceeded { attempts, last_error }
WebhookSender trait { async fn send(&self, changes: &[IpChange]) -> Result<(), WebhookError> }  // Send + Sync
HttpWebhook<H: HttpClient, S: Sleeper = TokioSleeper> { client, sleeper, url, method, headers, body_template, retry_policy }
  // Factory: new(client, url) - uses TokioSleeper by default
  // Builder: with_sleeper(s), with_method(), with_headers(), with_body_template(), with_retry_policy()
  // Accessors: url(), method(), retry_policy()
IsRetryable trait { fn is_retryable(&self) -> bool }  // Implemented for HttpError, RetryableError

// Configuration
Cli { url, ip_version, method, headers, bearer, body_template, ... }  // clap-derived CLI args
  // Optional fields: url, ip_version, method, poll_interval, retry_max, retry_delay
  // Methods: parse_args(), parse_from_iter(), is_init()
Command::Init { output: PathBuf }  // Subcommand for generating config
IpVersionArg::V4 | V6 | Both  // CLI-specific enum, converted to IpVersion
TomlConfig { webhook, filter, monitor, retry }  // TOML config sections
  // Factory: load(path), parse(content)
ValidatedConfig { ip_version, url, method, headers, filter, poll_interval, ... }
  // Factory: from_raw(&Cli, Option<&TomlConfig>), load(&Cli)
  // Priority: CLI explicit > TOML > built-in defaults
  // Body template validated with Handlebars syntax check
  // Retry validation: max_delay >= initial_delay enforced
  // Display impl: shows key config (URL, IP version, poll, retry) without sensitive data (bearer)
ConfigError::FileRead | TomlParse | MissingRequired | InvalidUrl | InvalidRegex | InvalidIpVersion | InvalidTemplate | ...
  // Factory: missing(field, hint)
defaults::{METHOD, POLL_INTERVAL_SECS, RETRY_MAX_ATTEMPTS, RETRY_INITIAL_DELAY_SECS, ...}
  // Centralized default constants; also poll_interval(), retry_initial_delay(), retry_max_delay() helpers
write_default_config(path) -> Result<(), ConfigError>  // Generates ddns-a.toml template
default_config_template() -> String  // Returns commented TOML template
```
