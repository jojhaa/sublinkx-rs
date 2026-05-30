use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};

use crate::{
    dto::subscriptions::{
        CreateSubscriptionRequest, RenewSubscriptionRequest, SubscriptionListResponse,
        SubscriptionResponse, UpdateSubscriptionRequest,
    },
    errors::AppError,
    services::subscription_service,
    state::AppState,
};

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SubscriptionListResponse>, AppError> {
    subscription_service::require_auth(&state, &headers).await?;
    let response = subscription_service::list_subscriptions(&state).await?;
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
