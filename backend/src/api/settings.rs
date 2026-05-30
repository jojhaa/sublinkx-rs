use axum::{Json, extract::State, http::HeaderMap};

use crate::{
    dto::settings::{
        MihomoCoreDownloadResponse, MihomoCoreStatusResponse, SettingsResponse,
        UpdateSettingsRequest,
    },
    errors::AppError,
    services::{mihomo_core_service, settings_service},
    state::AppState,
};

pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, AppError> {
    settings_service::require_auth(&state, &headers).await?;
    let response = settings_service::get_settings(&state).await?;
    Ok(Json(response))
}

pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, AppError> {
    settings_service::require_auth(&state, &headers).await?;
    let response = settings_service::update_settings(&state, payload).await?;
    Ok(Json(response))
}

pub async fn mihomo_core_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MihomoCoreStatusResponse>, AppError> {
    settings_service::require_auth(&state, &headers).await?;
    let response = mihomo_core_service::get_status(&state).await?;
    Ok(Json(response))
}

pub async fn download_mihomo_core(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MihomoCoreDownloadResponse>, AppError> {
    settings_service::require_auth(&state, &headers).await?;
    let response = mihomo_core_service::download_latest(&state).await?;
    Ok(Json(response))
}
