use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppSettingsView {
    pub latency_auto_enabled: bool,
    pub latency_interval_minutes: i64,
    pub latency_core_path: String,
    pub latency_test_url: String,
    pub latency_timeout_secs: i64,
}
