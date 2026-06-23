use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
};
use std::net::SocketAddr;

use crate::{errors::AppError, services::export_service, state::AppState};

#[derive(Debug, serde::Deserialize)]
pub struct ExportQuery {
    pub target: Option<String>,
    pub mode: Option<String>,
}

pub async fn get_subscription(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Path(token): Path<String>,
    Query(query): Query<ExportQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    export_service::export_subscription(
        &state,
        &token,
        query.target.as_deref(),
        query.mode.as_deref(),
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        &headers,
        Some(peer_addr.ip()),
    )
    .await
}
