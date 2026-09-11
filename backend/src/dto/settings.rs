use serde::{Deserialize, Serialize};

use crate::domain::settings::AppSettingsView;

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub public_base_url: String,
    pub latency_auto_enabled: bool,
    pub latency_interval_minutes: i64,
    pub latency_concurrency: i64,
    pub latency_core_path: String,
    pub latency_test_url: String,
    pub latency_timeout_secs: i64,
    #[serde(default)]
    pub ip_probe_auto_enabled: Option<bool>,
    #[serde(default)]
    pub ip_probe_interval_minutes: Option<i64>,
    #[serde(default)]
    pub ip_probe_after_upstream_import: Option<bool>,
    #[serde(default)]
    pub country_detection_auto_enabled: Option<bool>,
    #[serde(default)]
    pub country_detection_interval_minutes: Option<i64>,
    #[serde(default)]
    pub risk_enforcement_enabled: Option<bool>,
    #[serde(default)]
    pub connectivity_default_target: Option<String>,
    #[serde(default)]
    pub connectivity_default_rounds: Option<i64>,
    #[serde(default)]
    pub connectivity_sync_last_latency: Option<bool>,
    #[serde(default)]
    pub public_export_cache_ttl_seconds: Option<i64>,
    #[serde(default)]
    pub public_export_ip_limit_per_minute: Option<i64>,
    #[serde(default)]
    pub public_export_global_limit_per_minute: Option<i64>,
    #[serde(default)]
    pub mihomo_country_load_min_nodes: Option<i64>,
    #[serde(default)]
    pub mihomo_country_fallback_min_nodes: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    pub code: &'static str,
    pub data: AppSettingsView,
}

#[derive(Debug, Serialize)]
pub struct MihomoCoreStatus {
    pub os: String,
    pub arch: String,
    pub supported: bool,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct MihomoCoreStatusResponse {
    pub code: &'static str,
    pub data: MihomoCoreStatus,
}

#[derive(Debug, Serialize)]
pub struct MihomoCoreDownloadResult {
    pub os: String,
    pub arch: String,
    pub version: String,
    pub asset_name: String,
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct MihomoCoreDownloadResponse {
    pub code: &'static str,
    pub data: MihomoCoreDownloadResult,
}
