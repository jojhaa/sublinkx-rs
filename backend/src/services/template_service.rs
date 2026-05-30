use axum::http::HeaderMap;

use crate::{
    domain::{client::is_known_template_kind, template::TemplateView},
    dto::templates::{
        CreateTemplateRequest, TemplateListResponse, TemplateResponse, UpdateTemplateRequest,
    },
    errors::AppError,
    repository::{subscription_repo, template_repo},
    state::AppState,
    utils::time::now_rfc3339,
};

use super::auth_service;

pub async fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    auth_service::require_user(state, headers).await.map(|_| ())
}

pub async fn list_templates(state: &AppState) -> Result<TemplateListResponse, AppError> {
    let data = template_repo::list(&state.db)
        .await?
        .into_iter()
        .map(TemplateView::from)
        .collect();

    Ok(TemplateListResponse {
        code: "00000",
        data,
    })
}

pub async fn get_template(state: &AppState, id: i64) -> Result<TemplateResponse, AppError> {
    let data = template_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("template not found".to_string()))?
        .into();

    Ok(TemplateResponse {
        code: "00000",
        data,
    })
}

pub async fn create_template(
    state: &AppState,
    payload: CreateTemplateRequest,
) -> Result<TemplateResponse, AppError> {
    validate_template_input(&payload.name, &payload.kind, &payload.content)?;

    if template_repo::find_by_name(&state.db, payload.name.trim())
        .await?
        .is_some()
    {
        return Err(AppError::BadRequest(
            "template name already exists".to_string(),
        ));
    }

    let now = now_rfc3339();
    let data = template_repo::insert(
        &state.db,
        &template_repo::NewTemplateRecord {
            name: payload.name.trim(),
            kind: payload.kind.trim(),
            content: payload.content.trim(),
            created_at: &now,
            updated_at: &now,
        },
    )
    .await?
    .into();

    Ok(TemplateResponse {
        code: "00000",
        data,
    })
}

pub async fn update_template(
    state: &AppState,
    id: i64,
    payload: UpdateTemplateRequest,
) -> Result<TemplateResponse, AppError> {
    validate_template_input(&payload.name, &payload.kind, &payload.content)?;

    template_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("template not found".to_string()))?;

    if let Some(duplicate) = template_repo::find_by_name(&state.db, payload.name.trim()).await?
        && duplicate.id != id
    {
        return Err(AppError::BadRequest(
            "template name already exists".to_string(),
        ));
    }

    let data = template_repo::update(
        &state.db,
        id,
        &template_repo::UpdateTemplateRecord {
            name: payload.name.trim(),
            kind: payload.kind.trim(),
            content: payload.content.trim(),
            updated_at: &now_rfc3339(),
        },
    )
    .await?
    .into();

    Ok(TemplateResponse {
        code: "00000",
        data,
    })
}

pub async fn delete_template(state: &AppState, id: i64) -> Result<(), AppError> {
    template_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("template not found".to_string()))?;

    if subscription_repo::count_by_template_id(&state.db, id).await? > 0 {
        return Err(AppError::BadRequest(
            "template is still used by subscriptions".to_string(),
        ));
    }

    template_repo::delete(&state.db, id).await?;
    Ok(())
}

fn validate_template_input(name: &str, kind: &str, content: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }

    if content.trim().is_empty() {
        return Err(AppError::BadRequest("content is required".to_string()));
    }

    let normalized_kind = kind.trim();
    if normalized_kind.is_empty() {
        return Err(AppError::BadRequest("kind is required".to_string()));
    }

    if !is_known_template_kind(normalized_kind) {
        return Err(AppError::BadRequest(format!(
            "unsupported template kind: {}",
            normalized_kind
        )));
    }

    Ok(())
}
