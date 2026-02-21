use tokio::sync::mpsc;
use crate::loggers::worker::LogWorker;
use crate::loggers::core::{LogLevel, LogRecord};
use std::sync::Arc;
use arc_swap::ArcSwap;

pub struct LoggerConfig {
    pub level: LogLevel,
    pub component: String,
}

#[derive(Clone)]
pub struct Logger {
    pub sender: mpsc::Sender<LogRecord>,
    pub config: Arc<ArcSwap<LoggerConfig>>,
}

pub struct LoggerBuilder {
    component: String,
    level: LogLevel,
    buffer_size: usize,
}

impl LoggerBuilder {
    pub fn new(component: &str) -> Self {
        Self {
            component: component.to_string(),
            level: LogLevel::Info,
            buffer_size: 1024,
        }
    }

    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    pub fn build(self) -> Result<Logger, crate::core::error::NgError> {
        let (tx, rx) = mpsc::channel(self.buffer_size);
        let config = Arc::new(ArcSwap::from_pointee(LoggerConfig {
            level: self.level,
            component: self.component,
        }));

        let worker = LogWorker::new(rx);
        tokio::spawn(async move {
            worker.run().await;
        });

        Ok(Logger { sender: tx, config })
    }
}
