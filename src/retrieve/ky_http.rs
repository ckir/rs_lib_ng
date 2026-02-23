//! src/retrieve/ky_http.rs
//!
//! KyHttp: resilient HTTP helper with improved retry semantics, single-body read,
//! bounded permit re-acquisition, deterministic test hooks, and explicit Retry-After handling.
use crate::core::error::NgError;
use crate::loggers::Logger;
use chrono::{DateTime, Utc};
use reqwest::{header::HeaderMap, Client, Method, Request, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::{sleep, timeout};
use rand::{rngs::SmallRng, Rng, SeedableRng};

/// KyOptions
///
/// Public options for KyHttp. Includes test hooks for deterministic backoff.
#[derive(Clone)]
pub struct KyOptions {
    /// Optional timeout for the underlying reqwest client.
    pub timeout: Option<Duration>,

    /// Number of retries (not total attempts). Total attempts = retry + 1.
    pub retry: usize,

    /// Concurrency limit (number of simultaneous logical requests) used when
    /// `semaphore` is not provided.
    pub limit: usize,

    /// Status codes that are considered retryable.
    pub status_codes: HashSet<StatusCode>,

    /// Status codes that should be checked for Retry-After header.
    pub after_status_codes: HashSet<StatusCode>,

    /// Maximum allowed Retry-After duration (if set).
    pub max_retry_after: Option<Duration>,

    /// Maximum backoff limit for computed delays.
    pub backoff_limit: Option<Duration>,

    /// Whether to retry on timeout errors.
    pub retry_on_timeout: bool,

    /// Optional predicate to decide whether to retry.
    /// Receives `Option<&reqwest::Response>` (None for network errors), `&NgError`, and attempt number (1-based).
    pub should_retry:
        Option<Arc<dyn Fn(Option<&reqwest::Response>, &NgError, usize) -> bool + Send + Sync>>,

    /// Allowed HTTP methods for requests.
    pub allowed_methods: HashSet<Method>,

    /// Optional externally provided semaphore to share concurrency limits across instances.
    pub semaphore: Option<Arc<Semaphore>>,

    /// If true, backoff jitter is deterministic and small for tests.
    pub test_mode: bool,

    /// When true, disable jitter entirely.
    pub disable_jitter: bool,

    /// Threshold (ms) above which a permit will be released before sleeping.
    pub permit_release_threshold_ms: u64,
}

impl Default for KyOptions {
    fn default() -> Self {
        // default status codes: 408 413 429 500 502 503 504
        let mut status_codes = HashSet::new();
        for &c in &[408u16, 413u16, 429u16, 500u16, 502u16, 503u16, 504u16] {
            status_codes.insert(StatusCode::from_u16(c).unwrap());
        }

        // afterStatusCodes: 413, 429, 503
        let mut after_status_codes = HashSet::new();
        for &c in &[413u16, 429u16, 503u16] {
            after_status_codes.insert(StatusCode::from_u16(c).unwrap());
        }

        // allowed methods: GET, HEAD, OPTIONS by default (idempotent)
        let mut allowed_methods = HashSet::new();
        for m in &[
            Method::GET,
            Method::HEAD,
            Method::OPTIONS,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::TRACE,
        ] {            
            allowed_methods.insert(m.clone());
        }

        Self {
            timeout: Some(Duration::from_secs(15)),
            retry: 2,
            limit: 2,
            status_codes,
            after_status_codes,
            max_retry_after: None,
            backoff_limit: None,
            retry_on_timeout: false,
            should_retry: None,
            allowed_methods,
            semaphore: None,
            test_mode: false,
            disable_jitter: false,
            permit_release_threshold_ms: 2000,
        }
    }
}

/// ApiResponse<T>
///
/// Standard response wrapper returned by KyHttp methods.
#[derive(Debug)]
pub struct ApiResponse<T> {
    /// Parsed JSON body when success.
    pub data: Option<T>,

    /// Raw error body text when non-success.
    pub error_body: Option<String>,

    /// HTTP status code.
    pub status: u16,

    /// Whether the response was successful (2xx).
    pub success: bool,

    /// Response headers.
    pub headers: HeaderMap,
}

/// KyHttp
///
/// Primary HTTP helper.
#[derive(Clone)]
pub struct KyHttp {
    client: Client,
    logger: Logger,
    opts: KyOptions,
    semaphore: Arc<Semaphore>,
}

impl KyHttp {
    pub fn new(logger: Logger) -> Self {
        Self::new_with_opts(logger, None)
    }

    pub fn new_with_opts(logger: Logger, opts: Option<KyOptions>) -> Self {
        let opts = opts.unwrap_or_default();
        let mut builder = Client::builder();
        if let Some(timeout) = opts.timeout {
            builder = builder.timeout(timeout);
        }
        let client = builder.build().unwrap_or_else(|_| Client::new());

        let semaphore = if let Some(s) = &opts.semaphore {
            s.clone()
        } else {
            Arc::new(Semaphore::new(opts.limit.max(1)))
        };

        Self {
            client,
            logger,
            opts,
            semaphore,
        }
    }

    /// Prepare request hook (placeholder for auth/global headers).
    fn prepare_request(&self, rb: RequestBuilder) -> RequestBuilder {
        rb
    }

    /// Compute delay using the existing formula but with optional cap and jitter.
    fn compute_delay(&self, attempt: usize) -> Duration {
        // attempt is 1-based
        let pow = 2f64.powi((attempt as i32) - 1);
        let ms = 0.3f64 * pow * 1000.0;
        let mut dur = Duration::from_millis(ms.round() as u64);

        if let Some(limit) = self.opts.backoff_limit {
            if dur > limit {
                dur = limit;
            }
        }

        dur
    }

    /// Compute exponential backoff with optional jitter. attempt starts at 1.
    fn compute_backoff_with_jitter(&self, attempt: usize, rng: &mut SmallRng) -> Duration {
        // Compute base backoff in milliseconds
        let mut base_ms = self.compute_delay(attempt).as_millis() as u64;

        // Apply backoff_limit if present
        if let Some(limit) = self.opts.backoff_limit {
            let limit_ms = limit.as_millis() as u64;
            if base_ms > limit_ms {
                base_ms = limit_ms;
            }
        }

        // Determine jitter
        let jitter_ms = if self.opts.disable_jitter {
            0
        } else {
            let jitter_max = (base_ms / 10).max(1);
            if self.opts.test_mode {
                // small deterministic jitter in test mode
                rng.gen_range(0..=jitter_max.min(5))
            } else {
                rng.gen_range(0..=jitter_max)
            }
        };

        // Candidate backoff
        let mut candidate = base_ms.saturating_add(jitter_ms);

        // Cap by max_retry_after if set
        if let Some(max_ra) = self.opts.max_retry_after {
            let max_ra_ms = max_ra.as_millis() as u64;
            if candidate > max_ra_ms {
                candidate = max_ra_ms;
            }
        }

        // Cap by backoff_limit if set
        if let Some(limit) = self.opts.backoff_limit {
            let limit_ms = limit.as_millis() as u64;
            if candidate > limit_ms {
                candidate = limit_ms;
            }
        }

        Duration::from_millis(candidate)
    }

    /// Parse Retry-After header from headers. Supports numeric seconds and several date formats.
    fn parse_retry_after_from_headers(headers: &HeaderMap) -> Option<Duration> {
        if let Some(v) = headers.get("retry-after") {
            if let Ok(s) = v.to_str() {
                let s_trim = s.trim();

                // numeric seconds
                if let Ok(secs) = s_trim.parse::<u64>() {
                    return Some(Duration::from_secs(if secs == 0 { 1 } else { secs }));
                }

                // IMF-fixdate
                if let Ok(dt) = DateTime::parse_from_str(s_trim, "%a, %d %b %Y %H:%M:%S GMT") {
                    let dt_utc = dt.with_timezone(&Utc);
                    let now = Utc::now();
                    if dt_utc > now {
                        let diff = dt_utc.signed_duration_since(now);
                        return Some(Duration::from_secs(diff.num_seconds().max(1) as u64));
                    } else {
                        return Some(Duration::from_secs(1));
                    }
                }

                // RFC2822
                if let Ok(dt) = DateTime::parse_from_rfc2822(s_trim) {
                    let dt_utc = dt.with_timezone(&Utc);
                    let now = Utc::now();
                    if dt_utc > now {
                        let diff = dt_utc.signed_duration_since(now);
                        return Some(Duration::from_secs(diff.num_seconds().max(1) as u64));
                    } else {
                        return Some(Duration::from_secs(1));
                    }
                }

                // RFC3339
                if let Ok(dt) = DateTime::parse_from_rfc3339(s_trim) {
                    let dt_utc = dt.with_timezone(&Utc);
                    let now = Utc::now();
                    if dt_utc > now {
                        let diff = dt_utc.signed_duration_since(now);
                        return Some(Duration::from_secs(diff.num_seconds().max(1) as u64));
                    } else {
                        return Some(Duration::from_secs(1));
                    }
                }
            }
        }
        None
    }

    /// Sleep helper that releases permit for long waits and attempts a bounded re-acquire.
    async fn smart_sleep_and_maybe_reacquire(&self, duration: Duration, permit: &mut Option<OwnedSemaphorePermit>) {
        if duration.as_millis() as u64 > self.opts.permit_release_threshold_ms {
            // Release permit to avoid blocking throughput for long waits.
            let _dropped = permit.take();
            sleep(duration).await;
            // Attempt a bounded re-acquire to avoid indefinite blocking.
            if let Ok(sem_res) = timeout(Duration::from_millis(200), self.semaphore.clone().acquire_owned()).await {
                if let Ok(p) = sem_res {
                    *permit = Some(p);
                }
            }
        } else {
            // Short wait: keep permit to preserve logical ordering.
            sleep(duration).await;
        }
    }

    // Core request logic with retries and concurrency control.
    async fn request_with_retry<T, B>(
        &self,
        method: Method,
        url: &str,
        headers: HeaderMap,
        body: Option<&B>,
    ) -> Result<ApiResponse<T>, NgError>
    where
        T: DeserializeOwned + Send + 'static,
        B: Serialize + ?Sized,
    {
        // Validate allowed method
        if !self.opts.allowed_methods.contains(&method) {
            crate::error!(
                self.logger,
                "Method not allowed",
                "method" => method.as_str(),
                "url" => url
            );
            return Err(NgError::InternalError(format!(
                "Method {} not allowed",
                method.as_str()
            )));
        }

        crate::info!(
            self.logger,
            "Request start",
            "method" => method.as_str(),
            "url" => url
        );

        // total attempts = retry + 1
        let max_attempts = self.opts.retry.saturating_add(1);

        // Acquire permit once for the logical request (RAII guard)
        let mut permit: Option<OwnedSemaphorePermit> = Some(self.semaphore.clone().acquire_owned().await
            .map_err(|_| NgError::InternalError("Semaphore closed".into()))?);

        // Deterministic RNG for test_mode
        let mut rng = if self.opts.test_mode { SmallRng::seed_from_u64(0xC0FFEE) } else { SmallRng::from_entropy() };

        // last error for enriched diagnostics
        let mut last_err: Option<NgError> = None;
        let mut last_status: Option<u16> = None;
        let mut last_body_snippet: Option<String> = None;

        for attempt in 1..=max_attempts {
            if attempt > 1 {
                crate::info!(self.logger, "Retry attempt", "url" => url, "attempt" => attempt);
            }

            // Build request
            let mut rb = self.client.request(method.clone(), url).headers(headers.clone());
            if let Some(b) = body {
                rb = rb.json(b);
            }
            let rb = self.prepare_request(rb);

            // Build and execute
            let built_req_result: Result<Request, reqwest::Error> = rb.build();
            let resp_result = match built_req_result {
                Ok(req) => self.client.execute(req).await.map_err(|e| e),
                Err(e) => Err(e),
            };

            match resp_result {
                Ok(resp) => {
                    let status = resp.status();
                    let status_u16 = status.as_u16();
                    let resp_headers = resp.headers().clone();
                    // Read body once and reuse
                    let body_text = resp.text().await.unwrap_or_default();
                    let snippet = if body_text.len() > 1024 { format!("{}...[truncated]", &body_text[..1024]) } else { body_text.clone() };

                    if status.is_success() {
                        match serde_json::from_str::<T>(&body_text) {
                            Ok(parsed) => {
                                drop(permit);
                                return Ok(ApiResponse {
                                    data: Some(parsed),
                                    error_body: None,
                                    status: status_u16,
                                    success: true,
                                    headers: resp_headers,
                                });
                            }
                            Err(e) => {
                                drop(permit);
                                return Err(NgError::HttpError(format!("JSON decode: {}", e)));
                            }
                        }
                    }

                    // Non-success: decide retry behavior
                    last_status = Some(status_u16);
                    last_body_snippet = Some(snippet.clone());
                    last_err = Some(NgError::HttpError(format!("Status: {}", status_u16)));

                    // First, if this status is one of the after_status_codes, prefer honoring
                    // the server-provided Retry-After header (numeric seconds or HTTP-date).
                    if self.opts.after_status_codes.contains(&status) {
                        if let Some(retry_after) = Self::parse_retry_after_from_headers(&resp_headers) {
                            // Apply caps: prefer max_retry_after, then backoff_limit if set.
                            let capped = if let Some(max) = self.opts.max_retry_after {
                                std::cmp::min(retry_after, max)
                            } else if let Some(limit) = self.opts.backoff_limit {
                                std::cmp::min(retry_after, limit)
                            } else {
                                retry_after
                            };

                            crate::info!(
                                self.logger,
                                "Respecting Retry-After header",
                                "url" => url,
                                "retry_after_secs" => capped.as_secs()
                            );

                            // If there are attempts remaining, sleep and then continue to next attempt.
                            if attempt < max_attempts {
                                self.smart_sleep_and_maybe_reacquire(capped, &mut permit).await;
                                continue;
                            } else {
                                // This is the last configured attempt but server asked to wait.
                                // Sleep and then perform one final request attempt (instead of giving up).
                                self.smart_sleep_and_maybe_reacquire(capped, &mut permit).await;

                                // Build a fresh request and execute it as the final attempt.
                                let mut final_rb = self.client.request(method.clone(), url).headers(headers.clone());
                                if let Some(b) = body {
                                    final_rb = final_rb.json(b);
                                }
                                let final_rb = self.prepare_request(final_rb);

                                match final_rb.build() {
                                    Ok(req) => match self.client.execute(req).await {
                                        Ok(final_resp) => {
                                            let final_status = final_resp.status();
                                            let final_status_u16 = final_status.as_u16();
                                            let final_headers = final_resp.headers().clone();
                                            let final_body_text = final_resp.text().await.unwrap_or_default();

                                            if final_status.is_success() {
                                                match serde_json::from_str::<T>(&final_body_text) {
                                                    Ok(parsed) => {
                                                        drop(permit);
                                                        return Ok(ApiResponse {
                                                            data: Some(parsed),
                                                            error_body: None,
                                                            status: final_status_u16,
                                                            success: true,
                                                            headers: final_headers,
                                                        });
                                                    }
                                                    Err(e) => {
                                                        drop(permit);
                                                        return Err(NgError::HttpError(format!("JSON decode: {}", e)));
                                                    }
                                                }
                                            } else {
                                                drop(permit);
                                                return Ok(ApiResponse {
                                                    data: None,
                                                    error_body: if final_body_text.is_empty() { None } else { Some(final_body_text) },
                                                    status: final_status_u16,
                                                    success: false,
                                                    headers: final_headers,
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            drop(permit);
                                            return Err(NgError::HttpError(e.to_string()));
                                        }
                                    },
                                    Err(e) => {
                                        drop(permit);
                                        return Err(NgError::InternalError(e.to_string()));
                                    }
                                }
                            }
                        }
                    }

                    let is_idempotent = self.opts.allowed_methods.contains(&method);
                    let allow_retries = is_idempotent;
                    let is_retryable_status = status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS;


                    if allow_retries && is_retryable_status {
                        // Prefer server Retry-After if present
                        if let Some(retry_after) = Self::parse_retry_after_from_headers(&resp_headers) {
                            let capped = if let Some(max) = self.opts.max_retry_after {
                                std::cmp::min(retry_after, max)
                            } else if let Some(limit) = self.opts.backoff_limit {
                                std::cmp::min(retry_after, limit)
                            } else {
                                retry_after
                            };

                            // If there are attempts remaining, sleep and continue normally.
                            if attempt < max_attempts {
                                self.smart_sleep_and_maybe_reacquire(capped, &mut permit).await;
                                continue;
                            }

                            // If this is the LAST attempt but server asked to wait, sleep and then
                            // perform one final request attempt after sleeping (instead of returning immediately).
                            self.smart_sleep_and_maybe_reacquire(capped, &mut permit).await;

                            // Build and execute a fresh final request attempt after sleeping.
                            let mut final_rb = self.client.request(method.clone(), url).headers(headers.clone());
                            if let Some(b) = body {
                                final_rb = final_rb.json(b);
                            }
                            let final_rb = self.prepare_request(final_rb);

                            match final_rb.build() {
                                Ok(req) => match self.client.execute(req).await {
                                    Ok(final_resp) => {
                                        let final_status = final_resp.status();
                                        let final_status_u16 = final_status.as_u16();
                                        let final_headers = final_resp.headers().clone();
                                        let final_body_text = final_resp.text().await.unwrap_or_default();

                                        if final_status.is_success() {
                                            match serde_json::from_str::<T>(&final_body_text) {
                                                Ok(parsed) => {
                                                    drop(permit);
                                                    return Ok(ApiResponse {
                                                        data: Some(parsed),
                                                        error_body: None,
                                                        status: final_status_u16,
                                                        success: true,
                                                        headers: final_headers,
                                                    });
                                                }
                                                Err(e) => {
                                                    drop(permit);
                                                    return Err(NgError::HttpError(format!("JSON decode: {}", e)));
                                                }
                                            }
                                        } else {
                                            drop(permit);
                                            return Ok(ApiResponse {
                                                data: None,
                                                error_body: if final_body_text.is_empty() { None } else { Some(final_body_text) },
                                                status: final_status_u16,
                                                success: false,
                                                headers: final_headers,
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        drop(permit);
                                        return Err(NgError::HttpError(e.to_string()));
                                    }
                                },
                                Err(e) => {
                                    drop(permit);
                                    return Err(NgError::InternalError(e.to_string()));
                                }
                            }
                        }

                        // Otherwise compute backoff with jitter (only if attempts remain)
                        if attempt < max_attempts {
                            let backoff = self.compute_backoff_with_jitter(attempt, &mut rng);
                            self.smart_sleep_and_maybe_reacquire(backoff, &mut permit).await;
                            continue;
                        }
                    }


                    // Not retryable or exhausted attempts: return ApiResponse with error body
                    drop(permit);
                    return Ok(ApiResponse {
                        data: None,
                        error_body: if body_text.is_empty() { None } else { Some(body_text) },
                        status: status_u16,
                        success: false,
                        headers: resp_headers,
                    });
                }
                Err(e) => {
                    // Network-level failure
                    crate::error!(self.logger, "Network failure", "url" => url, "error" => e.to_string());

                    if e.is_timeout() && !self.opts.retry_on_timeout {
                        drop(permit);
                        return Err(NgError::HttpError(e.to_string()));
                    }

                    last_err = Some(NgError::HttpError(e.to_string()));

                    // consult predicate if present
                    let should = if let Some(pred) = &self.opts.should_retry {
                        (pred)(None, last_err.as_ref().unwrap(), attempt)
                    } else {
                        true
                    };

                    if should && attempt < max_attempts {
                        let backoff = self.compute_backoff_with_jitter(attempt, &mut rng);
                        self.smart_sleep_and_maybe_reacquire(backoff, &mut permit).await;
                        continue;
                    } else {
                        drop(permit);
                        return Err(last_err.unwrap_or_else(|| NgError::InternalError("Network failure".into())));
                    }
                }
            }
        }

        // Exhausted attempts: return enriched error
        let attempts = max_attempts;
        let mut parts = Vec::new();
        if let Some(s) = last_status { parts.push(format!("status={}", s)); }
        if let Some(b) = last_body_snippet { parts.push(format!("body=\"{}\"", b.replace('"', "'"))); }
        parts.push(format!("attempts={}", attempts));
        if let Some(e) = last_err { parts.push(format!("last_err=\"{}\"", format!("{:?}", e).replace('"', "'"))); }
        Err(NgError::InternalError(parts.join(", ")))
    }

    /// Public GET convenience
    pub async fn get<T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<ApiResponse<T>, NgError> {
        self.request_with_retry(Method::GET, url, headers, Option::<&()>::None).await
    }

    /// /// put
    ///
    /// Public PUT request with JSON body and JSON response parsing.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL.
    /// * `headers` - Request headers.
    /// * `body` - Body to serialize as JSON.
    pub async fn put<T: DeserializeOwned + Send + 'static, B: Serialize + ?Sized>(
        &self,
        url: &str,
        headers: HeaderMap,
        body: &B,
    ) -> Result<ApiResponse<T>, NgError> {
        self.request_with_retry(Method::PUT, url, headers, Some(body)).await
    }

    /// /// post
    ///
    /// Public POST request with JSON body and JSON response parsing.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL.
    /// * `headers` - Request headers.
    /// * `body` - Body to serialize as JSON.
    pub async fn post<T: DeserializeOwned + Send + 'static, B: Serialize + ?Sized>(
        &self,
        url: &str,
        headers: HeaderMap,
        body: &B,
    ) -> Result<ApiResponse<T>, NgError> {
        self.request_with_retry(Method::POST, url, headers, Some(body)).await
    }

    /// /// patch
    ///
    /// Public PATCH request with JSON body and JSON response parsing.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL.
    /// * `headers` - Request headers.
    /// * `body` - Body to serialize as JSON.
    pub async fn patch<T: DeserializeOwned + Send + 'static, B: Serialize + ?Sized>(
        &self,
        url: &str,
        headers: HeaderMap,
        body: &B,
    ) -> Result<ApiResponse<T>, NgError> {
        self.request_with_retry(Method::PATCH, url, headers, Some(body)).await
    }

    /// /// delete
    ///
    /// Public DELETE request that parses JSON into `T`.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL.
    /// * `headers` - Request headers.
    pub async fn delete<T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<ApiResponse<T>, NgError> {
        self.request_with_retry(Method::DELETE, url, headers, Option::<&()>::None)
            .await
    }

    /// /// head
    ///
    /// Public HEAD request. Returns ApiResponse with no parsed body (data = None).
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL.
    /// * `headers` - Request headers.
    pub async fn head(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<ApiResponse<serde_json::Value>, NgError> {
        // HEAD typically has no body; reuse request_with_retry but ignore body parsing by using Value.
        self.request_with_retry(Method::HEAD, url, headers, Option::<&()>::None)
            .await
    }

    /// /// options
    ///
    /// Public OPTIONS request that parses JSON into `T`.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL.
    /// * `headers` - Request headers.
    pub async fn options<T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<ApiResponse<T>, NgError> {
        self.request_with_retry(Method::OPTIONS, url, headers, Option::<&()>::None)
            .await
    }

    /// /// trace
    ///
    /// Public TRACE request that parses JSON into `T`.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL.
    /// * `headers` - Request headers.
    pub async fn trace<T: DeserializeOwned + Send + 'static>(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<ApiResponse<T>, NgError> {
        self.request_with_retry(Method::TRACE, url, headers, Option::<&()>::None)
            .await
    }
}
