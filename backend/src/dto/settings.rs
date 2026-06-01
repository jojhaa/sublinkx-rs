use serde::{Deserialize, Serialize};

use crate::domain::settings::AppSettingsView;

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub public_base_url: String,
    pub latency_auto_enabled: bool,
    pub latency_interval_minutes: i64,
    pub latency_core_path: String,
    pub latency_test_url: String,
    pub latency_timeout_secs: i64,
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
