use axum::http::HeaderMap;

use crate::{
    domain::settings::AppSettingsView,
    dto::settings::{SettingsResponse, UpdateSettingsRequest},
    errors::AppError,
    repository::settings_repo,
    state::AppState,
    utils::time::now_rfc3339,
};

use super::{auth_service, mihomo_core_service, url_safety};

const PUBLIC_BASE_URL: &str = "site.public_base_url";
const LATENCY_AUTO_ENABLED: &str = "latency.auto_enabled";
pub(crate) const LATENCY_CORE_PATH: &str = "latency.core_path";
const LATENCY_INTERVAL_MINUTES: &str = "latency.interval_minutes";
const LATENCY_CONCURRENCY: &str = "latency.concurrency";
const LATENCY_TEST_URL: &str = "latency.test_url";
const LATENCY_TIMEOUT_SECS: &str = "latency.timeout_secs";
const IP_PROBE_AUTO_ENABLED: &str = "ip_probe.auto_enabled";
const IP_PROBE_INTERVAL_MINUTES: &str = "ip_probe.interval_minutes";
const IP_PROBE_AFTER_UPSTREAM_IMPORT: &str = "ip_probe.after_upstream_import";
const COUNTRY_DETECTION_AUTO_ENABLED: &str = "ip_intelligence.auto_enabled";
const COUNTRY_DETECTION_INTERVAL_MINUTES: &str = "ip_intelligence.interval_minutes";
const CONNECTIVITY_DEFAULT_TARGET: &str = "connectivity.default_target";
const CONNECTIVITY_DEFAULT_ROUNDS: &str = "connectivity.default_rounds";
const CONNECTIVITY_SYNC_LAST_LATENCY: &str = "connectivity.sync_last_latency";
const PUBLIC_EXPORT_CACHE_TTL_SECONDS: &str = "public_export.cache_ttl_seconds";
const PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE: &str = "public_export.ip_limit_per_minute";
const PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE: &str = "public_export.global_limit_per_minute";
const MIHOMO_COUNTRY_LOAD_MIN_NODES: &str = "mihomo.country_load_min_nodes";
const MIHOMO_COUNTRY_FALLBACK_MIN_NODES: &str = "mihomo.country_fallback_min_nodes";
const DEFAULT_LATENCY_AUTO_ENABLED: bool = true;
const DEFAULT_LATENCY_INTERVAL_MINUTES: i64 = 30;
const DEFAULT_LATENCY_CONCURRENCY: i64 = 2;
const DEFAULT_LATENCY_TEST_URL: &str = "https://cp.cloudflare.com/generate_204";
const DEFAULT_LATENCY_TIMEOUT_SECS: i64 = 10;
const DEFAULT_IP_PROBE_AUTO_ENABLED: bool = false;
const DEFAULT_IP_PROBE_INTERVAL_MINUTES: i64 = 360;
const DEFAULT_IP_PROBE_AFTER_UPSTREAM_IMPORT: bool = true;
const DEFAULT_COUNTRY_DETECTION_AUTO_ENABLED: bool = true;
const DEFAULT_COUNTRY_DETECTION_INTERVAL_MINUTES: i64 = 5;
const DEFAULT_CONNECTIVITY_TARGET: &str = "system_default";
const DEFAULT_CONNECTIVITY_ROUNDS: i64 = 3;
const DEFAULT_CONNECTIVITY_SYNC_LAST_LATENCY: bool = true;
const DEFAULT_PUBLIC_EXPORT_CACHE_TTL_SECONDS: i64 = 30;
const DEFAULT_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE: i64 = 120;
const DEFAULT_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE: i64 = 1200;
const DEFAULT_MIHOMO_COUNTRY_LOAD_MIN_NODES: i64 = 2;
const DEFAULT_MIHOMO_COUNTRY_FALLBACK_MIN_NODES: i64 = 3;
pub(crate) const MAX_LATENCY_CONCURRENCY: i64 = 8;
pub(crate) const MIN_IP_PROBE_INTERVAL_MINUTES: i64 = 15;
pub(crate) const MAX_IP_PROBE_INTERVAL_MINUTES: i64 = 10080;
pub(crate) const MIN_COUNTRY_DETECTION_INTERVAL_MINUTES: i64 = 5;
pub(crate) const MAX_COUNTRY_DETECTION_INTERVAL_MINUTES: i64 = 1440;
pub(crate) const MIN_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE: i64 = 10;
pub(crate) const MAX_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE: i64 = 6000;
pub(crate) const MIN_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE: i64 = 100;
pub(crate) const MAX_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE: i64 = 60000;
pub(crate) const MAX_PUBLIC_EXPORT_CACHE_TTL_SECONDS: i64 = 300;
pub(crate) const MAX_MIHOMO_COUNTRY_GROUP_MIN_NODES: i64 = 20;

