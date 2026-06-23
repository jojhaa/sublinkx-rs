use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;

use crate::{
    dto::auth::{ChangeCredentialsRequest, LoginRequest, LogoutResponse, MeResponse},
    errors::AppError,
    services::auth_service,
    state::AppState,
};

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Response, AppError> {
    let outcome = auth_service::login(&state, &headers, Some(peer_addr.ip()), payload).await?;
    let mut response = Json(outcome.response).into_response();
    auth_service::append_session_headers(
        response.headers_mut(),
        &outcome.session,
        state.config.security.auth_cookie_secure,
    )?;
    Ok(response)
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
) -> Result<Response, AppError> {
    let outcome = auth_service::change_credentials(&state, &headers, payload).await?;
    let mut response = Json(outcome.response).into_response();
    auth_service::append_session_headers(
        response.headers_mut(),
        &outcome.session,
        state.config.security.auth_cookie_secure,
    )?;
    Ok(response)
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    auth_service::require_user_for_credentials_change(&state, &headers).await?;
    let mut response = Json(LogoutResponse { code: "00000" }).into_response();
    auth_service::append_logout_headers(
        response.headers_mut(),
        state.config.security.auth_cookie_secure,
    )?;
    Ok(response)
}
