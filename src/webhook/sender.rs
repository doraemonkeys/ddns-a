//! Webhook sender trait and HTTP implementation.

use crate::monitor::IpChange;
use crate::time::{Sleeper, TokioSleeper};

use super::{HttpClient, HttpError, HttpRequest, RetryPolicy, RetryableError, WebhookError};
use handlebars::Handlebars;
use serde::Serialize;

/// Trait for sending IP change notifications to external services.
///
/// This abstraction allows for different notification mechanisms
/// (HTTP webhooks, message queues, etc.) and enables testing with mocks.
///
/// # Implementation Notes
///
/// Implementations should handle retries internally if appropriate,
/// returning [`WebhookError::MaxRetriesExceeded`] when all attempts fail.
pub trait WebhookSender: Send + Sync {
    /// Sends a notification about IP address changes.
    ///
    /// # Arguments
    ///
    /// * `changes` - The IP address changes to report
    ///
    /// # Errors
    ///
    /// Returns [`WebhookError`] if the notification fails after all retries.
    fn send(
        &self,
        changes: &[IpChange],
    ) -> impl std::future::Future<Output = Result<(), WebhookError>> + Send;
}

/// HTTP-based webhook sender with retry support.
///
/// Sends IP change notifications via HTTP requests, with configurable
/// retry behavior using exponential backoff.
///
/// # Template Support
///
/// The body can be templated using Handlebars syntax. Available variables:
/// - `changes`: Array of change objects, each with:
///   - `adapter`: Adapter name
///   - `address`: IP address string
///   - `kind`: "added" or "removed"
///   - `timestamp`: Unix timestamp (seconds)
///
/// # Type Parameters
///
/// - `H`: The HTTP client implementation
/// - `S`: The sleeper implementation for retry delays (defaults to [`TokioSleeper`])
///
/// # Example
///
/// ```
/// use ddns_a::webhook::{HttpWebhook, ReqwestClient, RetryPolicy};
/// use url::Url;
///
/// let webhook = HttpWebhook::new(
///     ReqwestClient::new(),
///     Url::parse("https://api.example.com/ddns").unwrap(),
/// );
/// ```
#[derive(Debug)]
pub struct HttpWebhook<H, S = TokioSleeper> {
    client: H,
    sleeper: S,
    url: url::Url,
    method: http::Method,
    headers: http::HeaderMap,
    body_template: Option<String>,
    retry_policy: RetryPolicy,
}

impl<H> HttpWebhook<H, TokioSleeper> {
    /// Creates a new HTTP webhook with default settings.
    ///
    /// Uses POST method, no custom headers, no body template,
    /// default retry policy, and [`TokioSleeper`] for delays.
    #[must_use]
    pub fn new(client: H, url: url::Url) -> Self {
        Self {
            client,
            sleeper: TokioSleeper,
            url,
            method: http::Method::POST,
            headers: http::HeaderMap::new(),
            body_template: None,
            retry_policy: RetryPolicy::default(),
        }
    }
}

impl<H, S> HttpWebhook<H, S> {
    /// Sets a custom sleeper for retry delays.
    ///
    /// This is primarily useful for testing to avoid actual delays.
    #[must_use]
    pub fn with_sleeper<S2>(self, sleeper: S2) -> HttpWebhook<H, S2> {
        HttpWebhook {
            client: self.client,
            sleeper,
            url: self.url,
            method: self.method,
            headers: self.headers,
            body_template: self.body_template,
            retry_policy: self.retry_policy,
        }
    }

    /// Sets the HTTP method.
    #[must_use]
    pub fn with_method(mut self, method: http::Method) -> Self {
        self.method = method;
        self
    }

    /// Sets the HTTP headers.
    #[must_use]
    pub fn with_headers(mut self, headers: http::HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Sets the body template (Handlebars syntax).
    #[must_use]
    pub fn with_body_template(mut self, template: impl Into<String>) -> Self {
        self.body_template = Some(template.into());
        self
    }

    /// Sets the retry policy.
    #[must_use]
    pub const fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Returns the configured URL.
    #[must_use]
    pub const fn url(&self) -> &url::Url {
        &self.url
    }

    /// Returns the configured HTTP method.
    #[must_use]
    pub const fn method(&self) -> &http::Method {
        &self.method
    }

    /// Returns the configured retry policy.
    #[must_use]
    pub const fn retry_policy(&self) -> &RetryPolicy {
        &self.retry_policy
    }
}

/// Template data for rendering webhook body.
#[derive(Serialize)]
struct TemplateData<'a> {
    changes: Vec<ChangeData<'a>>,
}

