use serde::{Deserialize, Serialize};
use chrono::{Utc, NaiveDate, Duration as ChronoDuration, TimeZone};
use chrono_tz::US::Eastern;
 use crate::markets::nasdaq::apicallnasdaq::NasdaqApi;
use crate::core::error::NgError;
use crate::loggers::Logger;

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

pub struct MarketStatus {
    api: NasdaqApi,
    _logger: Logger,
}

impl MarketStatus {
    pub fn new(_logger: Logger) -> Self {
        Self { 
            api: NasdaqApi::new(_logger.clone()),
            _logger 
        }
    }

    pub async fn fetch_raw(&self) -> Result<serde_json::Value, NgError> {
        self.api.call("market-info").await
    }

    pub async fn fetch_status(&self) -> Result<MarketStatusData, NgError> {
        let json = self.fetch_raw().await?;
        serde_json::from_value(json["data"].clone())
            .map_err(|e| NgError::HttpError(format!("Deserialization error: {}", e)))
    }

    pub fn get_next_opening_delay(&self, status: &MarketStatusData) -> std::time::Duration {
        let now = Utc::now().with_timezone(&Eastern);
        let fmt = "%b %d, %Y";
        if let Ok(d) = NaiveDate::parse_from_str(&status.next_trade_date, fmt) {
            if let Some(target) = d.and_hms_opt(9, 30, 0) {
                let target_dt = Eastern.from_local_datetime(&target).unwrap();
                let diff = target_dt.signed_duration_since(now);
                if diff.num_seconds() > 0 {
                    return std::time::Duration::from_secs(diff.num_seconds() as u64);
                }
            }
        }
        std::time::Duration::from_secs(300)
    }

    pub fn format_duration(dur: ChronoDuration) -> String {
        let secs = dur.num_seconds();
        format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}
