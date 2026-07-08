use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};

use crate::{
    dto::upstream_subscriptions::{
        UpstreamSubscriptionImportResponse, UpstreamSubscriptionListResponse,
        UpstreamSubscriptionPayload, UpstreamSubscriptionResponse,
    },
    errors::AppError,
    services::{auth_service, upstream_subscription_service},
    state::AppState,
};

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UpstreamSubscriptionListResponse>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    Ok(Json(upstream_subscription_service::list(&state).await?))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpstreamSubscriptionPayload>,
) -> Result<Json<UpstreamSubscriptionResponse>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    Ok(Json(
        upstream_subscription_service::create(&state, payload).await?,
    ))
}

pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpstreamSubscriptionPayload>,
) -> Result<Json<UpstreamSubscriptionResponse>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    Ok(Json(
        upstream_subscription_service::update(&state, id, payload).await?,
    ))
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    upstream_subscription_service::delete(&state, id).await?;
    Ok(Json(serde_json::json!({
        "code": "00000",
        "message": "upstream subscription deleted"
    })))
}

pub async fn import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<UpstreamSubscriptionImportResponse>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    Ok(Json(
        upstream_subscription_service::import(&state, id).await?,
    ))
}
