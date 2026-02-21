use serde::{Deserialize, Serialize};
use serde_json::Value;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace, Debug, Info, Warn, Error, Fatal,
}

#[derive(Debug, Serialize)]
pub struct LogRecord {
    pub ts: DateTime<Utc>,
    pub level: LogLevel,
    pub msg: String,
    pub component: String,
    pub ctx: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sys: Option<SysInfo>,
}

#[derive(Debug, Serialize)]
pub struct SysInfo {
    pub cpu_usage: f32,
    pub mem_used_kb: u64,
    pub load_avg: Vec<f64>,
    pub uptime_secs: u64,
}
