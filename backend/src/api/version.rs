use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::{errors::AppError, services::auth_service, state::AppState};

const RELEASE_API_URL: &str = "https://api.github.com/repos/jojhaa/sublinkx-rs/releases/latest";
const RELEASE_PAGE_URL: &str = "https://github.com/jojhaa/sublinkx-rs/releases";
const UPDATE_CHECK_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Serialize)]
pub struct VersionResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub api_version: &'static str,
    pub repository: &'static str,
    pub license: &'static str,
    pub developer: DeveloperInfo,
}

#[derive(Serialize)]
pub struct DeveloperInfo {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheckResponse {
    pub checked: bool,
    pub update_available: bool,
    pub latest_version: Option<String>,
    pub latest_url: Option<String>,
    pub release_name: Option<String>,
    pub published_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    name: Option<String>,
    published_at: Option<String>,
}

pub async fn version(State(_state): State<AppState>) -> Json<VersionResponse> {
    Json(VersionResponse {
        name: "sublinkx-rs-backend",
        version: env!("CARGO_PKG_VERSION"),
        api_version: "v1",
        repository: "https://github.com/jojhaa/sublinkx-rs",
        license: "AGPL-3.0-or-later",
        developer: developer_info(),
    })
}

pub async fn update_check(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UpdateCheckResponse>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    Ok(Json(cached_update_check(env!("CARGO_PKG_VERSION")).await))
}

#[derive(Clone)]
struct CachedUpdateCheck {
    checked_at: Instant,
    response: UpdateCheckResponse,
}

static UPDATE_CHECK_CACHE: OnceLock<Mutex<Option<CachedUpdateCheck>>> = OnceLock::new();

async fn cached_update_check(current_version: &str) -> UpdateCheckResponse {
    let cache = UPDATE_CHECK_CACHE.get_or_init(|| Mutex::new(None));
    let mut cache = cache.lock().await;
    if let Some(cached) = cache.as_ref()
        && cached.checked_at.elapsed() < UPDATE_CHECK_CACHE_TTL
    {
        return cached.response.clone();
    }

    let response = check_latest_release(current_version).await;
    *cache = Some(CachedUpdateCheck {
        checked_at: Instant::now(),
        response: response.clone(),
    });
    response
}

async fn check_latest_release(current_version: &str) -> UpdateCheckResponse {
    match fetch_latest_release().await {
        Ok(release) => UpdateCheckResponse {
            checked: true,
            update_available: is_newer_version(&release.tag_name, current_version),
            latest_version: Some(release.tag_name),
            latest_url: Some(release.html_url),
            release_name: release.name,
            published_at: release.published_at,
            error: None,
        },
        Err(error) => UpdateCheckResponse {
            checked: false,
            update_available: false,
            latest_version: None,
            latest_url: Some(RELEASE_PAGE_URL.to_string()),
            release_name: None,
            published_at: None,
            error: Some(error),
        },
    }
}

async fn fetch_latest_release() -> Result<GithubRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("sublinkx-rs-update-checker")
        .build()
        .map_err(|error| format!("failed to create update client: {error}"))?;

    let response = client
        .get(RELEASE_API_URL)
        .send()
        .await
        .map_err(|error| format!("failed to fetch latest release: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("GitHub release API returned {}", response.status()));
    }

    response
        .json::<GithubRelease>()
        .await
        .map_err(|error| format!("failed to parse latest release: {error}"))
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let latest_parts = version_parts(latest);
    let current_parts = version_parts(current);

    for index in 0..latest_parts.len().max(current_parts.len()) {
        let latest_part = *latest_parts.get(index).unwrap_or(&0);
        let current_part = *current_parts.get(index).unwrap_or(&0);
        if latest_part != current_part {
            return latest_part > current_part;
        }
    }

    false
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .split(['.', '-', '+'])
        .map(|part| {
            part.chars()
                .take_while(|char| char.is_ascii_digit())
                .collect::<String>()
        })
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn developer_info() -> DeveloperInfo {
    DeveloperInfo {
        name: decode_masked(&[&[126, 67, 71], &[79, 10], &[121, 67, 70, 79, 68, 94]], 42),
        url: RELEASE_PAGE_URL
            .trim_end_matches("/sublinkx-rs/releases")
            .to_string(),
    }
}

fn decode_masked(chunks: &[&[u8]], key: u8) -> String {
    chunks
        .iter()
        .flat_map(|chunk| chunk.iter())
        .map(|byte| (byte ^ key) as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::is_newer_version;

    #[test]
    fn compares_release_versions() {
        assert!(is_newer_version("v0.1.2", "0.1.1"));
        assert!(is_newer_version("v0.2.0", "0.1.9"));
        assert!(!is_newer_version("v0.1.1", "0.1.1"));
        assert!(!is_newer_version("v0.1.0", "0.1.1"));
    }
}
