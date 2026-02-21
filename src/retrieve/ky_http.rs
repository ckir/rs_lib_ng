use reqwest::{header::HeaderMap, Client};
use serde::de::DeserializeOwned;
use crate::loggers::Logger;
use crate::error;
use crate::core::error::NgError;

#[derive(Debug)]
pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub error_body: Option<String>,
    pub status: u16,
    pub success: bool,
    pub headers: HeaderMap,
}

pub struct KyHttp {
    client: Client,
    logger: Logger,
}

impl KyHttp {
    pub fn new(logger: Logger) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            logger,
        }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str, headers: HeaderMap) -> Result<ApiResponse<T>, NgError> {
        match self.client.get(url).headers(headers).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let success = resp.status().is_success();
                let resp_headers = resp.headers().clone();

                if success {
                    let data = resp.json::<T>().await.map_err(|e| NgError::HttpError(e.to_string()))?;
                    Ok(ApiResponse { data: Some(data), error_body: None, status, success, headers: resp_headers })
                } else {
                    let error_text = resp.text().await.ok();
                    error!(self.logger, "HTTP Error", "url" => url, "status" => status);
                    Ok(ApiResponse { data: None, error_body: error_text, status, success, headers: resp_headers })
                }
            }
            Err(e) => {
                error!(self.logger, "Network Failure", "url" => url, "error" => e.to_string());
                Err(NgError::HttpError(e.to_string()))
            }
        }
    }
}