pub async fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    auth_service::require_user(state, headers).await.map(|_| ())
}

pub async fn get_settings(state: &AppState) -> Result<SettingsResponse, AppError> {
    Ok(SettingsResponse {
        code: "00000",
        data: load_settings(state).await?,
    })
}

pub async fn update_settings(
    state: &AppState,
    payload: UpdateSettingsRequest,
) -> Result<SettingsResponse, AppError> {
    let current = load_settings(state).await?;
    validate_interval(payload.latency_interval_minutes)?;
    validate_concurrency(payload.latency_concurrency)?;
    validate_timeout(payload.latency_timeout_secs)?;
    if let Some(interval) = payload.ip_probe_interval_minutes {
        validate_ip_probe_interval(interval)?;
    }
    if let Some(interval) = payload.country_detection_interval_minutes {
        validate_country_detection_interval(interval)?;
    }
    if let Some(target) = payload.connectivity_default_target.as_deref() {
        validate_connectivity_target(target)?;
    }
    if let Some(rounds) = payload.connectivity_default_rounds {
        validate_connectivity_rounds(rounds)?;
    }
    validate_public_export_settings(
        payload.public_export_cache_ttl_seconds,
        payload
            .public_export_ip_limit_per_minute
            .unwrap_or(current.public_export_ip_limit_per_minute),
        payload
            .public_export_global_limit_per_minute
            .unwrap_or(current.public_export_global_limit_per_minute),
    )?;
    validate_country_group_settings(
        payload
            .mihomo_country_load_min_nodes
            .unwrap_or(current.mihomo_country_load_min_nodes),
        payload
            .mihomo_country_fallback_min_nodes
            .unwrap_or(current.mihomo_country_fallback_min_nodes),
    )?;
    validate_core_path(&payload.latency_core_path)?;
    validate_test_url(&payload.latency_test_url).await?;
    validate_public_base_url(&payload.public_base_url)?;
    save_settings(state, &payload).await?;
    get_settings(state).await
}

pub async fn update_latency_core_path(state: &AppState, core_path: &str) -> Result<(), AppError> {
    settings_repo::set(
        &state.db,
        LATENCY_CORE_PATH,
        core_path.trim(),
        &now_rfc3339(),
    )
    .await?;
    Ok(())
}

