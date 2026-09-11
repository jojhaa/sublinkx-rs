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

pub async fn get_subscription_profile(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Path((token, target)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let target = full_profile_target(&target)
        .ok_or_else(|| AppError::NotFound("subscription profile not found".to_string()))?;

    export_service::export_subscription(
        &state,
        &token,
        Some(target),
        Some("best_effort"),
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        &headers,
        Some(peer_addr.ip()),
    )
    .await
}

fn full_profile_target(target: &str) -> Option<&'static str> {
    match target {
        "quanx" => Some("quanx"),
        "shadowrocket-profile" => Some("shadowrocket-profile"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::full_profile_target;

    #[test]
    fn full_profile_route_only_accepts_complete_configuration_targets() {
        assert_eq!(full_profile_target("quanx"), Some("quanx"));
        assert_eq!(
            full_profile_target("shadowrocket-profile"),
            Some("shadowrocket-profile")
        );
        assert_eq!(full_profile_target("shadowrocket"), None);
        assert_eq!(full_profile_target("mihomo"), None);
    }
}
