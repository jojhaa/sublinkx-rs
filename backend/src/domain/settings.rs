use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppSettingsView {
    pub public_base_url: String,
    pub latency_auto_enabled: bool,
    pub latency_interval_minutes: i64,
    pub latency_concurrency: i64,
    pub latency_core_path: String,
    pub latency_test_url: String,
    pub latency_timeout_secs: i64,
    pub ip_probe_auto_enabled: bool,
    pub ip_probe_interval_minutes: i64,
    pub ip_probe_after_upstream_import: bool,
    pub country_detection_auto_enabled: bool,
    pub country_detection_interval_minutes: i64,
    pub risk_enforcement_enabled: bool,
    pub connectivity_default_target: String,
    pub connectivity_default_rounds: i64,
    pub connectivity_sync_last_latency: bool,
    pub public_export_cache_ttl_seconds: i64,
    pub public_export_ip_limit_per_minute: i64,
    pub public_export_global_limit_per_minute: i64,
    pub mihomo_country_load_min_nodes: i64,
    pub mihomo_country_fallback_min_nodes: i64,
}
