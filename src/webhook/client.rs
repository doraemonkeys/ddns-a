//! Production HTTP client implementation using reqwest.

use super::{HttpClient, HttpError, HttpRequest, HttpResponse};

/// Production HTTP client using reqwest.
///
/// This is a thin wrapper around `reqwest::Client` that implements
/// the [`HttpClient`] trait. It inherits reqwest's default configuration
/// including connection pooling and reasonable timeouts.
///
/// # Example
///
/// ```no_run
/// use ddns_a::webhook::{ReqwestClient, HttpClient, HttpRequest};
/// use url::Url;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = ReqwestClient::new();
/// let url = Url::parse("https://api.example.com/webhook")?;
/// let request = HttpRequest::post(url).with_body(b"hello".to_vec());
/// let response = client.request(request).await?;
/// println!("Status: {}", response.status);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ReqwestClient {
    inner: reqwest::Client,
}

impl ReqwestClient {
    /// Creates a new HTTP client with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: reqwest::Client::new(),
        }
    }

    /// Creates an HTTP client from an existing reqwest client.
    ///
    /// Useful when you need custom configuration (timeouts, TLS, etc.).
    #[must_use]
    pub const fn from_client(client: reqwest::Client) -> Self {
        Self { inner: client }
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for ReqwestClient {
    async fn request(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        // Build the reqwest request
        let mut builder = self.inner.request(req.method, req.url.as_str());

        // Add headers
        for (name, value) in &req.headers {
            builder = builder.header(name, value);
        }

        // Add body if present
        if let Some(body) = req.body {
            builder = builder.body(body);
        }

        // Send the request
        let response = builder.send().await.map_err(|e| {
            if e.is_timeout() {
                HttpError::Timeout
            } else if e.is_builder() {
                HttpError::InvalidUrl(e.to_string())
            } else {
                HttpError::Connection(Box::new(e))
            }
        })?;

        // Extract response parts
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .bytes()
            .await
            .map_err(|e| HttpError::Connection(Box::new(e)))?
            .to_vec();

        Ok(HttpResponse::new(status, headers, body))
    }
}
