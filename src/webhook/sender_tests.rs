//! Tests for `WebhookSender` and `HttpWebhook`.

use super::sender::{HttpWebhook, IsRetryable, WebhookSender};
use super::{HttpClient, HttpError, HttpRequest, HttpResponse, RetryPolicy, RetryableError};
use crate::monitor::IpChange;
use crate::time::InstantSleeper;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

/// Mock HTTP client that returns a configurable sequence of responses.
#[derive(Debug)]
struct MockClient {
    responses: std::sync::Mutex<Vec<Result<HttpResponse, HttpError>>>,
    requests: std::sync::Mutex<Vec<HttpRequest>>,
    call_count: AtomicUsize,
}

impl MockClient {
    fn new(responses: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self {
            responses: std::sync::Mutex::new(responses),
            requests: std::sync::Mutex::new(Vec::new()),
            call_count: AtomicUsize::new(0),
        }
    }

    fn success() -> Self {
        Self::new(vec![Ok(HttpResponse::new(
            http::StatusCode::OK,
            http::HeaderMap::new(),
            vec![],
        ))])
    }

    fn failing_then_success(failures: usize) -> Self {
        let mut responses = Vec::new();
        for _ in 0..failures {
            responses.push(Err(HttpError::Timeout));
        }
        responses.push(Ok(HttpResponse::new(
            http::StatusCode::OK,
            http::HeaderMap::new(),
            vec![],
        )));
        Self::new(responses)
    }

    fn always_failing() -> Self {
        Self::new(vec![
            Err(HttpError::Timeout),
            Err(HttpError::Timeout),
            Err(HttpError::Timeout),
            Err(HttpError::Timeout),
            Err(HttpError::Timeout),
        ])
    }

    fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    fn captured_requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl HttpClient for MockClient {
    async fn request(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.requests.lock().unwrap().push(req);
        self.responses.lock().unwrap().remove(0)
    }
}

impl HttpClient for Arc<MockClient> {
    async fn request(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        (**self).request(req).await
    }
}

fn test_url() -> url::Url {
    url::Url::parse("https://example.com/webhook").unwrap()
}

fn test_changes() -> Vec<IpChange> {
    vec![IpChange::added(
        "eth0",
        "192.168.1.1".parse::<IpAddr>().unwrap(),
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
    )]
}

mod http_webhook_builder {
    use super::*;

    #[test]
    fn new_creates_webhook_with_defaults() {
        let client = MockClient::success();
        let webhook = HttpWebhook::new(client, test_url());

        assert_eq!(webhook.url().as_str(), "https://example.com/webhook");
        assert_eq!(*webhook.method(), http::Method::POST);
    }

    #[test]
    fn with_method_sets_method() {
        let client = MockClient::success();
        let webhook = HttpWebhook::new(client, test_url()).with_method(http::Method::PUT);

        assert_eq!(*webhook.method(), http::Method::PUT);
    }

    #[test]
    fn with_headers_sets_headers() {
        let client = MockClient::success();
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_static("Bearer token"),
        );

        let webhook = HttpWebhook::new(client, test_url()).with_headers(headers);

        // The webhook stores headers, verified through captured request in send test
        assert_eq!(*webhook.method(), http::Method::POST); // Still has default method
    }

    #[test]
    fn with_body_template_sets_template() {
        let client = MockClient::success();
        let webhook =
            HttpWebhook::new(client, test_url()).with_body_template(r#"{"changes": {{changes}}}"#);

        // Template is used during send, tested in send tests
        assert_eq!(*webhook.method(), http::Method::POST);
    }

    #[test]
    fn with_retry_policy_sets_policy() {
        let client = MockClient::success();
        let policy = RetryPolicy::new().with_max_attempts(5);
        let webhook = HttpWebhook::new(client, test_url()).with_retry_policy(policy);

        assert_eq!(webhook.retry_policy().max_attempts, 5);
    }

    #[test]
    fn builder_chains_correctly() {
        let client = MockClient::success();
        let webhook = HttpWebhook::new(client, test_url())
            .with_method(http::Method::PATCH)
            .with_body_template("{}")
            .with_retry_policy(RetryPolicy::new().with_max_attempts(10));

        assert_eq!(*webhook.method(), http::Method::PATCH);
        assert_eq!(webhook.retry_policy().max_attempts, 10);
    }
}

mod http_webhook_send {
    use super::*;

