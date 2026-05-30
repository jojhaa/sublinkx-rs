use axum::{Json, extract::State, http::HeaderMap};

use crate::{
    dto::auth::{ChangeCredentialsRequest, LoginRequest, LoginResponse, MeResponse},
    errors::AppError,
    services::auth_service,
    state::AppState,
};

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let response = auth_service::login(&state, payload).await?;
    Ok(Json(response))
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AppError> {
    let response = auth_service::current_user(&state, &headers).await?;
    Ok(Json(response))
}

pub async fn change_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ChangeCredentialsRequest>,
) -> Result<Json<MeResponse>, AppError> {
    let response = auth_service::change_credentials(&state, &headers, payload).await?;
    Ok(Json(response))
}
