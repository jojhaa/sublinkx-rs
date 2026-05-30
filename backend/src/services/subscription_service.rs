use axum::http::HeaderMap;
use rand::{Rng, distr::Alphanumeric};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    domain::{
        node::NodeView,
        subscription::{SubscriptionRecord, SubscriptionView},
    },
    dto::subscriptions::{
        CreateSubscriptionRequest, RenewSubscriptionRequest, SubscriptionListResponse,
        SubscriptionResponse, UpdateSubscriptionRequest,
    },
    errors::AppError,
    repository::{
        group_repo::{self, GroupTable},
        node_repo, subscription_repo, template_repo,
    },
    state::AppState,
    utils::time::now_rfc3339,
};

use super::auth_service;

pub async fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    auth_service::require_user(state, headers).await.map(|_| ())
}

pub async fn list_subscriptions(state: &AppState) -> Result<SubscriptionListResponse, AppError> {
    let subscriptions = subscription_repo::list(&state.db).await?;
    let mut data = Vec::with_capacity(subscriptions.len());

    for item in subscriptions {
        data.push(build_view(state, item).await?);
    }

    Ok(SubscriptionListResponse {
        code: "00000",
        data,
    })
}

pub async fn get_subscription(state: &AppState, id: i64) -> Result<SubscriptionResponse, AppError> {
    let item = subscription_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("subscription not found".to_string()))?;

    let data = build_view(state, item).await?;
    Ok(SubscriptionResponse {
        code: "00000",
        data,
    })
}

pub async fn get_subscription_by_token(
    state: &AppState,
    token: &str,
) -> Result<SubscriptionView, AppError> {
    let item = subscription_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AppError::NotFound("subscription not found".to_string()))?;

    build_view(state, item).await
}

pub async fn create_subscription(
    state: &AppState,
    payload: CreateSubscriptionRequest,
) -> Result<SubscriptionResponse, AppError> {
    validate_subscription_name(&payload.name)?;

    if subscription_repo::find_by_name(&state.db, payload.name.trim())
        .await?
        .is_some()
    {
        return Err(AppError::BadRequest(
            "subscription name already exists".to_string(),
        ));
    }

    let token = generate_unique_token(state).await?;
    ensure_nodes_exist(state, &payload.node_ids).await?;
    ensure_template_exists(state, payload.template_id).await?;
    ensure_group_exists(state, payload.group_id).await?;
    validate_expires_at(payload.expires_at.as_deref())?;
    ensure_subscription_has_nodes_or_raw_template(state, &payload.node_ids, payload.template_id)
        .await?;

    let now = now_rfc3339();
    let record = subscription_repo::insert(
        &state.db,
        &subscription_repo::NewSubscriptionRecord {
            name: payload.name.trim(),
            token: &token,
            description: payload.description.as_deref().unwrap_or(""),
            default_client: payload.default_client.as_deref(),
            template_id: payload.template_id,
            group_id: payload.group_id,
            enabled: payload.enabled.unwrap_or(true),
            expires_at: payload.expires_at.as_deref(),
            created_at: &now,
            updated_at: &now,
        },
    )
    .await?;

    subscription_repo::replace_subscription_nodes(&state.db, record.id, &payload.node_ids).await?;

    let data = build_view(state, record).await?;
    Ok(SubscriptionResponse {
        code: "00000",
        data,
    })
}

pub async fn update_subscription(
    state: &AppState,
    id: i64,
    payload: UpdateSubscriptionRequest,
) -> Result<SubscriptionResponse, AppError> {
    validate_subscription_name(&payload.name)?;

    let existing = subscription_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("subscription not found".to_string()))?;

    if let Some(duplicate) = subscription_repo::find_by_name(&state.db, payload.name.trim()).await?
        && duplicate.id != id
    {
        return Err(AppError::BadRequest(
            "subscription name already exists".to_string(),
        ));
    }

    ensure_nodes_exist(state, &payload.node_ids).await?;
    ensure_template_exists(state, payload.template_id).await?;
    ensure_group_exists(state, payload.group_id).await?;
    validate_expires_at(payload.expires_at.as_deref())?;
    ensure_subscription_has_nodes_or_raw_template(state, &payload.node_ids, payload.template_id)
        .await?;

    let record = subscription_repo::update(
        &state.db,
        id,
        &subscription_repo::UpdateSubscriptionRecord {
            name: payload.name.trim(),
            token: &existing.token,
            description: payload.description.as_deref().unwrap_or(""),
            default_client: payload.default_client.as_deref(),
            template_id: payload.template_id,
            group_id: payload.group_id,
            enabled: payload.enabled.unwrap_or(existing.enabled),
            expires_at: payload.expires_at.as_deref(),
            updated_at: &now_rfc3339(),
        },
    )
    .await?;

    subscription_repo::replace_subscription_nodes(&state.db, id, &payload.node_ids).await?;

    let data = build_view(state, record).await?;
    Ok(SubscriptionResponse {
        code: "00000",
        data,
    })
}

pub async fn delete_subscription(state: &AppState, id: i64) -> Result<(), AppError> {
    subscription_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("subscription not found".to_string()))?;
    subscription_repo::delete(&state.db, id).await?;
    Ok(())
}

pub async fn rotate_subscription_token(
    state: &AppState,
    id: i64,
) -> Result<SubscriptionResponse, AppError> {
    let existing = subscription_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("subscription not found".to_string()))?;
    let token = generate_unique_token(state).await?;

    let record = subscription_repo::update(
        &state.db,
        id,
        &subscription_repo::UpdateSubscriptionRecord {
            name: &existing.name,
            token: &token,
            description: &existing.description,
            default_client: existing.default_client.as_deref(),
            template_id: existing.template_id,
            group_id: existing.group_id,
            enabled: existing.enabled,
            expires_at: existing.expires_at.as_deref(),
            updated_at: &now_rfc3339(),
        },
    )
    .await?;

    let data = build_view(state, record).await?;
    Ok(SubscriptionResponse {
        code: "00000",
        data,
    })
}

