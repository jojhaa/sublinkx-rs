use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};

use crate::{
    dto::upstream_subscriptions::{
        DeleteUpstreamSubscriptionQuery, DeleteUpstreamSubscriptionResponse,
        UpstreamSubscriptionImportResponse, UpstreamSubscriptionListQuery,
        UpstreamSubscriptionListResponse, UpstreamSubscriptionPayload,
        UpstreamSubscriptionResponse,
    },
    errors::AppError,
    services::{auth_service, upstream_subscription_service},
    state::AppState,
};

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<UpstreamSubscriptionListQuery>,
) -> Result<Json<UpstreamSubscriptionListResponse>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    Ok(Json(
        upstream_subscription_service::list(&state, query).await?,
    ))
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
    Query(query): Query<DeleteUpstreamSubscriptionQuery>,
) -> Result<Json<DeleteUpstreamSubscriptionResponse>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    let (deleted_nodes, detached_nodes) =
        upstream_subscription_service::delete(&state, id, query.delete_nodes).await?;
    Ok(Json(DeleteUpstreamSubscriptionResponse {
        code: "00000",
        message: "upstream subscription deleted",
        deleted_nodes,
        detached_nodes,
    }))
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
