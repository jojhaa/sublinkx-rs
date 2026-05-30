use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};

use crate::{
    dto::templates::{
        CreateTemplateRequest, TemplateListResponse, TemplateResponse, UpdateTemplateRequest,
    },
    errors::AppError,
    services::template_service,
    state::AppState,
};

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TemplateListResponse>, AppError> {
    template_service::require_auth(&state, &headers).await?;
    let response = template_service::list_templates(&state).await?;
    Ok(Json(response))
}

pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<TemplateResponse>, AppError> {
    template_service::require_auth(&state, &headers).await?;
    let response = template_service::get_template(&state, id).await?;
    Ok(Json(response))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<Json<TemplateResponse>, AppError> {
    template_service::require_auth(&state, &headers).await?;
    let response = template_service::create_template(&state, payload).await?;
    Ok(Json(response))
}

pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateTemplateRequest>,
) -> Result<Json<TemplateResponse>, AppError> {
    template_service::require_auth(&state, &headers).await?;
    let response = template_service::update_template(&state, id, payload).await?;
    Ok(Json(response))
}

pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    template_service::require_auth(&state, &headers).await?;
    template_service::delete_template(&state, id).await?;
    Ok(Json(serde_json::json!({
        "code": "00000",
        "message": "template deleted"
    })))
}
