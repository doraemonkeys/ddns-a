//! HTTP request/response types and client trait.

use super::HttpError;

/// An HTTP request to be sent.
///
/// This is a value type that can be constructed and passed to any
/// [`HttpClient`] implementation. It uses standard `http` crate types
/// for method and headers, ensuring compatibility with the broader ecosystem.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub method: http::Method,
    /// Target URL
    pub url: url::Url,
    /// HTTP headers to send
    pub headers: http::HeaderMap,
    /// Optional request body
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    /// Creates a new HTTP request with the given method and URL.
    ///
    /// Headers are initialized to an empty map and body is `None`.
    #[must_use]
    pub fn new(method: http::Method, url: url::Url) -> Self {
        Self {
            method,
            url,
            headers: http::HeaderMap::new(),
            body: None,
        }
    }

    /// Creates a GET request to the given URL.
    #[must_use]
    pub fn get(url: url::Url) -> Self {
        Self::new(http::Method::GET, url)
    }

    /// Creates a POST request to the given URL.
    #[must_use]
    pub fn post(url: url::Url) -> Self {
        Self::new(http::Method::POST, url)
    }

    /// Sets the request body.
    #[must_use]
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    /// Adds a header to the request.
    ///
    /// If the header name already exists, the value is appended
    /// (HTTP headers can have multiple values).
    #[must_use]
    pub fn with_header(mut self, name: http::HeaderName, value: http::HeaderValue) -> Self {
        self.headers.append(name, value);
        self
    }
}

/// An HTTP response received from a server.
///
/// Contains the status code, headers, and body of the response.
/// The body is fully buffered into memory.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code
    pub status: http::StatusCode,
    /// Response headers
    pub headers: http::HeaderMap,
    /// Response body (fully buffered)
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Creates a new HTTP response.
    #[must_use]
    pub const fn new(status: http::StatusCode, headers: http::HeaderMap, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// Returns true if the status code indicates success (2xx).
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// Returns the body as a UTF-8 string, if valid.
    #[must_use]
    pub fn body_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }
}

/// Trait for making HTTP requests.
///
/// # Design
///
/// This trait abstracts the HTTP client implementation, enabling:
/// - Dependency injection for testing with mock clients
/// - Swapping HTTP libraries without changing calling code
/// - Adding cross-cutting concerns (logging, metrics) via decorators
///
/// # Example
///
/// ```ignore
/// use ddns_a::webhook::{HttpClient, HttpRequest, HttpResponse, HttpError};
///
/// struct MockClient {
///     response: HttpResponse,
/// }
///
/// impl HttpClient for MockClient {
///     async fn request(&self, _req: HttpRequest) -> Result<HttpResponse, HttpError> {
///         Ok(self.response.clone())
///     }
/// }
/// ```
pub trait HttpClient: Send + Sync {
    /// Sends an HTTP request and returns the response.
    ///
    /// # Arguments
    ///
    /// * `req` - The HTTP request to send
    ///
    /// # Returns
    ///
    /// The HTTP response on success, or an [`HttpError`] on failure.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError`] when:
    /// - Network connection fails ([`HttpError::Connection`])
    /// - Request times out ([`HttpError::Timeout`])
    /// - URL is invalid ([`HttpError::InvalidUrl`])
    fn request(
        &self,
        req: HttpRequest,
    ) -> impl std::future::Future<Output = Result<HttpResponse, HttpError>> + Send;
}
