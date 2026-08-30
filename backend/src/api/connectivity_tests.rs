use axum::{
    Json,
    body::Body,
    extract::State,
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};

use crate::{
    dto::connectivity_tests::{ConnectivityTestPresetResponse, ConnectivityTestRequest},
    errors::AppError,
    services::{connectivity_test_service, node_service},
    state::AppState,
};

pub async fn presets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ConnectivityTestPresetResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    Ok(Json(connectivity_test_service::presets(&state).await?))
}

pub async fn stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConnectivityTestRequest>,
) -> Result<Response, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let body: Body = connectivity_test_service::stream(state, payload).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response())
}

pub async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let cancelled = state.request_cancel_manual_latency_run().await;
    Ok(Json(serde_json::json!({
        "code": "00000",
        "cancelled": cancelled,
    })))
}
