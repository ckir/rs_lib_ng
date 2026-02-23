//! # CNN API Mock Test Suite
//!
//! Validates the behavior of the CnnApi adapter using a local mock server.
//! Tests include successful JSON parsing, header verification, and error handling.

use wiremock::matchers::{method, path, header};
use wiremock::{Mock, MockServer, ResponseTemplate};
use serde_json::json;
use std::time::Duration; // Required for timeout configuration
use rs_lib_ng::markets::cnn::apicallcnn::CnnApi;
use rs_lib_ng::loggers::builder::LoggerBuilder;
use rs_lib_ng::retrieve::ky_http::KyOptions;
use rs_lib_ng::core::error::NgError;

/// Helper to initialize the CnnApi service and a mock server.
///
/// # Returns
/// A tuple containing the initialized [`CnnApi`] and the [`MockServer`].
async fn setup_cnn_test() -> (CnnApi, MockServer) {
    let server = MockServer::start().await;
    let logger = LoggerBuilder::new("cnn_test")
        .build()
        .expect("Failed to build test logger");
    let service = CnnApi::new(logger);
    (service, server)
}

#[tokio::test]
async fn test_cnn_call_success() {
    //! Scenario: CNN API returns a valid JSON response.
    //! Goal: Ensure the adapter returns the expected Value.
    let (service, server) = setup_cnn_test().await;

    let response_body = json!({
        "fear_and_greed": {
            "score": 75.0,
            "rating": "greed"
        }
    });

    Mock::given(method("GET"))
        .and(path("/data/v1"))
        // Verify that our default headers (shared with Nasdaq adapter) are sent
        .and(header("authority", "api.nasdaq.com")) 
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
        .mount(&server)
        .await;

    let url = format!("{}/data/v1", server.uri());
    let result = service.call(&url, None).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap()["fear_and_greed"]["rating"], "greed");
}

#[tokio::test]
async fn test_cnn_custom_header_injection() {
    //! Scenario: Setting a custom header like an API Key.
    //! Goal: Verify the header is correctly sent to the endpoint.
    let (mut service, server) = setup_cnn_test().await;

    // Use the dynamic set_header method fixed in the previous iteration
    service.set_header("x-cnn-token", "secret123");

    Mock::given(method("GET"))
        .and(header("x-cnn-token", "secret123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
        .mount(&server)
        .await;

    let result = service.call(&server.uri(), None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cnn_options_override() {
    //! Scenario: Passing KyOptions to override behavior.
    //! Goal: Verify that transient options are respected (e.g., lower timeout).
    let (service, server) = setup_cnn_test().await;

    let mut opts = KyOptions::default();
    // Fixed: timeout is Option<Duration>, not a raw integer
    opts.timeout = Some(Duration::from_millis(100)); 

    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({}))
                // Delay longer than the timeout to trigger the error
                .set_delay(Duration::from_millis(300))
        )
        .mount(&server)
        .await;

    let result = service.call(&server.uri(), Some(opts)).await;
    
    // Expect an error due to the transient 100ms timeout
    assert!(result.is_err());
}

#[tokio::test]
async fn test_cnn_non_json_error() {
    //! Scenario: Server returns HTML instead of JSON.
    //! Goal: Ensure NgError::NonJsonResponse is returned with a body snippet.
    let (service, server) = setup_cnn_test().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403).set_body_string("<html>Forbidden</html>"))
        .mount(&server)
        .await;

    let result = service.call(&server.uri(), None).await;

    match result {
        Err(NgError::NonJsonResponse { status, body_snippet, .. }) => {
            assert_eq!(status, 403);
            assert!(body_snippet.contains("<html>"));
        },
        _ => panic!("Expected NonJsonResponse, got {:?}", result),
    }
}