pub async fn load_settings(state: &AppState) -> Result<AppSettingsView, AppError> {
    let values = settings_repo::get_many(
        &state.db,
        &[
            PUBLIC_BASE_URL,
            LATENCY_AUTO_ENABLED,
            LATENCY_INTERVAL_MINUTES,
            LATENCY_CONCURRENCY,
            LATENCY_CORE_PATH,
            LATENCY_TEST_URL,
            LATENCY_TIMEOUT_SECS,
            IP_PROBE_AUTO_ENABLED,
            IP_PROBE_INTERVAL_MINUTES,
            IP_PROBE_AFTER_UPSTREAM_IMPORT,
            COUNTRY_DETECTION_AUTO_ENABLED,
            COUNTRY_DETECTION_INTERVAL_MINUTES,
            CONNECTIVITY_DEFAULT_TARGET,
            CONNECTIVITY_DEFAULT_ROUNDS,
            CONNECTIVITY_SYNC_LAST_LATENCY,
            PUBLIC_EXPORT_CACHE_TTL_SECONDS,
            PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE,
            PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE,
            MIHOMO_COUNTRY_LOAD_MIN_NODES,
            MIHOMO_COUNTRY_FALLBACK_MIN_NODES,
        ],
    )
    .await?;
    let public_base_url = values.get(PUBLIC_BASE_URL).cloned().unwrap_or_default();
    let auto_enabled = values
        .get(LATENCY_AUTO_ENABLED)
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(DEFAULT_LATENCY_AUTO_ENABLED);
    let interval_minutes = values
        .get(LATENCY_INTERVAL_MINUTES)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LATENCY_INTERVAL_MINUTES);
    let concurrency = values
        .get(LATENCY_CONCURRENCY)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LATENCY_CONCURRENCY);
    let core_path = values.get(LATENCY_CORE_PATH).cloned().unwrap_or_default();
    let test_url = values
        .get(LATENCY_TEST_URL)
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_LATENCY_TEST_URL.to_string());
    let timeout_secs = values
        .get(LATENCY_TIMEOUT_SECS)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LATENCY_TIMEOUT_SECS);
    let ip_probe_auto_enabled = values
        .get(IP_PROBE_AUTO_ENABLED)
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(DEFAULT_IP_PROBE_AUTO_ENABLED);
    let ip_probe_interval_minutes = values
        .get(IP_PROBE_INTERVAL_MINUTES)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_IP_PROBE_INTERVAL_MINUTES);
    let ip_probe_after_upstream_import = values
        .get(IP_PROBE_AFTER_UPSTREAM_IMPORT)
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(DEFAULT_IP_PROBE_AFTER_UPSTREAM_IMPORT);
    let country_detection_auto_enabled = values
        .get(COUNTRY_DETECTION_AUTO_ENABLED)
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(DEFAULT_COUNTRY_DETECTION_AUTO_ENABLED);
    let country_detection_interval_minutes = values
        .get(COUNTRY_DETECTION_INTERVAL_MINUTES)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_COUNTRY_DETECTION_INTERVAL_MINUTES);
    let connectivity_default_target = values
        .get(CONNECTIVITY_DEFAULT_TARGET)
        .filter(|value| validate_connectivity_target(value).is_ok())
        .cloned()
        .unwrap_or_else(|| DEFAULT_CONNECTIVITY_TARGET.to_string());
    let connectivity_default_rounds = values
        .get(CONNECTIVITY_DEFAULT_ROUNDS)
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| validate_connectivity_rounds(*value).is_ok())
        .unwrap_or(DEFAULT_CONNECTIVITY_ROUNDS);
    let connectivity_sync_last_latency = values
        .get(CONNECTIVITY_SYNC_LAST_LATENCY)
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(DEFAULT_CONNECTIVITY_SYNC_LAST_LATENCY);
    let public_export_cache_ttl_seconds = values
        .get(PUBLIC_EXPORT_CACHE_TTL_SECONDS)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_PUBLIC_EXPORT_CACHE_TTL_SECONDS)
        .clamp(0, MAX_PUBLIC_EXPORT_CACHE_TTL_SECONDS);
    let public_export_ip_limit_per_minute = values
        .get(PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE)
        .clamp(
            MIN_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE,
            MAX_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE,
        );
    let public_export_global_limit_per_minute = values
        .get(PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE)
        .clamp(
            MIN_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE.max(public_export_ip_limit_per_minute),
            MAX_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE,
        );
    let mihomo_country_load_min_nodes = values
        .get(MIHOMO_COUNTRY_LOAD_MIN_NODES)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_MIHOMO_COUNTRY_LOAD_MIN_NODES)
        .clamp(2, MAX_MIHOMO_COUNTRY_GROUP_MIN_NODES);
    let mihomo_country_fallback_min_nodes = values
        .get(MIHOMO_COUNTRY_FALLBACK_MIN_NODES)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_MIHOMO_COUNTRY_FALLBACK_MIN_NODES)
        .clamp(
            mihomo_country_load_min_nodes,
            MAX_MIHOMO_COUNTRY_GROUP_MIN_NODES,
        );

    Ok(AppSettingsView {
        public_base_url: trim_trailing_slash(&public_base_url),
        latency_auto_enabled: auto_enabled,
        latency_interval_minutes: interval_minutes.clamp(5, 1440),
        latency_concurrency: concurrency.clamp(1, MAX_LATENCY_CONCURRENCY),
        latency_core_path: core_path,
        latency_test_url: test_url,
        latency_timeout_secs: timeout_secs.clamp(3, 60),
        ip_probe_auto_enabled,
        ip_probe_interval_minutes: ip_probe_interval_minutes
            .clamp(MIN_IP_PROBE_INTERVAL_MINUTES, MAX_IP_PROBE_INTERVAL_MINUTES),
        ip_probe_after_upstream_import,
        country_detection_auto_enabled,
        country_detection_interval_minutes: country_detection_interval_minutes.clamp(
            MIN_COUNTRY_DETECTION_INTERVAL_MINUTES,
            MAX_COUNTRY_DETECTION_INTERVAL_MINUTES,
        ),
        connectivity_default_target,
        connectivity_default_rounds,
        connectivity_sync_last_latency,
        public_export_cache_ttl_seconds,
        public_export_ip_limit_per_minute,
        public_export_global_limit_per_minute,
        mihomo_country_load_min_nodes,
        mihomo_country_fallback_min_nodes,
    })
}

