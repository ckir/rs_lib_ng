use serde::{Deserialize, Serialize};
use chrono::NaiveDate;
use crate::markets::cnn::apicallcnn::CnnApi; // Correctly import CnnApi
use crate::core::error::NgError;
use crate::loggers::Logger;
// use crate::error; // Commented out to fix unused import warning

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FngData {
    pub score: f64,
    pub rating: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FearAndGreedResponse {
    pub fear_and_greed: FngData,
}

pub struct FearAndGreed {
    api: CnnApi,
    #[allow(dead_code)]
    logger: Logger,
}

impl FearAndGreed {
    pub fn new(logger: Logger) -> Self {
        Self {
            api: CnnApi::new(logger.clone()),
            logger,
        }
    }

    pub async fn fetch_current(&self) -> Result<FngData, NgError> {
        let json = self.api.call("graphdata").await?;
        let resp: FearAndGreedResponse = serde_json::from_value(json)
            .map_err(|e| NgError::InternalError(format!("JSON Decode: {}", e)))?;
        Ok(resp.fear_and_greed)
    }

    pub async fn fetch_at_date(&self, date: NaiveDate) -> Result<FngData, NgError> {
        let endpoint = format!("graphdata/{}", date.format("%Y-%m-%d"));
        let json = self.api.call(&endpoint).await?;
        let resp: FearAndGreedResponse = serde_json::from_value(json)
            .map_err(|e| NgError::InternalError(format!("JSON Decode: {}", e)))?;
        Ok(resp.fear_and_greed)
    }
}
