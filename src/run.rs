//! Application execution logic.
//!
//! This module contains the main async execution loop that monitors
//! IP address changes and sends webhook notifications.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use thiserror::Error;
use tokio::signal;
use tokio_stream::StreamExt;

use ddns_a::config::ValidatedConfig;
use ddns_a::monitor::{
    DebouncePolicy, HybridMonitor, IpChange, PollingMonitor, diff, filter_by_version,
};
use ddns_a::network::filter::{CompositeFilter, FilteredFetcher};
use ddns_a::network::platform::PlatformFetcher;
use ddns_a::network::{AdapterSnapshot, AddressFetcher, IpVersion};
use ddns_a::state::{FileStateStore, LoadResult, StateStore};
use ddns_a::webhook::{HttpWebhook, ReqwestClient, WebhookSender};

/// Type alias for the application's filtered fetcher.
type AppFetcher = FilteredFetcher<PlatformFetcher, CompositeFilter>;

#[cfg(windows)]
use ddns_a::monitor::platform::PlatformListener;

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;

/// Error type for runtime execution failures.
#[derive(Debug, Error)]
pub enum RunError {
    /// Failed to create the API listener.
    #[error("Failed to create API listener: {0}")]
    ApiListenerCreation(#[source] ddns_a::monitor::ApiError),

    /// Unexpected stream termination.
    #[error("Monitor stream terminated unexpectedly")]
    StreamTerminated,

    /// Failed to fetch initial network state.
    #[error("Failed to fetch initial network state: {0}")]
    InitialFetch(#[source] ddns_a::network::FetchError),

    /// Failed to save state file.
    #[error("Failed to save state: {0}")]
    StateSave(#[source] ddns_a::state::StateError),
}

/// Runtime options extracted from validated config.
///
/// This struct holds only the fields needed for the monitoring loop,
/// allowing the config's `filter` field to be moved separately.
struct RuntimeOptions {
    ip_version: IpVersion,
    poll_interval: Duration,
    poll_only: bool,
    dry_run: bool,
    state_file: Option<PathBuf>,
}

impl From<&ValidatedConfig> for RuntimeOptions {
    fn from(config: &ValidatedConfig) -> Self {
        Self {
            ip_version: config.ip_version,
            poll_interval: config.poll_interval,
            poll_only: config.poll_only,
            dry_run: config.dry_run,
            state_file: config.state_file.clone(),
        }
    }
}

/// Executes the main application loop.
///
/// This function:
/// 1. Creates the network fetcher with configured filters
/// 2. Detects startup changes (if state file is configured)
/// 3. Creates the monitor (hybrid or polling-only based on config)
/// 4. Creates the webhook sender
/// 5. Runs the monitoring loop until shutdown signal (Ctrl+C)
///
/// # Errors
///
/// Returns an error if:
/// - The API listener fails to initialize (in hybrid mode)
/// - The monitor stream terminates unexpectedly
/// - Failed to fetch initial network state (when state file is configured)
///
/// # Coverage Note
///
/// This function is excluded from coverage because it requires:
/// - Platform-specific network APIs
/// - Real async runtime with signal handling
#[cfg(not(tarpaulin_include))]
pub async fn execute(config: ValidatedConfig) -> Result<(), RunError> {
    // Extract runtime options before consuming config fields
    let options = RuntimeOptions::from(&config);

    // Create the webhook sender
    let webhook = create_webhook(&config);

    // Create the fetcher with filters (consumes config.filter)
    let fetcher = FilteredFetcher::new(PlatformFetcher::default(), config.filter);

    // Log startup info
    if options.dry_run {
        tracing::info!("Dry-run mode enabled - webhook requests will be logged but not sent");
    }

    // Create state store if configured
    let state_store = options.state_file.as_ref().map(FileStateStore::new);

    // Perform startup change detection if state file is configured
    if let Some(ref store) = state_store {
        tracing::info!("State persistence enabled: {}", store.path().display());
        startup_change_detection(store, &fetcher, &webhook, &options).await?;
    }

    if options.poll_only {
        tracing::info!(
            "Polling-only mode enabled (interval: {}s)",
            options.poll_interval.as_secs()
        );
        run_polling_loop(fetcher, webhook, options, state_store).await
    } else {
        tracing::info!(
            "Hybrid mode enabled (API events + polling every {}s)",
            options.poll_interval.as_secs()
        );
        run_hybrid_loop(fetcher, webhook, options, state_store).await
    }
}

/// Detects and handles IP changes that occurred while the program was stopped.
///
/// Compares the current network state with the previously saved state.
/// If changes are detected, sends a webhook notification.
///
/// Excluded from coverage - requires platform APIs.
#[cfg(not(tarpaulin_include))]
async fn startup_change_detection<W: WebhookSender>(
    store: &FileStateStore,
    fetcher: &AppFetcher,
    webhook: &W,
    options: &RuntimeOptions,
) -> Result<(), RunError> {
    // Fetch current network state
    let current = fetcher.fetch().map_err(RunError::InitialFetch)?;

    // Compare with saved state
    let startup_changes = detect_startup_changes(store, &current, options.ip_version);

    // Handle any detected changes
    if startup_changes.is_empty() {
        tracing::debug!("No IP changes detected since last run");
    } else {
        tracing::info!(
            "Detected {} change(s) since last run",
            startup_changes.len()
        );
        handle_changes(&startup_changes, webhook, options.dry_run).await;
    }

    // Save current state (optimistic save - before webhook result matters)
    // This ensures the state reflects the actual current IPs
    if let Err(e) = store.save(&current).await {
        tracing::error!("Failed to save state: {e}");
        return Err(RunError::StateSave(e));
    }

    Ok(())
}

/// Compares current network state with saved state and returns changes.
fn detect_startup_changes(
    store: &impl StateStore,
    current: &[AdapterSnapshot],
    ip_version: IpVersion,
) -> Vec<IpChange> {
    detect_startup_changes_with_timestamp(store, current, ip_version, SystemTime::now())
}

/// Compares current network state with saved state and returns changes.
///
/// This variant accepts a timestamp for testability.
fn detect_startup_changes_with_timestamp(
    store: &impl StateStore,
    current: &[AdapterSnapshot],
    ip_version: IpVersion,
    timestamp: SystemTime,
) -> Vec<IpChange> {
    match store.load() {
        LoadResult::Loaded(saved) => {
            let changes = diff(&saved, current, timestamp);
            filter_by_version(changes, ip_version)
        }
        LoadResult::NotFound => {
            tracing::info!("No previous state found, starting fresh");
            vec![]
        }
        LoadResult::Corrupted { reason } => {
            tracing::warn!("State file corrupted ({reason}), will overwrite on next save");
            vec![]
        }
    }
}

/// Creates the HTTP webhook sender from configuration.
fn create_webhook(config: &ValidatedConfig) -> HttpWebhook<ReqwestClient> {
    let mut webhook = HttpWebhook::new(ReqwestClient::new(), config.url.clone())
        .with_method(config.method.clone())
        .with_headers(config.headers.clone())
        .with_retry_policy(config.retry_policy.clone());

    if let Some(ref template) = config.body_template {
        webhook = webhook.with_body_template(template);
    }

    webhook
}

/// Runs the polling-only monitoring loop.
///
/// Excluded from coverage - requires platform APIs and signal handling.
#[cfg(not(tarpaulin_include))]
async fn run_polling_loop<W: WebhookSender>(
    fetcher: AppFetcher,
    webhook: W,
    options: RuntimeOptions,
    state_store: Option<FileStateStore>,
) -> Result<(), RunError> {
    let monitor = PollingMonitor::new(fetcher, options.poll_interval)
        .with_debounce(DebouncePolicy::default());

    let mut stream = monitor.into_stream();
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            biased;

            () = &mut shutdown => {
                tracing::info!("Shutdown signal received, stopping...");
                return Ok(());
            }

            changes = stream.next() => {
                match changes {
                    Some(changes) => {
                        // Filter by IP version before processing
                        let filtered = filter_by_version(changes, options.ip_version);
                        if !filtered.is_empty() {
                            save_state_if_configured(state_store.as_ref(), stream.current_snapshot()).await;
                            handle_changes(&filtered, &webhook, options.dry_run).await;
                        }
                    }
                    None => {
                        // Stream ended unexpectedly
                        return Err(RunError::StreamTerminated);
                    }
                }
            }
        }
    }
}

/// Saves state to the store if configured.
///
/// Uses optimistic save strategy: state is saved before webhook delivery.
/// This ensures the state reflects actual current IPs regardless of webhook success.
/// On restart, previously notified changes won't re-trigger (by design).
async fn save_state_if_configured(
    store: Option<&FileStateStore>,
    snapshot: Option<&[AdapterSnapshot]>,
) {
    if let (Some(store), Some(snapshot)) = (store, snapshot) {
        if let Err(e) = store.save(snapshot).await {
            tracing::error!("Failed to save state: {e}");
        }
    }
}

/// Runs the hybrid (API + polling) monitoring loop.
///
/// Excluded from coverage - requires Windows API and signal handling.
#[cfg(not(tarpaulin_include))]
#[cfg(windows)]
async fn run_hybrid_loop<W: WebhookSender>(
    fetcher: AppFetcher,
    webhook: W,
    options: RuntimeOptions,
    state_store: Option<FileStateStore>,
) -> Result<(), RunError> {
    let listener = PlatformListener::new().map_err(RunError::ApiListenerCreation)?;

    let monitor = HybridMonitor::new(fetcher, listener, options.poll_interval)
        .with_debounce(DebouncePolicy::default());

    let mut stream = monitor.into_stream();
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    // Track if we've logged the degradation
    let mut logged_degradation = false;

    loop {
        tokio::select! {
            biased;

            () = &mut shutdown => {
                tracing::info!("Shutdown signal received, stopping...");
                return Ok(());
            }

            changes = stream.next() => {
                // Check for degradation
                if !logged_degradation && stream.is_polling_only() {
                    tracing::warn!("API listener failed, degraded to polling-only mode");
                    logged_degradation = true;
                }

                match changes {
                    Some(changes) => {
                        // Filter by IP version before processing
                        let filtered = filter_by_version(changes, options.ip_version);
                        if !filtered.is_empty() {
                            save_state_if_configured(state_store.as_ref(), stream.current_snapshot()).await;
                            handle_changes(&filtered, &webhook, options.dry_run).await;
                        }
                    }
                    None => {
                        // Stream ended unexpectedly
                        return Err(RunError::StreamTerminated);
                    }
                }
            }
        }
    }
}

/// Non-Windows stub for hybrid loop.
///
/// Excluded from coverage - requires platform APIs and signal handling.
#[cfg(not(tarpaulin_include))]
#[cfg(not(windows))]
async fn run_hybrid_loop<W: WebhookSender>(
    fetcher: AppFetcher,
    webhook: W,
    options: RuntimeOptions,
    state_store: Option<FileStateStore>,
) -> Result<(), RunError> {
    // On non-Windows platforms, fall back to polling-only
    tracing::warn!("API listener not supported on this platform, using polling-only mode");
    run_polling_loop(fetcher, webhook, options, state_store).await
}

/// Handles a batch of IP changes.
async fn handle_changes<W: WebhookSender>(changes: &[IpChange], webhook: &W, dry_run: bool) {
    // Log the changes
    for change in changes {
        let action = if change.is_added() { "+" } else { "-" };
        tracing::info!(
            "{action} {address} on {adapter}",
            address = change.address,
            adapter = change.adapter,
        );
    }

    // Send webhook (unless dry-run)
    if dry_run {
        tracing::debug!("Dry-run: skipping webhook for {} change(s)", changes.len());
        return;
    }

    match webhook.send(changes).await {
        Ok(()) => {
            tracing::debug!("Webhook sent successfully for {} change(s)", changes.len());
        }
        Err(e) => {
            tracing::error!("Webhook failed: {e}");
        }
    }
}

/// Returns a future that completes when a shutdown signal is received.
///
/// Excluded from coverage - requires OS signal handling.
#[cfg(not(tarpaulin_include))]
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