    #[tokio::test]
    async fn sends_request_to_configured_url() {
        let client = Arc::new(MockClient::success());
        let webhook = HttpWebhook::new(client.clone(), test_url());

        webhook.send(&test_changes()).await.unwrap();

        let requests = client.captured_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url.as_str(), "https://example.com/webhook");
    }

    #[tokio::test]
    async fn uses_configured_method() {
        let client = Arc::new(MockClient::success());
        let webhook = HttpWebhook::new(client.clone(), test_url()).with_method(http::Method::PUT);

        webhook.send(&test_changes()).await.unwrap();

        let requests = client.captured_requests();
        assert_eq!(requests[0].method, http::Method::PUT);
    }

    #[tokio::test]
    async fn includes_configured_headers() {
        let client = Arc::new(MockClient::success());
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );

        let webhook = HttpWebhook::new(client.clone(), test_url()).with_headers(headers);
        webhook.send(&test_changes()).await.unwrap();

        let requests = client.captured_requests();
        assert_eq!(
            requests[0].headers.get(http::header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn renders_body_template() {
        let client = Arc::new(MockClient::success());
        // Use valid Handlebars syntax - iterate over changes array
        let template =
            r#"{"changes":[{{#each changes}}"{{address}}"{{#unless @last}},{{/unless}}{{/each}}]}"#;

        let webhook = HttpWebhook::new(client.clone(), test_url()).with_body_template(template);
        webhook.send(&test_changes()).await.unwrap();

        let requests = client.captured_requests();
        assert!(requests[0].body.is_some());
        let body = String::from_utf8(requests[0].body.clone().unwrap()).unwrap();
        assert!(body.contains("192.168.1.1"));
    }

    #[tokio::test]
    async fn empty_changes_still_sends() {
        let client = Arc::new(MockClient::success());
        let webhook = HttpWebhook::new(client.clone(), test_url());

        webhook.send(&[]).await.unwrap();

        assert_eq!(client.calls(), 1);
    }

    #[tokio::test]
    async fn success_response_returns_ok() {
        let client = MockClient::success();
        let webhook = HttpWebhook::new(client, test_url());

        let result = webhook.send(&test_changes()).await;
        assert!(result.is_ok());
    }
}

mod http_webhook_retry {
    use super::*;

    #[tokio::test]
    async fn retries_on_timeout() {
        let client = Arc::new(MockClient::failing_then_success(1));
        let policy = RetryPolicy::new().with_max_attempts(3);

        let webhook = HttpWebhook::new(client.clone(), test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        assert!(result.is_ok());
        assert_eq!(client.calls(), 2); // 1 failure + 1 success
    }

    #[tokio::test]
    async fn retries_on_server_error() {
        let responses = vec![
            Ok(HttpResponse::new(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                http::HeaderMap::new(),
                vec![],
            )),
            Ok(HttpResponse::new(
                http::StatusCode::OK,
                http::HeaderMap::new(),
                vec![],
            )),
        ];
        let client = Arc::new(MockClient::new(responses));
        let policy = RetryPolicy::new().with_max_attempts(3);

        let webhook = HttpWebhook::new(client.clone(), test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        assert!(result.is_ok());
        assert_eq!(client.calls(), 2);
    }

    #[tokio::test]
    async fn fails_after_max_retries() {
        let client = Arc::new(MockClient::always_failing());
        let policy = RetryPolicy::new().with_max_attempts(3);

        let webhook = HttpWebhook::new(client.clone(), test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        assert!(result.is_err());
        assert_eq!(client.calls(), 3);
    }

    #[tokio::test]
    async fn max_retries_exceeded_error_contains_attempt_count() {
        let client = MockClient::always_failing();
        let policy = RetryPolicy::new().with_max_attempts(3);

        let webhook = HttpWebhook::new(client, test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        match result {
            Err(super::super::WebhookError::MaxRetriesExceeded { attempts, .. }) => {
                assert_eq!(attempts, 3);
            }
            _ => panic!("Expected MaxRetriesExceeded error"),
        }
    }

    #[tokio::test]
    async fn single_attempt_no_retry() {
        let client = Arc::new(MockClient::always_failing());
        let policy = RetryPolicy::new().with_max_attempts(1);

        let webhook = HttpWebhook::new(client.clone(), test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        assert!(result.is_err());
        assert_eq!(client.calls(), 1);
    }
}

mod http_webhook_error_handling {
    use super::*;

    #[tokio::test]
    async fn connection_error_is_retried() {
        let responses = vec![
            Err(HttpError::Connection(Box::new(std::io::Error::other(
                "refused",
            )))),
            Ok(HttpResponse::new(
                http::StatusCode::OK,
                http::HeaderMap::new(),
                vec![],
            )),
        ];
        let client = Arc::new(MockClient::new(responses));
        let policy = RetryPolicy::new().with_max_attempts(2);

        let webhook = HttpWebhook::new(client.clone(), test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn client_error_4xx_is_not_retried() {
        // 4xx errors (except 408, 429) are not retried - they indicate client issues
        let responses = vec![
            Ok(HttpResponse::new(
                http::StatusCode::BAD_REQUEST,
                http::HeaderMap::new(),
                vec![],
            )),
            Ok(HttpResponse::new(
                http::StatusCode::OK,
                http::HeaderMap::new(),
                vec![],
            )),
        ];
        let client = Arc::new(MockClient::new(responses));
        let policy = RetryPolicy::new().with_max_attempts(2);

        let webhook = HttpWebhook::new(client.clone(), test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        // Should fail immediately without retry
        assert!(result.is_err());
        assert_eq!(client.calls(), 1);
    }

    #[tokio::test]
    async fn status_429_is_retried() {
        // 429 Too Many Requests is retryable
        let responses = vec![
            Ok(HttpResponse::new(
                http::StatusCode::TOO_MANY_REQUESTS,
                http::HeaderMap::new(),
                vec![],
            )),
            Ok(HttpResponse::new(
                http::StatusCode::OK,
                http::HeaderMap::new(),
                vec![],
            )),
        ];
        let client = Arc::new(MockClient::new(responses));
        let policy = RetryPolicy::new().with_max_attempts(2);

        let webhook = HttpWebhook::new(client.clone(), test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        assert!(result.is_ok());
        assert_eq!(client.calls(), 2);
    }

    #[tokio::test]
    async fn status_408_is_retried() {
        // 408 Request Timeout is retryable
        let responses = vec![
            Ok(HttpResponse::new(
                http::StatusCode::REQUEST_TIMEOUT,
                http::HeaderMap::new(),
                vec![],
            )),
            Ok(HttpResponse::new(
                http::StatusCode::OK,
                http::HeaderMap::new(),
                vec![],
            )),
        ];
        let client = Arc::new(MockClient::new(responses));
        let policy = RetryPolicy::new().with_max_attempts(2);

        let webhook = HttpWebhook::new(client.clone(), test_url())
            .with_sleeper(InstantSleeper)
            .with_retry_policy(policy);
        let result = webhook.send(&test_changes()).await;

        assert!(result.is_ok());
        assert_eq!(client.calls(), 2);
    }
}

mod template_rendering {
    use super::*;

    #[tokio::test]
    async fn renders_changes_array() {
        let client = Arc::new(MockClient::success());
        let template =
            r#"[{{#each changes}}{"addr":"{{address}}"}{{#unless @last}},{{/unless}}{{/each}}]"#;

        let changes = vec![
            IpChange::added(
                "eth0",
                "192.168.1.1".parse().unwrap(),
                SystemTime::UNIX_EPOCH,
            ),
            IpChange::removed("eth1", "10.0.0.1".parse().unwrap(), SystemTime::UNIX_EPOCH),
        ];

        let webhook = HttpWebhook::new(client.clone(), test_url()).with_body_template(template);
        webhook.send(&changes).await.unwrap();

        let requests = client.captured_requests();
        let body = String::from_utf8(requests[0].body.clone().unwrap()).unwrap();
        assert!(body.contains("192.168.1.1"));
        assert!(body.contains("10.0.0.1"));
    }

    #[tokio::test]
    async fn renders_change_kind() {
        let client = Arc::new(MockClient::success());
        let template = r"{{#each changes}}{{kind}}{{/each}}";

        let changes = vec![
            IpChange::added(
                "eth0",
                "192.168.1.1".parse().unwrap(),
                SystemTime::UNIX_EPOCH,
            ),
            IpChange::removed("eth1", "10.0.0.1".parse().unwrap(), SystemTime::UNIX_EPOCH),
        ];

        let webhook = HttpWebhook::new(client.clone(), test_url()).with_body_template(template);
        webhook.send(&changes).await.unwrap();

        let requests = client.captured_requests();
        let body = String::from_utf8(requests[0].body.clone().unwrap()).unwrap();
        assert!(body.contains("added"));
        assert!(body.contains("removed"));
    }

    #[tokio::test]
    async fn invalid_template_returns_error() {
        let client = MockClient::success();
        let template = "{{#if}}"; // Invalid Handlebars

        let webhook = HttpWebhook::new(client, test_url()).with_body_template(template);
        let result = webhook.send(&test_changes()).await;

        assert!(result.is_err());
    }
}

mod is_retryable_trait {
    use super::*;

    #[test]
    fn connection_error_is_retryable() {
        let error = HttpError::Connection(Box::new(std::io::Error::other("network")));
        assert!(error.is_retryable());
    }

    #[test]
    fn timeout_is_retryable() {
        let error = HttpError::Timeout;
        assert!(error.is_retryable());
    }

    #[test]
    fn invalid_url_is_not_retryable() {
        let error = HttpError::InvalidUrl("bad url".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn retryable_error_http_delegates() {
        let error = RetryableError::Http(HttpError::Timeout);
        assert!(error.is_retryable());

        let error = RetryableError::Http(HttpError::InvalidUrl("bad".to_string()));
        assert!(!error.is_retryable());
    }

    #[test]
    fn status_500_is_retryable() {
        let error = RetryableError::NonSuccessStatus {
            status: http::StatusCode::INTERNAL_SERVER_ERROR,
            body: None,
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn status_503_is_retryable() {
        let error = RetryableError::NonSuccessStatus {
            status: http::StatusCode::SERVICE_UNAVAILABLE,
            body: None,
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn status_429_is_retryable() {
        let error = RetryableError::NonSuccessStatus {
            status: http::StatusCode::TOO_MANY_REQUESTS,
            body: None,
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn status_408_is_retryable() {
        let error = RetryableError::NonSuccessStatus {
            status: http::StatusCode::REQUEST_TIMEOUT,
            body: None,
        };
        assert!(error.is_retryable());
    }

    #[test]
    fn status_400_is_not_retryable() {
        let error = RetryableError::NonSuccessStatus {
            status: http::StatusCode::BAD_REQUEST,
            body: None,
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn status_404_is_not_retryable() {
        let error = RetryableError::NonSuccessStatus {
            status: http::StatusCode::NOT_FOUND,
            body: None,
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn template_error_is_not_retryable() {
        let error = RetryableError::Template("bad template".to_string());
        assert!(!error.is_retryable());
    }
}

mod error_display {
    use super::*;
    use std::error::Error;

    #[test]
    fn retryable_error_http_displays_source() {
        let error = RetryableError::Http(HttpError::Timeout);
        assert!(error.to_string().contains("timed out"));
    }

    #[test]
    fn retryable_error_non_success_status_displays_code_and_body() {
        let error = RetryableError::NonSuccessStatus {
            status: http::StatusCode::INTERNAL_SERVER_ERROR,
            body: Some("Internal error".to_string()),
        };
        assert!(error.to_string().contains("500"));
        assert!(error.to_string().contains("Internal error"));
    }

    #[test]
    fn retryable_error_non_success_status_without_body() {
        let error = RetryableError::NonSuccessStatus {
            status: http::StatusCode::INTERNAL_SERVER_ERROR,
            body: None,
        };
        let display = error.to_string();
        assert!(display.contains("500"));
        assert!(display.contains("<no body>"));
    }

    #[test]
    fn webhook_error_max_retries_displays_attempts() {
        let error = super::super::WebhookError::MaxRetriesExceeded {
            attempts: 5,
            last_error: RetryableError::Http(HttpError::Timeout),
        };
        assert!(error.to_string().contains("5 attempts"));
    }

    #[test]
    fn webhook_error_max_retries_has_source() {
        let error = super::super::WebhookError::MaxRetriesExceeded {
            attempts: 3,
            last_error: RetryableError::Http(HttpError::Timeout),
        };
        assert!(error.source().is_some());
    }
}

mod traits {
    use super::*;

    #[test]
    fn webhook_sender_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HttpWebhook<MockClient>>();
    }

    #[test]
    fn http_webhook_debug_is_readable() {
        let client = MockClient::success();
        let webhook = HttpWebhook::new(client, test_url());
        let debug = format!("{webhook:?}");

        assert!(debug.contains("HttpWebhook"));
    }
}
