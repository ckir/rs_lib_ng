//! tests/test_ky_http.rs
//!
//! Comprehensive integration test suite for the KyHttp module.
//!
//! RustDock: This file contains integration tests for the KyHttp helper, covering:
//! - Success and failure scenarios for all supported HTTP methods.
//! - Default and custom configuration behavior.
//! - Concurrency limiting via semaphores.
//! - Exponential backoff and Retry-After header logic.

use reqwest::header::{HeaderMap, USER_AGENT};
use rs_lib_ng::loggers::{Logger, LoggerBuilder};
use rs_lib_ng::retrieve::ky_http::{KyHttp, KyOptions};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// /// TestData
/// 
/// A simple serializable struct used to verify JSON request and response bodies.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct TestData {
    /// A generic message string.
    pub message: String,
}

/// /// get_test_logger
/// 
/// Helper function to initialize a standard logger for integration tests.
fn get_test_logger() -> Logger {
    // Build a logger with a specific name for test identification.
    LoggerBuilder::new("test-ky-http").build().unwrap()
}

// =========================================================================
// DEFAULT PARAMETER TESTS
// =========================================================================

/// /// test_default_get_success
/// 
/// Verifies that KyHttp can perform a successful GET request using default options.
#[tokio::test]
async fn test_default_get_success() {
    // Start a local mock server.
    let mock_server = MockServer::start().await;
    // Instantiate KyHttp with default options.
    let client = KyHttp::new(get_test_logger());

    let body = TestData { message: "success".into() };

    // Set up the mock expectation.
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&mock_server)
        .await;

    // Perform the request.
    let res = client
        .get::<TestData>(&format!("{}/ok", mock_server.uri()), HeaderMap::new())
        .await
        .expect("Request should not fail"); // Use expect to provide debug info if it fails

    assert!(res.success);
    assert_eq!(res.status, 200);
    assert_eq!(res.data.unwrap(), body);
}

/// /// test_default_retry_exhaustion
/// 
/// Verifies that the default retry limit (2 retries, 3 total attempts) is respected.
#[tokio::test]
async fn test_default_retry_exhaustion() {
    let mock_server = MockServer::start().await;
    let client = KyHttp::new(get_test_logger());

    // Respond with 500 Internal Server Error.
    // Added set_body_json to prevent decoding errors during retries.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500).set_body_json(&serde_json::json!({})))
        .expect(3) // 1 initial + 2 retries.
        .mount(&mock_server)
        .await;

    let res = client
        .get::<serde_json::Value>(&mock_server.uri(), HeaderMap::new())
        .await
        .unwrap();

    // The result should indicate failure but return the status.
    assert!(!res.success);
    assert_eq!(res.status, 500);
}

// =========================================================================
// CUSTOM PARAMETER TESTS
// =========================================================================

/// /// test_custom_retry_and_backoff
/// 
/// Verifies that custom retry counts and backoff limits are correctly applied.
#[tokio::test]
async fn test_custom_retry_and_backoff() {
    let mock_server = MockServer::start().await;
    
    // Configure 4 retries (5 total attempts) and a very short backoff limit.
    let mut opts = KyOptions::default();
    opts.retry = 4;
    opts.backoff_limit = Some(Duration::from_millis(10));
    
    let client = KyHttp::new_with_opts(get_test_logger(), Some(opts));

    // Added set_body_json so the client doesn't panic on empty body decoding.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_json(&serde_json::json!({})))
        .expect(5) 
        .mount(&mock_server)
        .await;

    let _ = client.get::<serde_json::Value>(&mock_server.uri(), HeaderMap::new()).await;
}

