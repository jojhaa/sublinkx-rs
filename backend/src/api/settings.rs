use axum::{Json, extract::State, http::HeaderMap};

use crate::{
    dto::settings::{SettingsResponse, UpdateSettingsRequest},
    errors::AppError,
    services::settings_service,
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
