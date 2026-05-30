use axum::http::HeaderMap;

use crate::{
    domain::settings::AppSettingsView,
    dto::settings::{SettingsResponse, UpdateSettingsRequest},
    errors::AppError,
    repository::settings_repo,
    state::AppState,
    utils::time::now_rfc3339,
};

use super::auth_service;

const LATENCY_AUTO_ENABLED: &str = "latency.auto_enabled";
pub(crate) const LATENCY_CORE_PATH: &str = "latency.core_path";
const LATENCY_INTERVAL_MINUTES: &str = "latency.interval_minutes";
const LATENCY_TEST_URL: &str = "latency.test_url";
const LATENCY_TIMEOUT_SECS: &str = "latency.timeout_secs";
const DEFAULT_LATENCY_AUTO_ENABLED: bool = true;
const DEFAULT_LATENCY_INTERVAL_MINUTES: i64 = 30;
const DEFAULT_LATENCY_TEST_URL: &str = "https://www.gstatic.com/generate_204";
const DEFAULT_LATENCY_TIMEOUT_SECS: i64 = 10;

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
    validate_interval(payload.latency_interval_minutes)?;
    validate_timeout(payload.latency_timeout_secs)?;
    validate_test_url(&payload.latency_test_url)?;
    save_settings(
        state,
        payload.latency_auto_enabled,
        payload.latency_interval_minutes,
        payload.latency_core_path.trim(),
        payload.latency_test_url.trim(),
        payload.latency_timeout_secs,
    )
    .await?;
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
    let auto_enabled = settings_repo::get(&state.db, LATENCY_AUTO_ENABLED)
        .await?
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(DEFAULT_LATENCY_AUTO_ENABLED);
    let interval_minutes = settings_repo::get(&state.db, LATENCY_INTERVAL_MINUTES)
        .await?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LATENCY_INTERVAL_MINUTES);
    let core_path = settings_repo::get(&state.db, LATENCY_CORE_PATH)
        .await?
        .unwrap_or_default();
    let test_url = settings_repo::get(&state.db, LATENCY_TEST_URL)
        .await?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_LATENCY_TEST_URL.to_string());
    let timeout_secs = settings_repo::get(&state.db, LATENCY_TIMEOUT_SECS)
        .await?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LATENCY_TIMEOUT_SECS);

    Ok(AppSettingsView {
        latency_auto_enabled: auto_enabled,
        latency_interval_minutes: interval_minutes.clamp(5, 1440),
        latency_core_path: core_path,
        latency_test_url: test_url,
        latency_timeout_secs: timeout_secs.clamp(3, 60),
    })
}

async fn save_settings(
    state: &AppState,
    auto_enabled: bool,
    interval_minutes: i64,
    core_path: &str,
    test_url: &str,
    timeout_secs: i64,
) -> Result<(), AppError> {
    let now = now_rfc3339();
    settings_repo::set(
        &state.db,
        LATENCY_AUTO_ENABLED,
        if auto_enabled { "true" } else { "false" },
        &now,
    )
    .await?;
    settings_repo::set(
        &state.db,
        LATENCY_INTERVAL_MINUTES,
        &interval_minutes.to_string(),
        &now,
    )
    .await?;
    settings_repo::set(&state.db, LATENCY_CORE_PATH, core_path, &now).await?;
    settings_repo::set(&state.db, LATENCY_TEST_URL, test_url, &now).await?;
    settings_repo::set(
        &state.db,
        LATENCY_TIMEOUT_SECS,
        &timeout_secs.to_string(),
        &now,
    )
    .await?;
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

fn validate_timeout(timeout_secs: i64) -> Result<(), AppError> {
    if !(3..=60).contains(&timeout_secs) {
        return Err(AppError::BadRequest(
            "latency timeout must be between 3 and 60 seconds".to_string(),
        ));
    }
    Ok(())
}

fn validate_test_url(test_url: &str) -> Result<(), AppError> {
    let value = test_url.trim();
    if !(value.starts_with("http://") || value.starts_with("https://")) {
        return Err(AppError::BadRequest(
            "latency test url must start with http:// or https://".to_string(),
        ));
    }
    Ok(())
}
