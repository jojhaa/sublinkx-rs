use axum::http::HeaderMap;

use crate::{
    domain::group::GroupView,
    dto::groups::{CreateGroupRequest, GroupListResponse, GroupResponse, UpdateGroupRequest},
    errors::AppError,
    repository::group_repo::{self, GroupTable},
    state::AppState,
    utils::time::now_rfc3339,
};

use super::auth_service;

pub async fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    auth_service::require_user(state, headers).await.map(|_| ())
}

pub async fn list_groups(
    state: &AppState,
    table: GroupTable,
) -> Result<GroupListResponse, AppError> {
    let data = group_repo::list(&state.db, table)
        .await?
        .into_iter()
        .map(GroupView::from)
        .collect();

    Ok(GroupListResponse {
        code: "00000",
        data,
    })
}

pub async fn create_group(
    state: &AppState,
    table: GroupTable,
    payload: CreateGroupRequest,
) -> Result<GroupResponse, AppError> {
    validate_group_name(&payload.name)?;

    if group_repo::find_by_name(&state.db, table, payload.name.trim())
        .await?
        .is_some()
    {
        return Err(AppError::BadRequest(
            "group name already exists".to_string(),
        ));
    }

    let now = now_rfc3339();
    let data = group_repo::insert(
        &state.db,
        table,
        &group_repo::NewGroupRecord {
            name: payload.name.trim(),
            sort_order: payload.sort_order.unwrap_or(0),
            created_at: &now,
            updated_at: &now,
        },
    )
    .await?
    .into();

    Ok(GroupResponse {
        code: "00000",
        data,
    })
}

pub async fn update_group(
    state: &AppState,
    table: GroupTable,
    id: i64,
    payload: UpdateGroupRequest,
) -> Result<GroupResponse, AppError> {
    validate_group_name(&payload.name)?;

    let existing = group_repo::find_by_id(&state.db, table, id)
        .await?
        .ok_or_else(|| AppError::NotFound("group not found".to_string()))?;

    if let Some(duplicate) = group_repo::find_by_name(&state.db, table, payload.name.trim()).await?
        && duplicate.id != id
    {
        return Err(AppError::BadRequest(
            "group name already exists".to_string(),
        ));
    }

    let data = group_repo::update(
        &state.db,
        table,
        id,
        &group_repo::UpdateGroupRecord {
            name: payload.name.trim(),
            sort_order: payload.sort_order.unwrap_or(existing.sort_order),
            updated_at: &now_rfc3339(),
        },
    )
    .await?
    .into();

    Ok(GroupResponse {
        code: "00000",
        data,
    })
}

pub async fn delete_group(state: &AppState, table: GroupTable, id: i64) -> Result<(), AppError> {
    group_repo::find_by_id(&state.db, table, id)
        .await?
        .ok_or_else(|| AppError::NotFound("group not found".to_string()))?;

    if group_repo::count_usage(&state.db, table, id).await? > 0 {
        return Err(AppError::BadRequest(
            "group is still used by records".to_string(),
        ));
    }

    group_repo::delete(&state.db, table, id).await?;
    Ok(())
}

fn validate_group_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    Ok(())
}