async fn save_settings(state: &AppState, payload: &UpdateSettingsRequest) -> Result<(), AppError> {
    let now = now_rfc3339();
    settings_repo::set(
        &state.db,
        PUBLIC_BASE_URL,
        &trim_trailing_slash(&payload.public_base_url),
        &now,
    )
    .await?;
    settings_repo::set(
        &state.db,
        LATENCY_AUTO_ENABLED,
        if payload.latency_auto_enabled {
            "true"
        } else {
            "false"
        },
        &now,
    )
    .await?;
    settings_repo::set(
        &state.db,
        LATENCY_INTERVAL_MINUTES,
        &payload.latency_interval_minutes.to_string(),
        &now,
    )
    .await?;
    settings_repo::set(
        &state.db,
        LATENCY_CONCURRENCY,
        &payload.latency_concurrency.to_string(),
        &now,
    )
    .await?;
    settings_repo::set(
        &state.db,
        LATENCY_CORE_PATH,
        payload.latency_core_path.trim(),
        &now,
    )
    .await?;
    settings_repo::set(
        &state.db,
        LATENCY_TEST_URL,
        payload.latency_test_url.trim(),
        &now,
    )
    .await?;
    settings_repo::set(
        &state.db,
        LATENCY_TIMEOUT_SECS,
        &payload.latency_timeout_secs.to_string(),
        &now,
    )
    .await?;
    if let Some(enabled) = payload.ip_probe_auto_enabled {
        settings_repo::set(
            &state.db,
            IP_PROBE_AUTO_ENABLED,
            if enabled { "true" } else { "false" },
            &now,
        )
        .await?;
    }
    if let Some(interval) = payload.ip_probe_interval_minutes {
        settings_repo::set(
            &state.db,
            IP_PROBE_INTERVAL_MINUTES,
            &interval.to_string(),
            &now,
        )
        .await?;
    }
    if let Some(enabled) = payload.ip_probe_after_upstream_import {
        settings_repo::set(
            &state.db,
            IP_PROBE_AFTER_UPSTREAM_IMPORT,
            if enabled { "true" } else { "false" },
            &now,
        )
        .await?;
    }
    if let Some(enabled) = payload.country_detection_auto_enabled {
        settings_repo::set(
            &state.db,
            COUNTRY_DETECTION_AUTO_ENABLED,
            if enabled { "true" } else { "false" },
            &now,
        )
        .await?;
    }
    if let Some(interval) = payload.country_detection_interval_minutes {
        settings_repo::set(
            &state.db,
            COUNTRY_DETECTION_INTERVAL_MINUTES,
            &interval.to_string(),
            &now,
        )
        .await?;
    }
    if let Some(target) = payload.connectivity_default_target.as_deref() {
        settings_repo::set(&state.db, CONNECTIVITY_DEFAULT_TARGET, target.trim(), &now).await?;
    }
    if let Some(rounds) = payload.connectivity_default_rounds {
        settings_repo::set(
            &state.db,
            CONNECTIVITY_DEFAULT_ROUNDS,
            &rounds.to_string(),
            &now,
        )
        .await?;
    }
    if let Some(enabled) = payload.connectivity_sync_last_latency {
        settings_repo::set(
            &state.db,
            CONNECTIVITY_SYNC_LAST_LATENCY,
            if enabled { "true" } else { "false" },
            &now,
        )
        .await?;
    }
    save_optional_i64(
        state,
        PUBLIC_EXPORT_CACHE_TTL_SECONDS,
        payload.public_export_cache_ttl_seconds,
        &now,
    )
    .await?;
    save_optional_i64(
        state,
        PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE,
        payload.public_export_ip_limit_per_minute,
        &now,
    )
    .await?;
    save_optional_i64(
        state,
        PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE,
        payload.public_export_global_limit_per_minute,
        &now,
    )
    .await?;
    save_optional_i64(
        state,
        MIHOMO_COUNTRY_LOAD_MIN_NODES,
        payload.mihomo_country_load_min_nodes,
        &now,
    )
    .await?;
    save_optional_i64(
        state,
        MIHOMO_COUNTRY_FALLBACK_MIN_NODES,
        payload.mihomo_country_fallback_min_nodes,
        &now,
    )
    .await?;
    if payload.public_export_cache_ttl_seconds.is_some()
        || payload.mihomo_country_load_min_nodes.is_some()
        || payload.mihomo_country_fallback_min_nodes.is_some()
    {
        state.clear_public_export_cache().await;
    }
    Ok(())
}

