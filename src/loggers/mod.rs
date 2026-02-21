
pub mod builder;
pub mod core;
pub mod worker;
pub mod transports;

pub use builder::{Logger, LoggerBuilder};
pub use core::LogLevel;

#[macro_export]
macro_rules! log_base {
    ($logger:expr, $level:expr, $msg:expr, $($k:expr => $v:expr),*) => {
        {
            let mut ctx = std::collections::HashMap::new();
            $(
                ctx.insert($k.to_string(), serde_json::to_value($v).unwrap_or(serde_json::Value::Null));
            )*
            
            let record = $crate::loggers::core::LogRecord {
                ts: chrono::Utc::now(),
                level: $level,
                msg: $msg.to_string(),
                component: $logger.config.load().component.clone(),
                ctx,
                sys: None,
            };
            
            let _ = $logger.sender.try_send(record);
        }
    };
}

#[macro_export]
macro_rules! info {
    ($logger:expr, $msg:expr, $($k:expr => $v:expr),*) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Info, $msg, $($k => $v),*)
    };
}

#[macro_export]
macro_rules! error {
    ($logger:expr, $msg:expr, $($k:expr => $v:expr),*) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Error, $msg, $($k => $v),*)
    };
}

#[macro_export]
macro_rules! fatal {
    ($logger:expr, $msg:expr, $($k:expr => $v:expr),*) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Fatal, $msg, $($k => $v),*)
    };
}
