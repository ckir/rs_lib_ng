//! # Nasdaq Market Status Module
//!
//! Provides high-level methods to fetch market data and calculate operational
//! timings. This module is designed to be used by an orchestrator to manage
//! polling intervals and execution timing.

use chrono::{Duration as ChronoDuration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::US::Eastern;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::error::NgError;
use crate::{error, info};
use crate::loggers::Logger;
use crate::markets::nasdaq::apicallnasdaq::NasdaqApi;
use crate::retrieve::ky_http::KyOptions;

/// Represents the deserialized market information from Nasdaq.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatusData {
    pub country: String,
    pub market_indicator: String,
    pub ui_market_indicator: String,
    pub market_count_down: String,
    pub pre_market_opening_time: String,
    pub pre_market_closing_time: String,
    pub market_opening_time: String,
    pub market_closing_time: String,
    pub after_hours_market_opening_time: String,
    pub after_hours_market_closing_time: String,
    pub previous_trade_date: String,
    pub next_trade_date: String,
    pub is_business_day: bool,
    pub mrkt_status: String,
}

/// Service to fetch and analyze Nasdaq market status.
pub struct MarketStatus {
    api: NasdaqApi,
    logger: Logger,
}

impl MarketStatus {
    /// Creates a new instance of `MarketStatus`.
    pub fn new(logger: Logger) -> Self {
        Self {
            api: NasdaqApi::new(logger.clone()),
            logger,
        }
    }

    /// Fetches the raw JSON response from the Nasdaq market-info endpoint.
    pub async fn fetch_raw(&self, options: Option<KyOptions>) -> Result<Value, NgError> {
        let endpoint = "https://api.nasdaq.com/api/market-info/";
        self.api.call(endpoint, options).await
    }

    /// Fetches and deserializes the market status into typed data.
    pub async fn fetch_status(&self, options: Option<KyOptions>) -> Result<MarketStatusData, NgError> {
        let json = self.fetch_raw(options).await?;
        
        let data = json.get("data").ok_or_else(|| {
            NgError::MalformedResponse {
                endpoint: "market-info".to_string(),
                details: "Missing 'data' field".to_string(),
            }
        })?;

        serde_json::from_value(data.clone()).map_err(|e| {
            error!(self.logger, "Deserialization error in MarketStatus", "error" => e.to_string());
            NgError::MalformedResponse {
                endpoint: "market-info".to_string(),
                details: format!("JSON error: {}", e),
            }
        })
    }

    /// Determines if the market is currently in the Regular Trading Session.
    ///
    /// Checks if today is a business day and if the current Eastern Time 
    /// is between 09:30 AM and 04:00 PM.
    pub fn is_regular_session(&self, status: &MarketStatusData) -> bool {
        if !status.is_business_day {
            return false;
        }
        let now = Utc::now().with_timezone(&Eastern).time();
        let open = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        let close = NaiveTime::from_hms_opt(16, 0, 0).unwrap();

        now >= open && now < close
    }

    /// Calculates the precise duration until the next market opening.
    ///
    /// # Returns
    /// * `Ok(Duration)` representing the time until 09:30 AM ET on the next trade date.
    /// * `Err(NgError)` if the date string from Nasdaq cannot be parsed.
    pub fn get_next_opening_delay(&self, status: &MarketStatusData) -> Result<std::time::Duration, NgError> {
        let now = Utc::now().with_timezone(&Eastern);
        let fmt = "%b %d, %Y"; // e.g., "Feb 24, 2026"
        
        let d = NaiveDate::parse_from_str(&status.next_trade_date, fmt).map_err(|e| {
            NgError::MalformedResponse {
                endpoint: "market-info".to_string(),
                details: format!("Date parsing failed for '{}': {}", status.next_trade_date, e),
            }
        })?;

        let target_naive = d.and_hms_opt(9, 30, 0).unwrap();
        let target_dt = Eastern.from_local_datetime(&target_naive).single().ok_or_else(|| {
            NgError::InternalError("Ambiguous timezone conversion during market open calculation".into())
        })?;

        let diff = target_dt.signed_duration_since(now);
        let secs = diff.num_seconds();

        if secs > 0 {
            Ok(std::time::Duration::from_secs(secs as u64))
        } else {
            // If the time has already passed for the recorded "next_trade_date", 
            // we return a 0 duration so the caller knows to refresh data.
            Ok(std::time::Duration::from_secs(0))
        }
    }

    /// Blocks the current task until the market opens.
    ///
    /// Useful for transition alerts. If the market is already open or the 
    /// delay cannot be calculated, it returns immediately.
    pub async fn wait_until_open(&self, status: &MarketStatusData) {
        if let Ok(delay) = self.get_next_opening_delay(status) {
            if delay.as_secs() > 0 {
                info!(
                    self.logger, 
                    "Market transition alert: Waiting for opening", 
                    "wait_time" => self.format_duration(ChronoDuration::from_std(delay).unwrap_or(ChronoDuration::zero()))
                );
                tokio::time::sleep(delay).await;
                info!(self.logger, "Market opening time reached.");
            }
        }
    }

    /// Formats a Chrono Duration into a standard HH:MM:SS string.
    pub fn format_duration(&self, dur: ChronoDuration) -> String {
        let secs = dur.num_seconds().abs();
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}