async fn save_optional_i64(
    state: &AppState,
    key: &str,
    value: Option<i64>,
    now: &str,
) -> Result<(), AppError> {
    if let Some(value) = value {
        settings_repo::set(&state.db, key, &value.to_string(), now).await?;
    }
    Ok(())
}

fn validate_interval(interval_minutes: i64) -> Result<(), AppError> {
    if !(5..=1440).contains(&interval_minutes) {
        return Err(AppError::BadRequest(
            "latency interval must be between 5 and 1440 minutes".to_string(),
        ));
    }
    Ok(())
}

fn validate_concurrency(concurrency: i64) -> Result<(), AppError> {
    if !(1..=MAX_LATENCY_CONCURRENCY).contains(&concurrency) {
        return Err(AppError::BadRequest(format!(
            "latency concurrency must be between 1 and {MAX_LATENCY_CONCURRENCY}"
        )));
    }
    Ok(())
}

fn validate_ip_probe_interval(interval_minutes: i64) -> Result<(), AppError> {
    if !(MIN_IP_PROBE_INTERVAL_MINUTES..=MAX_IP_PROBE_INTERVAL_MINUTES).contains(&interval_minutes)
    {
        return Err(AppError::BadRequest(format!(
            "IP probe interval must be between {MIN_IP_PROBE_INTERVAL_MINUTES} and {MAX_IP_PROBE_INTERVAL_MINUTES} minutes"
        )));
    }
    Ok(())
}

fn validate_country_detection_interval(interval_minutes: i64) -> Result<(), AppError> {
    if !(MIN_COUNTRY_DETECTION_INTERVAL_MINUTES..=MAX_COUNTRY_DETECTION_INTERVAL_MINUTES)
        .contains(&interval_minutes)
    {
        return Err(AppError::BadRequest(format!(
            "country detection interval must be between {MIN_COUNTRY_DETECTION_INTERVAL_MINUTES} and {MAX_COUNTRY_DETECTION_INTERVAL_MINUTES} minutes"
        )));
    }
    Ok(())
}