pub async fn renew_subscription(
    state: &AppState,
    id: i64,
    payload: RenewSubscriptionRequest,
) -> Result<SubscriptionResponse, AppError> {
    if payload.days <= 0 || payload.days > 3650 {
        return Err(AppError::BadRequest(
            "renew days must be between 1 and 3650".to_string(),
        ));
    }

    let existing = subscription_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("subscription not found".to_string()))?;
    let base = existing
        .expires_at
        .as_deref()
        .and_then(parse_rfc3339)
        .filter(|value| *value > OffsetDateTime::now_utc())
        .unwrap_or_else(OffsetDateTime::now_utc);
    let expires_at = base
        .saturating_add(Duration::days(payload.days))
        .format(&Rfc3339)
        .map_err(|_| AppError::Internal)?;

    let record = subscription_repo::update(
        &state.db,
        id,
        &subscription_repo::UpdateSubscriptionRecord {
            name: &existing.name,
            token: &existing.token,
            description: &existing.description,
            default_client: existing.default_client.as_deref(),
            template_id: existing.template_id,
            group_id: existing.group_id,
            enabled: true,
            expires_at: Some(&expires_at),
            updated_at: &now_rfc3339(),
        },
    )
    .await?;

    let data = build_view(state, record).await?;
    Ok(SubscriptionResponse {
        code: "00000",
        data,
    })
}

async fn build_view(
    state: &AppState,
    record: SubscriptionRecord,
) -> Result<SubscriptionView, AppError> {
    let status = subscription_status(&record).to_string();
    let relation_rows = subscription_repo::list_subscription_nodes(&state.db, record.id).await?;
    let mut node_ids = Vec::with_capacity(relation_rows.len());
    let mut nodes = Vec::with_capacity(relation_rows.len());

    for row in relation_rows {
        node_ids.push(row.node_id);
        let node = node_repo::find_by_id(&state.db, row.node_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound("subscription references missing node".to_string())
            })?;
        nodes.push(NodeView::try_from(node).map_err(|_| AppError::Internal)?);
    }

    Ok(SubscriptionView {
        id: record.id,
        name: record.name,
        token: record.token,
        description: record.description,
        default_client: record.default_client,
        template_id: record.template_id,
        group_id: record.group_id,
        enabled: record.enabled,
        expires_at: record.expires_at.clone(),
        status,
        node_ids,
        nodes,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

async fn ensure_nodes_exist(state: &AppState, node_ids: &[i64]) -> Result<(), AppError> {
    for node_id in node_ids {
        if node_repo::find_by_id(&state.db, *node_id).await?.is_none() {
            return Err(AppError::BadRequest(format!("node not found: {}", node_id)));
        }
    }
    Ok(())
}

async fn ensure_template_exists(
    state: &AppState,
    template_id: Option<i64>,
) -> Result<(), AppError> {
    if let Some(template_id) = template_id
        && template_repo::find_by_id(&state.db, template_id)
            .await?
            .is_none()
    {
        return Err(AppError::BadRequest(format!(
            "template not found: {}",
            template_id
        )));
    }

    Ok(())
}

async fn ensure_group_exists(state: &AppState, group_id: Option<i64>) -> Result<(), AppError> {
    if let Some(group_id) = group_id
        && group_repo::find_by_id(&state.db, GroupTable::Subscription, group_id)
            .await?
            .is_none()
    {
        return Err(AppError::BadRequest(format!(
            "subscription group not found: {}",
            group_id
        )));
    }

    Ok(())
}

async fn ensure_subscription_has_nodes_or_raw_template(
    state: &AppState,
    node_ids: &[i64],
    template_id: Option<i64>,
) -> Result<(), AppError> {
    if !node_ids.is_empty() {
        return Ok(());
    }

    if let Some(template_id) = template_id
        && let Some(template) = template_repo::find_by_id(&state.db, template_id).await?
        && template
            .content
            .contains("x-sublinkx-upstream-template: true")
    {
        return Ok(());
    }

    Err(AppError::BadRequest(
        "at least one node is required unless an upstream raw template is selected".to_string(),
    ))
}

fn validate_subscription_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    Ok(())
}

pub fn subscription_is_expired(subscription: &SubscriptionView) -> bool {
    subscription
        .expires_at
        .as_deref()
        .and_then(parse_rfc3339)
        .is_some_and(|expires_at| expires_at <= OffsetDateTime::now_utc())
}

fn subscription_status(record: &SubscriptionRecord) -> &'static str {
    if !record.enabled {
        return "disabled";
    }
    if record
        .expires_at
        .as_deref()
        .and_then(parse_rfc3339)
        .is_some_and(|expires_at| expires_at <= OffsetDateTime::now_utc())
    {
        return "expired";
    }
    "active"
}

fn validate_expires_at(expires_at: Option<&str>) -> Result<(), AppError> {
    if let Some(value) = expires_at
        && parse_rfc3339(value).is_none()
    {
        return Err(AppError::BadRequest(
            "expires_at must be RFC3339 datetime".to_string(),
        ));
    }
    Ok(())
}

fn parse_rfc3339(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

async fn generate_unique_token(state: &AppState) -> Result<String, AppError> {
    for _ in 0..10 {
        let token = rand::rng()
            .sample_iter(Alphanumeric)
            .take(32)
            .map(char::from)
            .collect::<String>();

        if subscription_repo::find_by_token(&state.db, &token)
            .await?
            .is_none()
        {
            return Ok(token);
        }
    }

    Err(AppError::Internal)
}