/// Individual change data for template rendering.
#[derive(Serialize)]
struct ChangeData<'a> {
    adapter: &'a str,
    address: String,
    kind: &'static str,
    timestamp: u64,
}

impl<'a> From<&'a IpChange> for ChangeData<'a> {
    fn from(change: &'a IpChange) -> Self {
        let kind = if change.is_added() {
            "added"
        } else {
            "removed"
        };
        // Pre-epoch timestamps (shouldn't occur in practice for DDNS events) default to 0
        let timestamp = change
            .timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        Self {
            adapter: &change.adapter,
            address: change.address.to_string(),
            kind,
            timestamp,
        }
    }
}

impl<H: HttpClient, S: Sleeper> HttpWebhook<H, S> {
    /// Renders the body template with the given changes.
    fn render_body(&self, changes: &[IpChange]) -> Result<Option<Vec<u8>>, RetryableError> {
        let Some(template) = &self.body_template else {
            return Ok(None);
        };

        let data = TemplateData {
            changes: changes.iter().map(ChangeData::from).collect(),
        };

        let handlebars = Handlebars::new();
        let rendered = handlebars
            .render_template(template, &data)
            .map_err(|e| RetryableError::Template(e.to_string()))?;

        Ok(Some(rendered.into_bytes()))
    }

    /// Builds the HTTP request for the given changes.
    fn build_request(&self, changes: &[IpChange]) -> Result<HttpRequest, RetryableError> {
        let mut request = HttpRequest::new(self.method.clone(), self.url.clone());

        // Copy headers
        for (name, value) in &self.headers {
            request.headers.append(name, value.clone());
        }

        // Add body if template is configured
        if let Some(body) = self.render_body(changes)? {
            request.body = Some(body);
        }

        Ok(request)
    }

    /// Executes a single request attempt.
    async fn execute_request(&self, request: &HttpRequest) -> Result<(), RetryableError> {
        let response = self.client.request(request.clone()).await?;

        if response.is_success() {
            return Ok(());
        }

        Err(RetryableError::NonSuccessStatus {
            status: response.status,
            body: response.body_text().map(ToString::to_string),
        })
    }

    /// Sends with retry logic.
    async fn send_with_retry(&self, changes: &[IpChange]) -> Result<(), WebhookError> {
        let request = self.build_request(changes)?;

        let mut last_error: Option<RetryableError> = None;

        for attempt in 1..=self.retry_policy.max_attempts {
            match self.execute_request(&request).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // Non-retryable errors fail immediately
                    if !e.is_retryable() {
                        return Err(e.into());
                    }

                    last_error = Some(e);

                    // Don't sleep after the last attempt
                    if self.retry_policy.should_retry(attempt) {
                        let delay = self.retry_policy.delay_for_retry(attempt - 1);
                        self.sleeper.sleep(delay).await;
                    }
                }
            }
        }

        Err(WebhookError::MaxRetriesExceeded {
            attempts: self.retry_policy.max_attempts,
            last_error: last_error.expect("max_attempts >= 1 ensures at least one attempt"),
        })
    }
}

impl<H: HttpClient, S: Sleeper> WebhookSender for HttpWebhook<H, S> {
    async fn send(&self, changes: &[IpChange]) -> Result<(), WebhookError> {
        self.send_with_retry(changes).await
    }
}

/// Extension trait for checking if an error is retryable.
///
/// Determines whether an error represents a transient failure that
/// warrants a retry attempt. Used by [`HttpWebhook`] to decide whether
/// to continue retrying after a failure.
pub trait IsRetryable {
    /// Returns true if the error is potentially transient and should be retried.
    fn is_retryable(&self) -> bool;
}

impl IsRetryable for HttpError {
    fn is_retryable(&self) -> bool {
        match self {
            // Network errors are typically transient
            Self::Connection(_) | Self::Timeout => true,
            // URL errors are configuration issues, not transient
            Self::InvalidUrl(_) => false,
        }
    }
}

impl IsRetryable for RetryableError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Http(e) => e.is_retryable(),
            // Server errors (5xx) are typically transient
            // Rate limiting (429) is retryable
            // Some 4xx (408 Request Timeout) are retryable
            Self::NonSuccessStatus { status, .. } => {
                status.is_server_error()
                    || *status == http::StatusCode::TOO_MANY_REQUESTS
                    || *status == http::StatusCode::REQUEST_TIMEOUT
            }
            // Template errors are not retryable (configuration issue)
            Self::Template(_) => false,
        }
    }
}
