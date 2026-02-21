use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum NgError {
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}
