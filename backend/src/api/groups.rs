use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};

use crate::{
    dto::groups::{CreateGroupRequest, GroupListResponse, GroupResponse, UpdateGroupRequest},
    errors::AppError,
    repository::group_repo::GroupTable,
    services::group_service,
    state::AppState,
};

pub async fn list_node_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GroupListResponse>, AppError> {
    group_service::require_auth(&state, &headers).await?;
    Ok(Json(
        group_service::list_groups(&state, GroupTable::Node).await?,
    ))
}

pub async fn create_node_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGroupRequest>,
) -> Result<Json<GroupResponse>, AppError> {
    group_service::require_auth(&state, &headers).await?;
    Ok(Json(
        group_service::create_group(&state, GroupTable::Node, payload).await?,
    ))
}

pub async fn update_node_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateGroupRequest>,
) -> Result<Json<GroupResponse>, AppError> {
    group_service::require_auth(&state, &headers).await?;
    Ok(Json(
        group_service::update_group(&state, GroupTable::Node, id, payload).await?,
    ))
}

pub async fn delete_node_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    group_service::require_auth(&state, &headers).await?;
    group_service::delete_group(&state, GroupTable::Node, id).await?;
    Ok(Json(
        serde_json::json!({ "code": "00000", "message": "group deleted" }),
    ))
}

pub async fn list_subscription_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GroupListResponse>, AppError> {
    group_service::require_auth(&state, &headers).await?;
    Ok(Json(
        group_service::list_groups(&state, GroupTable::Subscription).await?,
    ))
}

pub async fn create_subscription_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGroupRequest>,
) -> Result<Json<GroupResponse>, AppError> {
    group_service::require_auth(&state, &headers).await?;
    Ok(Json(
        group_service::create_group(&state, GroupTable::Subscription, payload).await?,
    ))
}

pub async fn update_subscription_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateGroupRequest>,
) -> Result<Json<GroupResponse>, AppError> {
    group_service::require_auth(&state, &headers).await?;
    Ok(Json(
        group_service::update_group(&state, GroupTable::Subscription, id, payload).await?,
    ))
}

pub async fn delete_subscription_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    group_service::require_auth(&state, &headers).await?;
    group_service::delete_group(&state, GroupTable::Subscription, id).await?;
    Ok(Json(
        serde_json::json!({ "code": "00000", "message": "group deleted" }),
    ))
}
