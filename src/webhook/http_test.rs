//! Tests for HTTP request/response types.

use super::{HttpClient, HttpError, HttpRequest, HttpResponse};

mod http_request {
    use super::*;

    #[test]
    fn new_creates_request_with_method_and_url() {
        let url = url::Url::parse("https://example.com/api").unwrap();
        let req = HttpRequest::new(http::Method::PUT, url.clone());

        assert_eq!(req.method, http::Method::PUT);
        assert_eq!(req.url, url);
        assert!(req.headers.is_empty());
        assert!(req.body.is_none());
    }

    #[test]
    fn get_creates_get_request() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let req = HttpRequest::get(url);

        assert_eq!(req.method, http::Method::GET);
    }

    #[test]
    fn post_creates_post_request() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let req = HttpRequest::post(url);

        assert_eq!(req.method, http::Method::POST);
    }

    #[test]
    fn with_body_sets_body() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let body = b"test body".to_vec();
        let req = HttpRequest::post(url).with_body(body.clone());

        assert_eq!(req.body, Some(body));
    }

    #[test]
    fn with_header_adds_single_header() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let req = HttpRequest::get(url).with_header(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );

        assert_eq!(
            req.headers.get(http::header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }

    #[test]
    fn with_header_appends_multiple_values_for_same_name() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let req = HttpRequest::get(url)
            .with_header(
                http::header::ACCEPT,
                http::HeaderValue::from_static("text/html"),
            )
            .with_header(
                http::header::ACCEPT,
                http::HeaderValue::from_static("application/json"),
            );

        assert_eq!(req.headers.get_all(http::header::ACCEPT).iter().count(), 2);
    }

    #[test]
    fn builder_pattern_chains_correctly() {
        let url = url::Url::parse("https://example.com/api").unwrap();
        let req = HttpRequest::post(url)
            .with_body(b"data".to_vec())
            .with_header(
                http::header::AUTHORIZATION,
                http::HeaderValue::from_static("Bearer token"),
            );

        assert_eq!(req.method, http::Method::POST);
        assert_eq!(req.body, Some(b"data".to_vec()));
        assert!(req.headers.contains_key(http::header::AUTHORIZATION));
    }

    #[test]
    fn clone_creates_independent_copy() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let req1 = HttpRequest::post(url).with_body(b"original".to_vec());
        let req2 = req1.clone();

        assert_eq!(req1.body, req2.body);
        assert_eq!(req1.method, req2.method);
    }

    #[test]
    fn debug_format_is_readable() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let req = HttpRequest::get(url);
        let debug = format!("{req:?}");

        assert!(debug.contains("HttpRequest"));
        assert!(debug.contains("GET"));
    }
}

mod http_response {
    use super::*;

    #[test]
    fn new_creates_response_with_all_fields() {
        let status = http::StatusCode::OK;
        let headers = http::HeaderMap::new();
        let body = b"response body".to_vec();
        let resp = HttpResponse::new(status, headers, body.clone());

        assert_eq!(resp.status, http::StatusCode::OK);
        assert!(resp.headers.is_empty());
        assert_eq!(resp.body, body);
    }

    #[test]
    fn is_success_returns_true_for_2xx() {
        let statuses = [
            http::StatusCode::OK,
            http::StatusCode::CREATED,
            http::StatusCode::ACCEPTED,
            http::StatusCode::NO_CONTENT,
        ];

        for status in statuses {
            let resp = HttpResponse::new(status, http::HeaderMap::new(), vec![]);
            assert!(resp.is_success(), "Expected {status} to be success");
        }
    }

    #[test]
    fn is_success_returns_false_for_non_2xx() {
        let statuses = [
            http::StatusCode::BAD_REQUEST,
            http::StatusCode::UNAUTHORIZED,
            http::StatusCode::NOT_FOUND,
            http::StatusCode::INTERNAL_SERVER_ERROR,
        ];

        for status in statuses {
            let resp = HttpResponse::new(status, http::HeaderMap::new(), vec![]);
            assert!(!resp.is_success(), "Expected {status} to not be success");
        }
    }

    #[test]
    fn body_text_returns_valid_utf8() {
        let body = b"Hello, World!".to_vec();
        let resp = HttpResponse::new(http::StatusCode::OK, http::HeaderMap::new(), body);

        assert_eq!(resp.body_text(), Some("Hello, World!"));
    }

    #[test]
    fn body_text_returns_none_for_invalid_utf8() {
        let body = vec![0xFF, 0xFE]; // Invalid UTF-8
        let resp = HttpResponse::new(http::StatusCode::OK, http::HeaderMap::new(), body);

        assert!(resp.body_text().is_none());
    }

    #[test]
    fn body_text_returns_empty_string_for_empty_body() {
        let resp = HttpResponse::new(http::StatusCode::OK, http::HeaderMap::new(), vec![]);

        assert_eq!(resp.body_text(), Some(""));
    }

