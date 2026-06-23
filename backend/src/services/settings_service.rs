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
const DEFAULT_LATENCY_AUTO_ENABLED: bool = true;
const DEFAULT_LATENCY_INTERVAL_MINUTES: i64 = 30;
const DEFAULT_LATENCY_CONCURRENCY: i64 = 2;
const DEFAULT_LATENCY_TEST_URL: &str = "https://cp.cloudflare.com/generate_204";
const DEFAULT_LATENCY_TIMEOUT_SECS: i64 = 10;
pub(crate) const MAX_LATENCY_CONCURRENCY: i64 = 8;

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
    validate_concurrency(payload.latency_concurrency)?;
    validate_timeout(payload.latency_timeout_secs)?;
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
    let public_base_url = settings_repo::get(&state.db, PUBLIC_BASE_URL)
        .await?
        .unwrap_or_default();
    let auto_enabled = settings_repo::get(&state.db, LATENCY_AUTO_ENABLED)
        .await?
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(DEFAULT_LATENCY_AUTO_ENABLED);
    let interval_minutes = settings_repo::get(&state.db, LATENCY_INTERVAL_MINUTES)
        .await?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LATENCY_INTERVAL_MINUTES);
    let concurrency = settings_repo::get(&state.db, LATENCY_CONCURRENCY)
        .await?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LATENCY_CONCURRENCY);
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
        public_base_url: trim_trailing_slash(&public_base_url),
        latency_auto_enabled: auto_enabled,
        latency_interval_minutes: interval_minutes.clamp(5, 1440),
        latency_concurrency: concurrency.clamp(1, MAX_LATENCY_CONCURRENCY),
        latency_core_path: core_path,
        latency_test_url: test_url,
        latency_timeout_secs: timeout_secs.clamp(3, 60),
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
    use super::{MAX_LATENCY_CONCURRENCY, validate_concurrency, validate_core_path};

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
}
