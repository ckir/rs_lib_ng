// tests/test_logger.rs
use rs_lib_ng::loggers::core::{LogLevel, LogRecord};
use rs_lib_ng::loggers::Logger;
use rs_lib_ng::loggers::builder::LoggerConfig;
use arc_swap::ArcSwap;
use std::sync::Arc;
use tokio::sync::mpsc;
use serde_json::Value;
use chrono::Utc;

// Εισαγωγή των macros ώστε να είναι ορατά στο test
use rs_lib_ng::{trace, debug, info, warn, error, fatal};

#[tokio::test]
async fn logger_sends_info_and_error_records() {
    let (tx, mut rx) = mpsc::channel::<LogRecord>(16);

    let cfg = LoggerConfig {
        level: LogLevel::Info,
        component: "test-component".to_string(),
    };
    let config = Arc::new(ArcSwap::from_pointee(cfg));
    let logger = Logger { sender: tx.clone(), config: config.clone() };

    trace!(logger, "trace message", "k" => "v1");
    debug!(logger, "debug message", "k" => "v2");
    info!(logger, "info message", "k" => "v3");
    warn!(logger, "warn message", "k" => "v4");
    error!(logger, "error message", "error" => "boom");
    fatal!(logger, "fatal message", "k" => "v6");

    // Πιάνουμε μέχρι 4 records (info,warn,error,fatal)
    let mut recs = Vec::new();
    for _ in 0..4 {
        match tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await {
            Ok(Some(r)) => recs.push(r),
            _ => break,
        }
    }

    assert_eq!(recs.len(), 4, "Expected 4 records (info,warn,error,fatal)");

    // Έλεγχοι επιπέδων και μηνυμάτων
    let levels: Vec<_> = recs.iter().map(|r| r.level.clone()).collect();
    let msgs: Vec<_> = recs.iter().map(|r| r.msg.clone()).collect();

    assert!(levels.contains(&LogLevel::Info));
    assert!(levels.contains(&LogLevel::Warn));
    assert!(levels.contains(&LogLevel::Error));
    assert!(levels.contains(&LogLevel::Fatal));

    assert!(msgs.iter().any(|m| m == "info message"));
    assert!(msgs.iter().any(|m| m == "warn message"));
    assert!(msgs.iter().any(|m| m == "error message"));
    assert!(msgs.iter().any(|m| m == "fatal message"));

    // Έλεγχος ctx για info record
    let info_rec = recs.iter().find(|r| r.level == LogLevel::Info).expect("info record missing");
    assert!(info_rec.ctx.contains_key("k"));
    if let Some(Value::String(s)) = info_rec.ctx.get("k") {
        assert_eq!(s, "v3");
    } else {
        panic!("info.k missing or wrong type");
    }

    // Timestamp sanity
    let now = Utc::now();
    let delta = now.signed_duration_since(recs[0].ts);
    assert!(delta.num_seconds() >= 0 && delta.num_minutes() < 5, "timestamp should be recent");
}