/// /// test_retry_after_numeric
/// 
/// Verifies that the client respects the 'Retry-After' header with numeric seconds.
#[tokio::test]
async fn test_retry_after_numeric() {
    // 1. Correct Logger initialization
    let logger = LoggerBuilder::new("test-ky-http").build().unwrap();
    let mock_server = MockServer::start().await;

    // 2. Setup the "Failure" Mock (Rate Limited)
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "1")
        )
        .up_to_n_times(2) // Allow 2 failures
        .mount(&mock_server)
        .await;

    // 3. Setup the "Success" Mock (Must match TestData contract)
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "message": "eventual success" // Field must match TestData struct
                }))
        )
        .mount(&mock_server)
        .await;

    let options = KyOptions {
        retry: 3, 
        ..KyOptions::default()
    };

    // 4. Use new_with_opts to pass custom options
    let client = KyHttp::new_with_opts(logger, Some(options));

    // 5. Execute and Assert
    let res = client
        .get::<TestData>(&mock_server.uri(), HeaderMap::new())
        .await
        .expect("Request should eventually succeed after retries");

    assert!(res.success);
    assert_eq!(res.status, 200);
    assert_eq!(res.data.unwrap().message, "eventual success");
}

/// /// test_concurrency_limiting
/// 
/// Verifies that the internal semaphore restricts concurrent logical requests.
#[tokio::test]
async fn test_concurrency_limiting() {
    let mock_server = MockServer::start().await;
    
    let mut opts = KyOptions::default();
    opts.limit = 1; // Allow only one request at a time.
    
    let client = KyHttp::new_with_opts(get_test_logger(), Some(opts));

    // Each request will take 200ms. 
    // Added set_body_json to satisfy JSON decoding requirement.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200)
            .set_delay(Duration::from_millis(200))
            .set_body_json(&serde_json::json!({})))
        .mount(&mock_server)
        .await;

    let server_uri = mock_server.uri();
    let start = std::time::Instant::now();
    
    // Execute two requests concurrently via join.
    let req1 = client.get::<serde_json::Value>(&server_uri, HeaderMap::new());
    let req2 = client.get::<serde_json::Value>(&server_uri, HeaderMap::new());

    let (res1, res2) = tokio::join!(req1, req2);

    assert!(res1.is_ok());
    assert!(res2.is_ok());
    // Total time must be at least 400ms because they were serialized by the semaphore.
    assert!(start.elapsed() >= Duration::from_millis(400));
}

/// /// test_method_filtering
/// 
/// Verifies that the client blocks methods not present in the allowed_methods set.
#[tokio::test]
async fn test_method_filtering() {
    let mock_server = MockServer::start().await;
    
    let mut opts = KyOptions::default();
    // Remove POST from allowed methods.
    opts.allowed_methods.remove(&reqwest::Method::POST);
    
    let client = KyHttp::new_with_opts(get_test_logger(), Some(opts));

    let res = client
        .post::<serde_json::Value, _>(&mock_server.uri(), HeaderMap::new(), &serde_json::json!({}))
        .await;

    // Check for the specific internal error.
    match res {
        Err(rs_lib_ng::core::error::NgError::InternalError(msg)) => {
            assert!(msg.contains("Method POST not allowed"));
        },
        _ => panic!("Expected InternalError for restricted method"),
    }
}

/// /// test_post_with_body
/// 
/// Verifies that POST requests correctly transmit JSON bodies.
#[tokio::test]
async fn test_post_with_body() {
    let mock_server = MockServer::start().await;
    let client = KyHttp::new(get_test_logger());
    let payload = TestData { message: "payload_content".into() };

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(201).set_body_json(&payload))
        .mount(&mock_server)
        .await;

    let res = client
        .post::<TestData, _>(&mock_server.uri(), HeaderMap::new(), &payload)
        .await
        .expect("Post failed");

    assert_eq!(res.status, 201);
    assert_eq!(res.data.unwrap(), payload);
}

/// /// test_custom_headers_transmission
/// 
/// Verifies that user-provided headers are correctly attached to the request.
#[tokio::test]
async fn test_custom_headers_transmission() {
    let mock_server = MockServer::start().await;
    let client = KyHttp::new(get_test_logger());

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, "KyHttpTestAgent/1.0".parse().unwrap());

    // Added set_body_json to prevent decoding errors during test.
    Mock::given(method("GET"))
        .and(header("user-agent", "KyHttpTestAgent/1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&serde_json::json!({})))
        .mount(&mock_server)
        .await;

    let res = client
        .get::<serde_json::Value>(&mock_server.uri(), headers)
        .await
        .unwrap();

    assert_eq!(res.status, 200);
}