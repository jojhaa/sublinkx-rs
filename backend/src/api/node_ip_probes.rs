use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};

use crate::{
    dto::node_ip_probes::{
        NodeIpIntelligenceStatusResponse, NodeIpProbeListResponse, NodeIpProbeRequest,
        NodeIpProbeResponse, RefreshNodeIpIntelligenceRequest, RefreshNodeIpIntelligenceResponse,
        UpdateNodeIpCountryRequest,
    },
    errors::AppError,
    services::{node_ip_probe_service, node_service},
    state::AppState,
};

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<NodeIpProbeListResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    Ok(Json(node_ip_probe_service::list(&state).await?))
}

pub async fn stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<NodeIpProbeRequest>,
) -> Result<Response, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let body: Body = node_ip_probe_service::stream(state, payload).await?;
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

pub async fn intelligence_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<NodeIpIntelligenceStatusResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    let config = &state.config.ip_intelligence;
    Ok(Json(NodeIpIntelligenceStatusResponse {
        code: "00000",
        enabled: config.enabled,
        base_url: config.enabled.then(|| config.base_url.clone()),
        source_key: config.enabled.then(|| config.source_key.clone()),
    }))
}

pub async fn refresh_intelligence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RefreshNodeIpIntelligenceRequest>,
) -> Result<Json<RefreshNodeIpIntelligenceResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    Ok(Json(
        node_ip_probe_service::refresh_intelligence(&state, payload).await?,
    ))
}

pub async fn update_country(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateNodeIpCountryRequest>,
) -> Result<Json<NodeIpProbeResponse>, AppError> {
    node_service::require_auth(&state, &headers).await?;
    Ok(Json(
        node_ip_probe_service::update_country(&state, id, payload).await?,
    ))
}
