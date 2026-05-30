use axum::{Json, extract::State, http::HeaderMap};

use crate::{
    dto::auth::{LoginRequest, LoginResponse, MeResponse},
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
