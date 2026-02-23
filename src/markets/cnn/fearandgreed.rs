//! # CNN Fear & Greed Index Service
//!
//! Provides a high-level interface for retrieving the CNN Fear & Greed Index.
//! It supports fetching current status and historical graph data, transforming 
//! raw API responses into structured domain models.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use chrono::{DateTime, Utc, TimeZone};
use crate::markets::cnn::apicallcnn::CnnApi;
use crate::retrieve::ky_http::KyOptions;
use crate::core::error::NgError;
use crate::loggers::Logger;

/// Represents a single measurement of the Fear & Greed index or one of its components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FngData {
    /// The specific date and time the reading was recorded.
    pub date: DateTime<Utc>,
    /// The numerical value of the index (typically 0.0 to 100.0).
    pub value: f64,
    /// The market sentiment rating associated with the value.
    pub rating: String,
}

/// Comprehensive status of the Fear & Greed index including sub-indicators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FearAndGreedStatus {
    /// The primary Fear & Greed index reading.
    pub current: FngData,
    /// Historical readings extracted from the index graph.
    pub history: Vec<FngData>,
    /// Market Momentum (S&P 500 vs 125-day moving average).
    pub market_momentum: FngData,
    /// Stock Price Strength (Net new highs vs lows).
    pub stock_price_strength: FngData,
    /// Stock Price Breadth (McClellan Summation Index).
    pub stock_price_breadth: FngData,
    /// Put and Call Options (Put/call ratio).
    pub put_call_options: FngData,
    /// Previous market close index value.
    pub previous_close: f64,
    /// Average index value from one week ago.
    pub previous_1_week: f64,
}

/// Service orchestrator for CNN Fear & Greed data retrieval.
pub struct FearAndGreed {
    /// Internal API client for CNN endpoints.
    api: CnnApi,
    /// Shared logger for diagnostic tracking.
    logger: Logger,
}

impl FearAndGreed {
    /// Creates a new instance of the `FearAndGreed` service.
    ///
    /// # Arguments
    /// * `logger` - A [`Logger`] handle used for internal telemetry.
    pub fn new(logger: Logger) -> Self {
        Self {
            api: CnnApi::new(logger.clone()),
            logger,
        }
    }

    /// Fetches the latest Fear & Greed index and sub-indicators.
    ///
    /// This method uses the base `graphdata` endpoint which contains 
    /// both current status and a 125-day historical window.
    ///
    /// # Arguments
    /// * `options` - Optional [`KyOptions`] for overriding request behavior.
    pub async fn fetch_latest(&self, options: Option<KyOptions>) -> Result<FearAndGreedStatus, NgError> {
        let url = "https://production.dataviz.cnn.io/index/fearandgreed/graphdata";
        let raw = self.api.call(url, options).await?;
        self.map_response(raw, url)
    }

    /// Fetches historical Fear & Greed data for a specific date.
    ///
    /// # Arguments
    /// * `date` - The target date in `%Y-%m-%d` format.
    /// * `options` - Optional [`KyOptions`] for request configuration.
    pub async fn fetch_at_date(&self, date: &str, options: Option<KyOptions>) -> Result<FearAndGreedStatus, NgError> {
        let url = format!("https://production.dataviz.cnn.io/index/fearandgreed/graphdata/{}", date);
        let raw = self.api.call(&url, options).await?;
        self.map_response(raw, &url)
    }

    /// Maps raw JSON response into a typed [`FearAndGreedStatus`].
    ///
    /// This handles the transformation of CNN's `x` (milliseconds) and `y` (value) 
    /// fields into standard date/value pairs.
    fn map_response(&self, json: Value, url: &str) -> Result<FearAndGreedStatus, NgError> {
        // Helper to extract nested FngData blocks from the various indicator keys
        let extract_indicator = |key: &str| -> FngData {
            let block = &json[key];
            FngData {
                date: block["timestamp"].as_f64()
                    .and_then(|ts| Utc.timestamp_millis_opt(ts as i64).single())
                    .unwrap_or_else(Utc::now),
                value: block["score"].as_f64().unwrap_or(0.0),
                rating: block["rating"].as_str().unwrap_or("unknown").to_string(),
            }
        };

        // Validate the presence of the primary data block
        let fg_primary = json.get("fear_and_greed").ok_or_else(|| NgError::MalformedResponse {
            endpoint: url.to_string(),
            details: "Missing 'fear_and_greed' root key".to_string(),
        })?;

        // Construct current primary reading
        let current = FngData {
            date: fg_primary["timestamp"].as_str()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            value: fg_primary["score"].as_f64().unwrap_or(0.0),
            rating: fg_primary["rating"].as_str().unwrap_or("unknown").to_string(),
        };

        // Map historical time-series (transforming x and y)
        let mut history = Vec::new();
        if let Some(data_points) = json["fear_and_greed_historical"]["data"].as_array() {
            for point in data_points {
                if let (Some(x), Some(y)) = (point["x"].as_f64(), point["y"].as_f64()) {
                    history.push(FngData {
                        date: Utc.timestamp_millis_opt(x as i64).unwrap(),
                        value: y,
                        rating: point["rating"].as_str().unwrap_or("").to_string(),
                    });
                }
            }
        }

        Ok(FearAndGreedStatus {
            current,
            history,
            market_momentum: extract_indicator("market_momentum_sp500"),
            stock_price_strength: extract_indicator("stock_price_strength"),
            stock_price_breadth: extract_indicator("stock_price_breadth"),
            put_call_options: extract_indicator("put_call_options"),
            previous_close: fg_primary["previous_close"].as_f64().unwrap_or(0.0),
            previous_1_week: fg_primary["previous_1_week"].as_f64().unwrap_or(0.0),
        })
    }
}