use serde_json::{Value, json};
use figment::{Figment, providers::{Format, Json, Env}};
use arc_swap::ArcSwap;
use std::sync::Arc;
use crate::core::error::NgError;

pub mod cloud;

pub struct ConfigManager {
    current: ArcSwap<Value>,
    source_info: String,
}

impl ConfigManager {
    /// LOCAL: Merges file + WEBLIB_ env vars. Fails if file missing.
    pub fn get_local_config(path: &str) -> Result<Self, NgError> {
        if !std::path::Path::new(path).exists() {
            return Err(NgError::ConfigError(format!("Local file not found: {}", path)));
        }

        let data: Value = Figment::new()
            .merge(Json::file(path))
            .merge(Env::prefixed("WEBLIB_").split("__"))
            .extract()
            .map_err(|e| NgError::ConfigError(e.to_string()))?;

        Ok(Self {
            current: ArcSwap::from_pointee(data),
            source_info: format!("local:{}", path),
        })
    }

    /// CLOUD: Downloads, decrypts, and extracts (Binary-Name + commonAll)
    pub async fn get_cloud_config(url: &str) -> Result<Self, NgError> {
        let full_json = cloud::load_remote_json(url).await?;
        
        // Binary name selection
        let bin_name = std::env::current_exe()
            .ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "default".to_string());

        let common = full_json.get("commonAll").cloned().unwrap_or(json!({}));
        let specific = full_json.get(&bin_name).cloned().unwrap_or(json!({}));

        // Merge logic: specific overrides common
        let mut merged = common;
        if let (Some(m), Some(s)) = (merged.as_object_mut(), specific.as_object()) {
            for (k, v) in s { m.insert(k.clone(), v.clone()); }
        }

        Ok(Self {
            current: ArcSwap::from_pointee(merged),
            source_info: format!("cloud:{}", url),
        })
    }

    pub fn get(&self) -> Arc<Value> {
        self.current.load_full()
    }
}
