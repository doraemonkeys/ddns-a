# Project Overview

> **Doc Maintenance**: Keep concise, clean up outdated content to reduce AI context.
> **Scope**: Current codebase state only, not future plans.

## Module Map

| Module | Purpose |
|--------|---------|
| `config` | `Cli` (clap), `TomlConfig`, `ValidatedConfig`, `ConfigError`; `defaults` submodule |
| `network` | `AdapterSnapshot`, `AdapterKind`, `IpVersion`; `AddressFetcher` trait; `FetchError` |
| `network::filter` | `AdapterFilter` trait; `NameRegexFilter`, `ExcludeVirtualFilter`, `ExcludeLoopbackFilter`; `CompositeFilter`; `FilteredFetcher` decorator |
| `network::platform` | `WindowsFetcher` (Windows, `GetAdaptersAddresses`); `PlatformFetcher` alias |
| `monitor` | `IpChange`, `diff()`; `DebouncePolicy`; `PollingMonitor`/`HybridMonitor`; `ApiListener` trait; `MonitorError`, `ApiError` |
| `monitor::platform` | `WindowsApiListener` (Windows, `NotifyIpInterfaceChange`); `PlatformListener` alias |
| `webhook` | `HttpRequest`, `HttpResponse`; `HttpClient` trait; `ReqwestClient`; `RetryPolicy`; `WebhookSender` trait, `HttpWebhook` |
| `time` | `Clock` trait, `SystemClock`; `Sleeper` trait, `TokioSleeper`, `InstantSleeper` |
| `main` (bin) | Entry: CLI, config, tracing, tokio runtime |
| `run` (bin) | `execute(ValidatedConfig)`: assembles components, graceful shutdown; `RunError` |

## Key Types

```rust
// Network
IpVersion::V4 | V6 | Both
AdapterKind::Ethernet | Wireless | Loopback | Virtual | Other(u32)
AdapterSnapshot { name, kind, ipv4_addresses, ipv6_addresses }
AddressFetcher trait { fetch() -> Result<Vec<AdapterSnapshot>, FetchError> }
FetchError::WindowsApi | PermissionDenied | Platform

// Filtering
AdapterFilter trait { fn matches(&self, adapter: &AdapterSnapshot) -> bool }
FilterMode::Include | Exclude
NameRegexFilter::include(pattern) | exclude(pattern)
ExcludeVirtualFilter, ExcludeLoopbackFilter  // Unit filters
CompositeFilter::new().with(filter)  // AND composition
FilteredFetcher<F, A>  // AddressFetcher decorator

// Monitor
IpChangeKind::Added | Removed
IpChange { adapter, address: IpAddr, timestamp, kind }
diff(&old, &new, timestamp) -> Vec<IpChange>
filter_by_version(changes, version) -> Vec<IpChange>
DebouncePolicy::new(window)  // Default: 2s
PollingMonitor<F, C>::new().with_clock().with_debounce().into_stream() -> PollingStream
HybridMonitor<F, L, C>::new().into_stream() -> HybridStream  // API + polling fallback
  // Debounce: API event starts window even without immediate changes (Windows timing)
merge_changes(&[IpChange], timestamp) -> Vec<IpChange>  // Net effect merge

// API Listener (one-time: into_stream consumes self)
ApiListener trait { type Stream; fn into_stream(self) -> Self::Stream }
  // Stream yields Result<(), ApiError> - notifications only
WindowsApiListener::new() -> Result<Self, ApiError>

// Errors
ApiError::WindowsApi | Stopped
MonitorError::Fetch(FetchError) | ApiListenerFailed(ApiError)

// HTTP
HttpRequest { method, url, headers, body }  // get(url), post(url), with_body(), with_header()
HttpResponse { status, headers, body }  // is_success(), body_text()
HttpClient trait { async fn request(&self, req) -> Result<HttpResponse, HttpError> }
HttpError::Connection | Timeout | InvalidUrl
ReqwestClient::new()

// Webhook
RetryPolicy { max_attempts, initial_delay, max_delay, multiplier }
  // Defaults: 3 attempts, 5s initial, 60s max, 2.0x
  // Builder: with_max_attempts(), with_initial_delay(), with_max_delay(), with_multiplier()
RetryableError::Http | NonSuccessStatus | Template
WebhookError::Retryable | MaxRetriesExceeded
WebhookSender trait { async fn send(&self, changes: &[IpChange]) -> Result<(), WebhookError> }
HttpWebhook<H, S>::new(client, url).with_method().with_headers().with_body_template().with_retry_policy()
IsRetryable trait { fn is_retryable(&self) -> bool }

// Config
Cli { url, ip_version, method, headers, bearer, body_template, poll_interval, retry_* }
Command::Init { output }
TomlConfig { webhook, filter, monitor, retry }  // load(path), parse(content)
ValidatedConfig { ip_version, url, method, headers, filter, poll_interval, retry_* }
  // from_raw(&Cli, Option<&TomlConfig>), load(&Cli)
  // Priority: CLI > TOML > defaults
ConfigError::FileRead | TomlParse | MissingRequired | InvalidUrl | InvalidRegex | InvalidTemplate | ...
defaults::{METHOD, POLL_INTERVAL_SECS, RETRY_*}
write_default_config(path), default_config_template()
```
