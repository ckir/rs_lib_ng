//! # Nasdaq API Caller
//!
//! This module provides a production-ready interface for communicating with 
//! Nasdaq API endpoints, handling mandatory headers, and validating business-level status codes.

use reqwest::header::{HeaderMap, HeaderValue, HeaderName};
use serde_json::Value;
use crate::retrieve::ky_http::{KyHttp, KyOptions};
use crate::core::error::NgError;
use crate::loggers::Logger; // Using the public re-export
use crate::warn;

/// Adapter for the Nasdaq API providing robust error handling and header management.
pub struct NasdaqApi {
    /// Internal resilient HTTP client instance.
    http: KyHttp,
    /// Logger handle for structured diagnostic output.
    logger: Logger,
}

impl NasdaqApi {
    /// Creates a new instance of `NasdaqApi`.
    ///
    /// # Arguments
    ///
    /// * `logger` - A cloneable `Logger` instance used for all internal telemetry.
    pub fn new(logger: Logger) -> Self {
        Self {
            http: KyHttp::new(logger.clone()),
            logger,
        }
    }

    /// Internal helper to construct the mandatory headers required for Nasdaq API requests.
    fn get_nasdaq_headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
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
            if let (Ok(name), Ok(value)) = (k.parse::<HeaderName>(), HeaderValue::from_str(v)) {
                h.insert(name, value);
            }
        }
        h
    }

    /// Executes an API call to Nasdaq with validation and support for custom retry/timeout options.
    ///
    /// This method validates that the response is valid JSON and that the internal 
    /// `rCode` is 200. If an override for `KyOptions` is provided, a transient 
    /// HTTP instance is created for that specific call.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The full URL string for the Nasdaq endpoint.
    /// * `options` - Optional `KyOptions` to override global client settings.
    pub async fn call(&self, endpoint: &str, options: Option<KyOptions>) -> Result<Value, NgError> {
        // Simple validation for the endpoint parameter
        if !endpoint.starts_with("http") {
            return Err(NgError::HttpError(format!("Invalid URL provided: {}", endpoint)));
        }

        // Logic to handle transient options via the "new instance way" per company policy
        let api_resp = if let Some(opts) = options {
            // Fix: Wrapping opts in Some() to match KyHttp::new_with_opts signature
            let transient_http = KyHttp::new_with_opts(self.logger.clone(), Some(opts));
            transient_http.get::<Value>(endpoint, self.get_nasdaq_headers()).await?
        } else {
            // Use the shared persistent instance
            self.http.get::<Value>(endpoint, self.get_nasdaq_headers()).await?
        };

        // 1. Check for valid JSON content (success flag indicates parsing succeeded)
        if !api_resp.success {
            let body_str = api_resp.error_body.as_deref().unwrap_or("");
            let snippet = if body_str.len() > 200 { &body_str[..200] } else { body_str };
            
            warn!(
                self.logger, 
                "Nasdaq API returned non-JSON content or HTTP error",
                "url" => endpoint,
                "status" => api_resp.status,
                "body_snippet" => snippet
            );

            return Err(NgError::NonJsonResponse {
                url: endpoint.to_string(),
                status: api_resp.status,
                body_snippet: snippet.to_string(),
            });
        }

        let body = api_resp.data.clone().unwrap_or(Value::Null);

        // 2. Validate the internal business status block and rCode
        // Fix: Added explicit type annotations for closure parameters to assist type inference
        let r_code = body.get("status")
            .and_then(|s: &Value| s.get("rCode"))
            .and_then(|r: &Value| r.as_i64());

        match r_code {
            Some(200) => Ok(body),
            Some(code) => {
                // Business Error: Strip "data" field to provide metadata-only context in logs
                let mut error_meta = body.clone();
                if let Some(obj) = error_meta.as_object_mut() {
                    obj.remove("data");
                }

                warn!(
                    self.logger, 
                    "Nasdaq Business Level Error detected", 
                    "rCode" => code, 
                    "url" => endpoint,
                    "context" => error_meta.to_string()
                );

                Err(NgError::NasdaqBusinessError {
                    r_code: code,
                    endpoint: endpoint.to_string(),
                    response: body,
                })
            }
            None => {
                // The structure does not follow the expected Nasdaq format
                warn!(self.logger, "Malformed Nasdaq response structure", "url" => endpoint);
                Err(NgError::MalformedResponse {
                    endpoint: endpoint.to_string(),
                    details: "Missing 'rCode' in response status block".into(),
                })
            }
        }
    }
}