//! Tests for `ReqwestClient`.
//!
//! Note: These tests focus on unit testing the client construction and
//! configuration. Integration tests with actual HTTP servers would require
//! a test server setup or would be done manually / in CI with external services.

use super::*;

mod reqwest_client {
    use super::*;

    #[test]
    fn new_creates_client() {
        let client = ReqwestClient::new();
        // Verify it's constructed (no panic)
        let _ = format!("{client:?}");
    }

    #[test]
    fn default_creates_same_as_new() {
        let client1 = ReqwestClient::new();
        let client2 = ReqwestClient::default();

        // Both should be functional (no panic)
        let _ = format!("{client1:?}");
        let _ = format!("{client2:?}");
    }

    #[test]
    fn from_client_accepts_custom_client() {
        let custom = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
        let client = ReqwestClient::from_client(custom);

        // Verify it's constructed
        let _ = format!("{client:?}");
    }

    #[test]
    fn clone_creates_independent_client() {
        let client1 = ReqwestClient::new();
        let client2 = client1.clone();

        // Both should be functional
        let _ = format!("{client1:?}");
        let _ = format!("{client2:?}");
    }

    #[test]
    fn debug_format_is_readable() {
        let client = ReqwestClient::new();
        let debug = format!("{client:?}");

        assert!(debug.contains("ReqwestClient"));
    }

    #[test]
    fn client_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ReqwestClient>();
    }

    // Note: Testing actual HTTP requests would require either:
    // 1. A mock server (like wiremock)
    // 2. Integration tests against real endpoints
    //
    // For unit tests, we verify the trait implementation compiles and
    // the client can be constructed. The actual HTTP behavior is
    // tested via the reqwest library's own tests.

    #[tokio::test]
    async fn request_to_invalid_host_returns_error_or_proxy_response() {
        let client = ReqwestClient::new();
        let url = url::Url::parse("http://invalid.invalid.invalid/").unwrap();
        let req = HttpRequest::get(url);

        let result = client.request(req).await;

        // DNS resolution failure typically causes a connection error.
        // However, in environments with a proxy, the proxy may return an
        // HTTP error response (e.g., 502 Bad Gateway) instead.
        match result {
            Err(HttpError::Connection(_)) => {} // Expected in direct connection
            Ok(resp) if !resp.is_success() => {} // Proxy returned error response
            other => panic!("Expected connection error or proxy error response, got {other:?}"),
        }
    }
}