fn validate_connectivity_target(target: &str) -> Result<(), AppError> {
    if !matches!(
        target.trim(),
        "system_default" | "cloudflare_204" | "google_204"
    ) {
        return Err(AppError::BadRequest(
            "connectivity default target is not a supported preset".to_string(),
        ));
    }
    Ok(())
}

fn validate_connectivity_rounds(rounds: i64) -> Result<(), AppError> {
    if !matches!(rounds, 1 | 3 | 5) {
        return Err(AppError::BadRequest(
            "connectivity default rounds must be 1, 3, or 5".to_string(),
        ));
    }
    Ok(())
}

fn validate_public_export_settings(
    cache_ttl_seconds: Option<i64>,
    ip_limit_per_minute: i64,
    global_limit_per_minute: i64,
) -> Result<(), AppError> {
    if let Some(ttl) = cache_ttl_seconds
        && !(0..=MAX_PUBLIC_EXPORT_CACHE_TTL_SECONDS).contains(&ttl)
    {
        return Err(AppError::BadRequest(format!(
            "public export cache TTL must be between 0 and {MAX_PUBLIC_EXPORT_CACHE_TTL_SECONDS} seconds"
        )));
    }
    if !(MIN_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE..=MAX_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE)
        .contains(&ip_limit_per_minute)
    {
        return Err(AppError::BadRequest(format!(
            "public export IP limit must be between {MIN_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE} and {MAX_PUBLIC_EXPORT_IP_LIMIT_PER_MINUTE} requests per minute"
        )));
    }
    if !(MIN_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE..=MAX_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE)
        .contains(&global_limit_per_minute)
    {
        return Err(AppError::BadRequest(format!(
            "public export global limit must be between {MIN_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE} and {MAX_PUBLIC_EXPORT_GLOBAL_LIMIT_PER_MINUTE} requests per minute"
        )));
    }
    if global_limit_per_minute < ip_limit_per_minute {
        return Err(AppError::BadRequest(
            "public export global limit cannot be lower than the per-IP limit".to_string(),
        ));
    }
    Ok(())
}

fn validate_country_group_settings(load: i64, fallback: i64) -> Result<(), AppError> {
    for (label, value) in [("load", load), ("fallback", fallback)] {
        if !(2..=MAX_MIHOMO_COUNTRY_GROUP_MIN_NODES).contains(&value) {
            return Err(AppError::BadRequest(format!(
                "Mihomo country {label} threshold must be between 2 and {MAX_MIHOMO_COUNTRY_GROUP_MIN_NODES} nodes"
            )));
        }
    }
    if fallback < load {
        return Err(AppError::BadRequest(
            "Mihomo country fallback threshold cannot be lower than the load threshold".to_string(),
        ));
    }
    Ok(())
}

fn validate_timeout(timeout_secs: i64) -> Result<(), AppError> {
    if !(3..=60).contains(&timeout_secs) {
        return Err(AppError::BadRequest(
            "latency timeout must be between 3 and 60 seconds".to_string(),
        ));
    }
    Ok(())
}

fn validate_core_path(core_path: &str) -> Result<(), AppError> {
    let value = core_path.trim();
    if value.is_empty() {
        return Ok(());
    }
    if value.chars().any(char::is_control) {
        return Err(AppError::BadRequest(
            "latency core path must not contain control characters".to_string(),
        ));
    }
    if !mihomo_core_service::is_allowed_mihomo_binary_path(std::path::Path::new(value)) {
        return Err(AppError::BadRequest(
            "latency core path must point to a supported Mihomo binary name".to_string(),
        ));
    }
    Ok(())
}

async fn validate_test_url(test_url: &str) -> Result<(), AppError> {
    let value = test_url.trim();
    url_safety::validate_public_http_url(value, "latency test url")
        .await
        .map(|_| ())
}