    #[test]
    fn clone_creates_independent_copy() {
        let body = b"data".to_vec();
        let resp1 = HttpResponse::new(http::StatusCode::OK, http::HeaderMap::new(), body);
        let resp2 = resp1.clone();

        assert_eq!(resp1.status, resp2.status);
        assert_eq!(resp1.body, resp2.body);
    }

    #[test]
    fn debug_format_is_readable() {
        let resp = HttpResponse::new(http::StatusCode::OK, http::HeaderMap::new(), vec![]);
        let debug = format!("{resp:?}");

        assert!(debug.contains("HttpResponse"));
        assert!(debug.contains("200"));
    }
}

mod http_error {
    use super::*;
    use std::error::Error;

    #[test]
    fn connection_error_displays_message() {
        let source = std::io::Error::other("network unavailable");
        let error = HttpError::Connection(Box::new(source));

        assert!(error.to_string().contains("Connection error"));
    }

    #[test]
    fn connection_error_preserves_source() {
        let source = std::io::Error::other("network unavailable");
        let error = HttpError::Connection(Box::new(source));

        assert!(error.source().is_some());
        assert!(
            error
                .source()
                .unwrap()
                .to_string()
                .contains("network unavailable")
        );
    }

    #[test]
    fn timeout_displays_message() {
        let error = HttpError::Timeout;
        assert_eq!(error.to_string(), "Request timed out");
    }

    #[test]
    fn timeout_has_no_source() {
        let error = HttpError::Timeout;
        assert!(error.source().is_none());
    }

    #[test]
    fn invalid_url_displays_message() {
        let error = HttpError::InvalidUrl("missing scheme".to_string());

        assert!(error.to_string().contains("Invalid URL"));
        assert!(error.to_string().contains("missing scheme"));
    }

    #[test]
    fn invalid_url_has_no_source() {
        let error = HttpError::InvalidUrl("bad".to_string());
        assert!(error.source().is_none());
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HttpError>();
    }
}

mod http_client_trait {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Mock client for testing the trait.
    struct MockClient {
        response: HttpResponse,
        call_count: Arc<AtomicUsize>,
    }

    impl MockClient {
        fn new(response: HttpResponse) -> Self {
            Self {
                response,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn calls(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl HttpClient for MockClient {
        async fn request(&self, _req: HttpRequest) -> Result<HttpResponse, HttpError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn mock_client_returns_configured_response() {
        let response = HttpResponse::new(
            http::StatusCode::CREATED,
            http::HeaderMap::new(),
            b"created".to_vec(),
        );
        let client = MockClient::new(response);

        let url = url::Url::parse("https://example.com/").unwrap();
        let result = client.request(HttpRequest::get(url)).await.unwrap();

        assert_eq!(result.status, http::StatusCode::CREATED);
        assert_eq!(result.body, b"created".to_vec());
    }

    #[tokio::test]
    async fn mock_client_tracks_call_count() {
        let response = HttpResponse::new(http::StatusCode::OK, http::HeaderMap::new(), vec![]);
        let client = MockClient::new(response);
        let url = url::Url::parse("https://example.com/").unwrap();

        client.request(HttpRequest::get(url.clone())).await.unwrap();
        client.request(HttpRequest::get(url.clone())).await.unwrap();
        client.request(HttpRequest::get(url)).await.unwrap();

        assert_eq!(client.calls(), 3);
    }

    /// Error-returning mock for testing error paths.
    struct FailingClient {
        error_type: &'static str,
    }

    impl HttpClient for FailingClient {
        async fn request(&self, _req: HttpRequest) -> Result<HttpResponse, HttpError> {
            match self.error_type {
                "timeout" => Err(HttpError::Timeout),
                "connection" => Err(HttpError::Connection(Box::new(std::io::Error::other(
                    "refused",
                )))),
                _ => Err(HttpError::InvalidUrl("bad".to_string())),
            }
        }
    }

    #[tokio::test]
    async fn failing_client_returns_timeout_error() {
        let client = FailingClient {
            error_type: "timeout",
        };
        let url = url::Url::parse("https://example.com/").unwrap();

        let result = client.request(HttpRequest::get(url)).await;

        assert!(matches!(result, Err(HttpError::Timeout)));
    }

    #[tokio::test]
    async fn failing_client_returns_connection_error() {
        let client = FailingClient {
            error_type: "connection",
        };
        let url = url::Url::parse("https://example.com/").unwrap();

        let result = client.request(HttpRequest::get(url)).await;

        assert!(matches!(result, Err(HttpError::Connection(_))));
    }

    #[test]
    fn trait_is_send_sync() {
        // Verify the trait requires Send + Sync bounds
        fn assert_send_sync<T: HttpClient>() {}
        assert_send_sync::<MockClient>();
        assert_send_sync::<FailingClient>();
    }
}
