use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use crate::retrieve::ky_http::KyHttp;
use crate::core::error::NgError;
use crate::loggers::Logger;
use crate::error;
use tokio::time::{sleep, Duration};

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
            ("accept", "application/json, text/plain, */*"),
            ("accept-language", "en-US,en;q=0.9"),
            ("cache-control", "no-cache"),
            ("dnt", "1"),
            ("origin", "https://www.nasdaq.com"),
            ("pragma", "no-cache"),
            ("referer", "https://www.nasdaq.com/"),
            ("sec-ch-ua", r#""Google Chrome";v="135", "Not-A.Brand";v="8", "Chromium";v="135""#),
            ("sec-ch-ua-mobile", "?0"),
            ("sec-ch-ua-platform", "\"Windows\""),
            ("sec-fetch-dest", "empty"),
            ("sec-fetch-mode", "cors"),
            ("sec-fetch-site", "same-site"),
            ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36"),
        ];
        for (k, v) in headers {
            h.insert(k, HeaderValue::from_static(v));
        }
        h
    }

    pub async fn call(&self, endpoint: &str) -> Result<Value, NgError> {
        let url = format!("https://api.nasdaq.com/api/{}", endpoint.trim_start_matches('/'));
        let mut last_err = NgError::HttpError("Max retries reached".into());

        for attempt in 1..=3 {
            match self.http.get_json::<Value>(&url, self.get_nasdaq_headers()).await {
                Ok(resp) if resp.success => {
                    if let Some(data) = resp.data {
                        if data["status"]["rCode"] == 200 || data["status"]["rCode"] == "200" {
                            return Ok(data);
                        }
                    }
                }
                Ok(resp) => last_err = NgError::HttpError(format!("Status: {}, Body: {:?}", resp.status, resp.error_body)),
                Err(e) => last_err = e,
            }
            error!(self.logger, "Nasdaq API Retry", "attempt" => attempt, "url" => &url);
            sleep(Duration::from_millis(1000 * attempt)).await;
        }
        Err(last_err)
    }
}
