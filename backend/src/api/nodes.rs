use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};

use crate::{
    dto::nodes::{
        CreateNodeRequest, ImportNodesFromSubscriptionRequest, NodeImportResponse,
        NodeLatencyBatchRequest, NodeLatencyBatchResponse, NodeLatencyResponse, NodeListResponse,
        NodeResponse, UpdateNodeRequest,
    },
    errors::AppError,
    services::node_service,
    state::AppState,
};

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<NodeListResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let response = node_service::list_nodes(&state).await?;
    Ok(Json(response))
}

pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<NodeResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let response = node_service::get_node(&state, id).await?;
    Ok(Json(response))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateNodeRequest>,
) -> Result<Json<NodeResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let response = node_service::create_node(&state, payload).await?;
    Ok(Json(response))
}

pub async fn import_from_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ImportNodesFromSubscriptionRequest>,
) -> Result<Json<NodeImportResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let response = node_service::import_nodes_from_subscription(&state, payload).await?;
    Ok(Json(response))
}

pub async fn test_latency(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<NodeLatencyResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let response = node_service::test_node_latency(&state, id).await?;
    Ok(Json(response))
}

pub async fn test_latency_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<NodeLatencyBatchRequest>,
) -> Result<Json<NodeLatencyBatchResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let response = node_service::test_node_latency_batch(&state, payload).await?;
    Ok(Json(response))
}

pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateNodeRequest>,
) -> Result<Json<NodeResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let response = node_service::update_node(&state, id, payload).await?;
    Ok(Json(response))
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    node_service::delete_node(&state, id).await?;
    Ok(Json(serde_json::json!({
        "code": "00000",
        "message": "node deleted"
    })))
}
