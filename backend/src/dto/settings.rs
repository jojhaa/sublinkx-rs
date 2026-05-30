use serde::{Deserialize, Serialize};

use crate::domain::settings::AppSettingsView;

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
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
