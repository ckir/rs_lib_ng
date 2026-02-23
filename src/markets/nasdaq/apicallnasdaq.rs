use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use crate::retrieve::ky_http::KyHttp;
use crate::core::error::NgError;
use crate::loggers::Logger;
use crate::error;

pub struct NasdaqApi {
    http: KyHttp,
    logger: Logger,
}

impl NasdaqApi {
    pub fn new(logger: Logger) -> Self {
        Self {
            http: KyHttp::new(logger.clone()),
            logger,
        }
    }

    fn get_nasdaq_headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        let headers = [
            ("authority", "api.nasdaq.com"),
            ("accept", "application/json, text/plain, */*"),
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
        ];
        for (k, v) in headers {
            h.insert(k, HeaderValue::from_static(v));
        }
        h
    }

    pub async fn call(&self, endpoint: &str) -> Result<Value, NgError> {
        let url = format!("https://api.nasdaq.com/api/market-info/{}", endpoint);
        match self.http.get::<Value>(&url, self.get_nasdaq_headers()).await {
            Ok(resp) if resp.success => Ok(resp.data.unwrap_or_default()),
            Ok(resp) => {
                error!(self.logger, "Nasdaq API failure", "status" => resp.status);
                Err(NgError::HttpError(format!("Status: {}", resp.status)))
            }
            Err(e) => Err(e),
        }
    }
}
