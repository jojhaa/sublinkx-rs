use axum::{Json, extract::State};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct VersionResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub api_version: &'static str,
    pub environment: String,
}

pub async fn version(State(state): State<AppState>) -> Json<VersionResponse> {
    Json(VersionResponse {
        name: "sublinkx-rs-backend",
        version: env!("CARGO_PKG_VERSION"),
        api_version: "v1",
        environment: state.config.server.environment.clone(),
    })
}
