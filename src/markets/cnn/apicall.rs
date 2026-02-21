use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use crate::retrieve::ky_http::KyHttp;
use crate::loggers::Logger;
use crate::core::error::NgError;
use crate::error;

pub struct ApiCall {
    http: KyHttp,
    logger: Logger,
}

impl ApiCall {
    pub fn new(logger: Logger) -> Self {
        Self {
            http: KyHttp::new(logger.clone()),
            logger,
        }
    }

    fn get_cnn_headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        let headers = [
            ("authority", "production.dataviz.cnn.io"),
            ("accept", "*/*"),
            ("accept-language", "en-US,en;q=0.9,el-GR;q=0.8,el;q=0.7,it;q=0.6"),
            ("cache-control", "no-cache"),
            ("dnt", "1"),
            ("origin", "https://edition.cnn.com"),
            ("pragma", "no-cache"),
            ("referer", "https://edition.cnn.com/"),
            ("sec-ch-ua", r#""Not_A Brand";v="8", "Chromium";v="120", "Google Chrome";v="120""#),
            ("sec-ch-ua-mobile", "?0"),
            ("sec-ch-ua-platform", "\"Windows\""),
            ("sec-fetch-dest", "empty"),
            ("sec-fetch-mode", "cors"),
            ("sec-fetch-site", "cross-site"),
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"),
        ];
        for (k, v) in headers {
            h.insert(k, HeaderValue::from_static(v));
        }
        h
    }

    pub async fn call(&self, endpoint: &str) -> Result<Value, NgError> {
        let url = format!("https://production.dataviz.cnn.io/index/fearandgreed/{}", endpoint.trim_start_matches('/'));
        
        match self.http.get_json::<Value>(&url, self.get_cnn_headers()).await {
            Ok(resp) if resp.success => Ok(resp.data.unwrap_or_default()),
            Ok(resp) => {
                let err_msg = format!("CNN API Failure: Status {}", resp.status);
                // Correctly use the error! macro defined in src/loggers/mod.rs [cite: 44]
                error!(self.logger, "CNN API Error", "status" => resp.status);
                Err(NgError::HttpError(err_msg))
            }
            Err(e) => {
                error!(self.logger, "CNN Connection Error", "error" => e.to_string());
                Err(e)
            }
        }
    }
}
