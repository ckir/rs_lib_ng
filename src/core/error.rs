//! # Core Error Module
//!
//! This module defines the central `NgError` type used throughout the library.
//! It leverages `thiserror` for error message formatting and `serde` for serialization.

use serde::Serialize;
use thiserror::Error;

/// Central error type for the `rs_lib_ng` library.
#[derive(Debug, Error, Serialize)]
pub enum NgError {
    /// Error related to configuration loading or merging.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Error related to internal logic or state.
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Standard HTTP request or network failure.
    #[error("HTTP error: {0}")]
    HttpError(String),

    /// Error returned when the Nasdaq API response is not valid JSON.
    /// This often occurs when the service is behind a maintenance page or proxy.
    #[error("Nasdaq API returned non-JSON content from {url}. Status: {status}")]
    NonJsonResponse {
        /// The target URL that was requested.
        url: String,
        /// The HTTP status code received.
        status: u16,
        /// A snippet of the response body for diagnostic purposes.
        body_snippet: String,
    },

    /// Error returned when the Nasdaq API returns a successful HTTP status but a 
    /// business-level failure (e.g., rCode is not 200).
    #[error("Nasdaq API business error (rCode {r_code}) at {endpoint}")]
    NasdaqBusinessError {
        /// The rCode returned in the JSON status block.
        r_code: i64,
        /// The endpoint URL that was called.
        endpoint: String,
        /// The full JSON response body for deeper inspection.
        response: serde_json::Value,
    },

    /// Error returned when the JSON structure is missing expected mandatory fields.
    #[error("Malformed Nasdaq API response structure at {endpoint}: {details}")]
    MalformedResponse {
        /// The endpoint URL that was called.
        endpoint: String,
        /// Description of why the structure was considered malformed.
        details: String,
    },
}