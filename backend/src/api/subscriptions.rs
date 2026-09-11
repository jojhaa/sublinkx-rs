use axum::{
    Json,
    extract::{ConnectInfo, Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
};
use std::net::SocketAddr;

use crate::{
    api::exports::ExportQuery,
    dto::subscriptions::{
        CreateSubscriptionRequest, RenewSubscriptionRequest, SubscriptionListQuery,
        SubscriptionListResponse, SubscriptionPortalResponse, SubscriptionResponse,
        UnlockSubscriptionPortalRequest, UpdateSubscriptionRequest,
    },
    errors::AppError,
    services::{export_service, subscription_service},
    state::AppState,
};

pub async fn unlock_portal(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(portal_slug): Path<String>,
    Json(payload): Json<UnlockSubscriptionPortalRequest>,
) -> Result<Json<SubscriptionPortalResponse>, AppError> {
    let rate_limit_ip = crate::services::auth_service::request_rate_limit_ip(
        &headers,
        Some(peer_addr.ip()),
        state.config.security.trust_proxy_headers,
    );
    let response = subscription_service::unlock_subscription_portal(
        &state,
        &portal_slug,
        &rate_limit_ip,
        &payload.access_code,
    )
    .await?;
    Ok(Json(response))
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SubscriptionListQuery>,
) -> Result<Json<SubscriptionListResponse>, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    let response = subscription_service::list_subscriptions(&state, query).await?;
    Ok(Json(response))
}

pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<SubscriptionResponse>, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    let response = subscription_service::get_subscription(&state, id).await?;
    Ok(Json(response))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> Result<Json<SubscriptionResponse>, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    let response = subscription_service::create_subscription(&state, payload).await?;
    Ok(Json(response))
}

pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateSubscriptionRequest>,
) -> Result<Json<SubscriptionResponse>, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    let response = subscription_service::update_subscription(&state, id, payload).await?;
    Ok(Json(response))
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    subscription_service::delete_subscription(&state, id).await?;
    Ok(Json(serde_json::json!({
        "code": "00000",
        "message": "subscription deleted"
    })))
}

pub async fn rotate_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<SubscriptionResponse>, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    let response = subscription_service::rotate_subscription_token(&state, id).await?;
    Ok(Json(response))
}

pub async fn renew(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<RenewSubscriptionRequest>,
) -> Result<Json<SubscriptionResponse>, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    let response = subscription_service::renew_subscription(&state, id, payload).await?;
    Ok(Json(response))
}

pub async fn export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<ExportQuery>,
) -> Result<impl IntoResponse, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    export_service::export_subscription_by_id(
        &state,
        id,
        query.target.as_deref(),
        query.mode.as_deref(),
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
    )
    .await
}
