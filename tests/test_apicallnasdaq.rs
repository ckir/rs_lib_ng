//! # Nasdaq API Exhaustive Test Suite
//!
//! This module provides comprehensive integration tests for the `NasdaqApi` struct.
//! It simulates various real-world scenarios including network errors, malformed 
//! responses, and Nasdaq-specific business logic failures using WireMock.

use wiremock::matchers::{method, header};
use wiremock::{Mock, MockServer, ResponseTemplate};
use serde_json::json;
use rs_lib_ng::markets::nasdaq::apicallnasdaq::NasdaqApi;
use rs_lib_ng::retrieve::ky_http::KyOptions;
use rs_lib_ng::loggers::builder::LoggerBuilder;
use rs_lib_ng::core::error::NgError;

/// Helper function to initialize a logger and the NasdaqApi instance.
///
/// This setup function creates a transient WireMock server and a standard
/// logger instance configured for the testing environment.
async fn setup_api() -> (NasdaqApi, MockServer) {
    // Initialize the Mock Server to intercept HTTP calls
    let server = MockServer::start().await;
    // Build a standard logger for telemetry - requires component name and unwrap
    let logger = LoggerBuilder::new("nasdaq_test")
        .build()
        .expect("Failed to initialize test logger");
    // Instantiate the API adapter
    let api = NasdaqApi::new(logger);
    (api, server)
}

#[tokio::test]
async fn test_successful_call_200_rcode() {
    //! Verifies that a standard successful Nasdaq response is parsed correctly.
    let (api, server) = setup_api().await;

    let response_body = json!({
        "data": { "symbol": "AAPL", "price": 150.0 },
        "status": { "rCode": 200, "bCodeMessage": null }
    });

    Mock::given(method("GET"))
        .and(header("authority", "api.nasdaq.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&server)
        .await;

    // Execute the call
    let result = api.call(&server.uri(), None).await;

    // Assertions
    assert!(result.is_ok());
    let data = result.unwrap();
    assert_eq!(data["data"]["symbol"], "AAPL");
}

#[tokio::test]
async fn test_business_error_non_200_rcode() {
    //! Tests the requirement that rCode != 200 results in a hard NgError::NasdaqBusinessError.
    let (api, server) = setup_api().await;

    let error_body = json!({
        "data": null,
        "status": { "rCode": 400, "bCodeMessage": "Invalid Symbol" }
    });

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(error_body))
        .mount(&server)
        .await;

    // Execute the call
    let result = api.call(&server.uri(), None).await;

    // Assertions
    match result {
        Err(NgError::NasdaqBusinessError { r_code, .. }) => assert_eq!(r_code, 400),
        _ => panic!("Expected NasdaqBusinessError, got {:?}", result),
    }
}

#[tokio::test]
async fn test_maintenance_mode_non_json_response() {
    //! Verifies handling of non-JSON content (e.g., HTML maintenance pages).
    let (api, server) = setup_api().await;

    let html_content = "<html><body>503 Service Unavailable (Maintenance)</body></html>";

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_string(html_content))
        .mount(&server)
        .await;

    // Execute the call
    let result = api.call(&server.uri(), None).await;

    // Assertions
    match result {
        Err(NgError::NonJsonResponse { status, body_snippet, .. }) => {
            assert_eq!(status, 503);
            assert!(body_snippet.contains("Maintenance"));
        },
        _ => panic!("Expected NonJsonResponse, got {:?}", result),
    }
}

#[tokio::test]
async fn test_malformed_json_missing_status() {
    //! Checks robustness against valid JSON that misses the mandatory Nasdaq status block.
    let (api, server) = setup_api().await;

    let malformed_body = json!({ "unexpected": "structure" });

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(malformed_body))
        .mount(&server)
        .await;

    // Execute the call
    let result = api.call(&server.uri(), None).await;

    // Assertions
    match result {
        Err(NgError::MalformedResponse { .. }) => (),
        _ => panic!("Expected MalformedResponse, got {:?}", result),
    }
}

#[tokio::test]
async fn test_custom_options_override() {
    //! Verifies that providing KyOptions triggers the transient instance logic correctly.
    let (api, server) = setup_api().await;

    // Configure options with 0 retries to fail fast - field name is 'retry'
    let mut opts = KyOptions::default();
    opts.retry = 0;

    // Server will return an error
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    // Execute call with custom options
    let result = api.call(&server.uri(), Some(opts)).await;

    // Assertions: Should fail immediately due to 0 retries
    assert!(result.is_err());
}

#[tokio::test]
async fn test_invalid_url_parameter() {
    //! Verifies that the internal URL validation prevents invalid parameters from proceeding.
    let logger = LoggerBuilder::new("invalid_url_test")
        .build()
        .expect("Failed to initialize logger");
    let api = NasdaqApi::new(logger);

    // Provide a malformed URL
    let result = api.call("not_a_url", None).await;

    // Assertions
    match result {
        Err(NgError::HttpError(msg)) => assert!(msg.contains("Invalid URL")),
        _ => panic!("Expected HttpError, got {:?}", result),
    }
}