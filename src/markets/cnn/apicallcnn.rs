//! # CNN Business API Adapter
//!
//! This module provides a resilient interface for interacting with CNN Business 
//! data services. It mirrors the design of the Nasdaq API caller, supporting 
//! browser-mimicry headers and per-request configuration overrides.

use reqwest::header::{HeaderMap, HeaderValue, HeaderName};
use serde_json::Value;
use crate::retrieve::ky_http::{KyHttp, KyOptions};
use crate::core::error::NgError;
use crate::loggers::Logger;
use crate::warn;

/// Adapter for CNN APIs supporting flexible endpoints and custom header management.
///
/// This struct wraps a `KyHttp` client and maintains its own set of headers to 
/// ensure that all requests to CNN services appear consistent and authenticated.
pub struct CnnApi {
    /// Resilient HTTP client with retry logic and telemetry.
    http: KyHttp,
    /// Shared logger for structured diagnostic events.
    logger: Logger,
    /// Internal storage for request headers.
    headers: HeaderMap,
}

impl CnnApi {
    /// Creates a new `CnnApi` instance with default browser-mimicry headers.
    /// 
    /// # Arguments
    /// * `logger` - A [`Logger`] instance used for reporting request status and errors.
    pub fn new(logger: Logger) -> Self {
        let mut api = Self {
            http: KyHttp::new(logger.clone()),
            logger,
            headers: HeaderMap::new(),
        };
        // Initialize with default header set
        api.set_default_headers();
        api
    }

    /// Sets the internal headers to a default set of browser-mimicry headers.
    /// 
    /// These headers mimic a standard Windows Chrome browser to prevent 
    /// requests from being flagged as automated traffic by CDN filters.
    fn set_default_headers(&mut self) {
        let headers = [
            ("authority", "api.nasdaq.com"),
            ("accept", "application/json, text/plain, */*"),
            ("accept-language", "en-US,en;q=0.9,el-GR;q=0.8,el;q=0.7,it;q=0.6"),
            ("cache-control", "no-cache"),
            ("dnt", "1"),
            ("origin", "https://www.nasdaq.com"),
            ("pragma", "no-cache"),
            ("referer", "https://www.nasdaq.com/"),
            ("sec-ch-ua", r#""Google Chrome";v="119", "Chromium";v="119", "Not?A_Brand";v="24""#),
            ("sec-ch-ua-mobile", "?0"),
            ("sec-ch-ua-platform", "\"Windows\""),
            ("sec-fetch-dest", "empty"),
            ("sec-fetch-mode", "cors"),
            ("sec-fetch-site", "same-site"),
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36"),
        ];

        for (k, v) in headers {
            if let Ok(value) = HeaderValue::from_str(v) {
                // Initializing with static strings is safe for HeaderMap
                self.headers.insert(k, value);
            }
        }
    }

    /// Updates or adds a specific header to the API caller.
    /// 
    /// # Arguments
    /// * `key` - The header name (e.g., "Authorization").
    /// * `value` - The header value string.
    pub fn set_header(&mut self, key: &str, value: &str) {
        // Attempt to parse the key into a valid HeaderName to resolve lifetime issues
        if let Ok(name) = HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(val) = HeaderValue::from_str(value) {
                // HeaderName is owned and satisfies the IntoHeaderName trait bound
                self.headers.insert(name, val);
            }
        }
    }

    /// Returns a clone of the current header set.
    ///
    /// Useful for inspecting the state of the adapter or passing headers
    /// to other internal components.
    pub fn get_headers(&self) -> HeaderMap {
        self.headers.clone()
    }

    /// Executes an asynchronous GET request to the specified CNN endpoint.
    ///
    /// This method automatically handles authentication headers and allows 
    /// for per-request overrides of the underlying HTTP client settings.
    ///
    /// # Arguments
    /// * `endpoint` - The full URL string to be called.
    /// * `options` - Optional [`KyOptions`] to override global retry or timeout settings.
    ///
    /// # Errors
    /// Returns [`NgError::NonJsonResponse`] if the server returns non-JSON content 
    /// or a non-success HTTP status code.
    pub async fn call(&self, endpoint: &str, options: Option<KyOptions>) -> Result<Value, NgError> {
        // Corrected: Uses new_with_opts to match ky_http.rs implementation
        let api_resp = if let Some(opts) = options {
            // Create a transient instance with the provided overrides
            let transient_http = KyHttp::new_with_opts(self.logger.clone(), Some(opts));
            transient_http.get::<Value>(endpoint, self.get_headers()).await?
        } else {
            // Use the persistent instance with default settings
            self.http.get::<Value>(endpoint, self.get_headers()).await?
        };

        // Validate the response status and content type
        if !api_resp.success {
            let body_str = api_resp.error_body.as_deref().unwrap_or("[No Body]");
            let snippet = if body_str.len() > 250 { &body_str[..250] } else { body_str };

            warn!(
                self.logger,
                "CNN API request failed",
                "url" => endpoint,
                "status" => api_resp.status,
                "snippet" => snippet
            );

            return Err(NgError::NonJsonResponse {
                url: endpoint.to_string(),
                status: api_resp.status,
                body_snippet: snippet.to_string(),
            });
        }

        // Return the deserialized JSON data
        Ok(api_resp.data.clone().unwrap_or(Value::Null))
    }
}