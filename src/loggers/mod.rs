// src/loggers/mod.rs

pub mod builder;
pub mod core;
pub mod worker;
pub mod transports;

pub use builder::{Logger, LoggerBuilder};
pub use core::LogLevel;

#[macro_export]
macro_rules! log_base {
    // No kv pairs
    ($logger:expr, $level:expr, $msg:expr) => {
        $crate::log_base!($logger, $level, $msg, );
    };
    // With kv pairs (zero or more)
    ($logger:expr, $level:expr, $msg:expr, $( $k:expr => $v:expr ),* $(,)? ) => {
        {
            // Level filtering: only send if record level >= configured level
            let cfg = $logger.config.load();
            if $level < cfg.level {
                // do nothing if below configured level
            } else {
                let mut ctx = std::collections::HashMap::new();
                $(
                    // Convert value to serde_json::Value; fallback to Null on error
                    ctx.insert($k.to_string(), serde_json::to_value($v).unwrap_or(serde_json::Value::Null));
                )*

                let record = $crate::loggers::core::LogRecord {
                    ts: chrono::Utc::now(),
                    level: $level,
                    msg: $msg.to_string(),
                    component: cfg.component.clone(),
                    ctx,
                    sys: None,
                };

                // best-effort send; ignore send errors
                let _ = $logger.sender.try_send(record);
            }
        }
    };
}

#[macro_export]
macro_rules! trace {
    ($logger:expr, $msg:expr $(, $k:expr => $v:expr )* $(,)? ) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Trace, $msg $(, $k => $v )* )
    };
}

#[macro_export]
macro_rules! debug {
    ($logger:expr, $msg:expr $(, $k:expr => $v:expr )* $(,)? ) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Debug, $msg $(, $k => $v )* )
    };
}

#[macro_export]
macro_rules! info {
    ($logger:expr, $msg:expr $(, $k:expr => $v:expr )* $(,)? ) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Info, $msg $(, $k => $v )* )
    };
}

#[macro_export]
macro_rules! warn {
    ($logger:expr, $msg:expr $(, $k:expr => $v:expr )* $(,)? ) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Warn, $msg $(, $k => $v )* )
    };
}

#[macro_export]
macro_rules! error {
    ($logger:expr, $msg:expr $(, $k:expr => $v:expr )* $(,)? ) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Error, $msg $(, $k => $v )* )
    };
}

#[macro_export]
macro_rules! fatal {
    ($logger:expr, $msg:expr $(, $k:expr => $v:expr )* $(,)? ) => {
        $crate::log_base!($logger, $crate::loggers::core::LogLevel::Fatal, $msg $(, $k => $v )* )
    };
}