fn validate_public_base_url(public_base_url: &str) -> Result<(), AppError> {
    let value = public_base_url.trim();
    if value.is_empty() {
        return Ok(());
    }
    url_safety::validate_http_url_format(value, "public base url")
}

fn trim_trailing_slash(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_COUNTRY_DETECTION_INTERVAL_MINUTES, MAX_IP_PROBE_INTERVAL_MINUTES,
        MAX_LATENCY_CONCURRENCY, MAX_MIHOMO_COUNTRY_GROUP_MIN_NODES,
        MAX_PUBLIC_EXPORT_CACHE_TTL_SECONDS, MIN_COUNTRY_DETECTION_INTERVAL_MINUTES,
        MIN_IP_PROBE_INTERVAL_MINUTES, validate_concurrency, validate_connectivity_rounds,
        validate_connectivity_target, validate_core_path, validate_country_detection_interval,
        validate_country_group_settings, validate_ip_probe_interval,
        validate_public_export_settings,
    };

    #[test]
    fn rejects_non_mihomo_core_path() {
        assert!(validate_core_path("cmd.exe").is_err());
        assert!(validate_core_path("/bin/sh").is_err());
    }

    #[test]
    fn allows_empty_core_path_for_auto_discovery() {
        assert!(validate_core_path("").is_ok());
    }

    #[test]
    fn validates_latency_concurrency_bounds() {
        assert!(validate_concurrency(1).is_ok());
        assert!(validate_concurrency(MAX_LATENCY_CONCURRENCY).is_ok());
        assert!(validate_concurrency(0).is_err());
        assert!(validate_concurrency(MAX_LATENCY_CONCURRENCY + 1).is_err());
    }

    #[test]
    fn validates_ip_probe_interval_bounds() {
        assert!(validate_ip_probe_interval(MIN_IP_PROBE_INTERVAL_MINUTES).is_ok());
        assert!(validate_ip_probe_interval(MAX_IP_PROBE_INTERVAL_MINUTES).is_ok());
        assert!(validate_ip_probe_interval(MIN_IP_PROBE_INTERVAL_MINUTES - 1).is_err());
        assert!(validate_ip_probe_interval(MAX_IP_PROBE_INTERVAL_MINUTES + 1).is_err());
    }

    #[test]
    fn validates_country_detection_interval_bounds() {
        assert!(
            validate_country_detection_interval(MIN_COUNTRY_DETECTION_INTERVAL_MINUTES).is_ok()
        );
        assert!(
            validate_country_detection_interval(MAX_COUNTRY_DETECTION_INTERVAL_MINUTES).is_ok()
        );
        assert!(
            validate_country_detection_interval(MIN_COUNTRY_DETECTION_INTERVAL_MINUTES - 1)
                .is_err()
        );
        assert!(
            validate_country_detection_interval(MAX_COUNTRY_DETECTION_INTERVAL_MINUTES + 1)
                .is_err()
        );
    }

    #[test]
    fn validates_connectivity_defaults() {
        assert!(validate_connectivity_target("system_default").is_ok());
        assert!(validate_connectivity_target("https://example.com").is_err());
        assert!(validate_connectivity_rounds(1).is_ok());
        assert!(validate_connectivity_rounds(5).is_ok());
        assert!(validate_connectivity_rounds(2).is_err());
    }

    #[test]
    fn validates_public_export_limits() {
        assert!(
            validate_public_export_settings(Some(MAX_PUBLIC_EXPORT_CACHE_TTL_SECONDS), 120, 1200)
                .is_ok()
        );
        assert!(validate_public_export_settings(Some(30), 120, 100).is_err());
    }

    #[test]
    fn validates_country_group_thresholds() {
        assert!(validate_country_group_settings(2, 3).is_ok());
        assert!(validate_country_group_settings(3, 2).is_err());
        assert!(
            validate_country_group_settings(2, MAX_MIHOMO_COUNTRY_GROUP_MIN_NODES + 1).is_err()
        );
    }
}
