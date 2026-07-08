use url::Url;

use crate::{
    dto::{
        nodes::ImportNodesFromSubscriptionRequest,
        upstream_subscriptions::{
            UpstreamSubscriptionImportResponse, UpstreamSubscriptionListResponse,
            UpstreamSubscriptionPayload, UpstreamSubscriptionResponse,
        },
    },
    errors::AppError,
    repository::{
        group_repo::{self, GroupTable},
        upstream_subscription_repo::{
            self, ImportResultRecord, NewUpstreamSubscriptionRecord,
            UpdateUpstreamSubscriptionRecord,
        },
    },
    state::AppState,
    utils::time::now_rfc3339,
};

use super::node_service;

pub async fn list(state: &AppState) -> Result<UpstreamSubscriptionListResponse, AppError> {
    backfill_existing_node_source_refs(state).await?;
    let records = upstream_subscription_repo::list(&state.db).await?;
    Ok(UpstreamSubscriptionListResponse {
        code: "00000",
        data: records.into_iter().map(Into::into).collect(),
    })
}

pub async fn create(
    state: &AppState,
    payload: UpstreamSubscriptionPayload,
) -> Result<UpstreamSubscriptionResponse, AppError> {
    let name = validate_name(&payload.name)?;
    let url = validate_url(&payload.url)?;
    ensure_group_exists(state, payload.group_id).await?;
    if upstream_subscription_repo::find_by_url(&state.db, &url)
        .await?
        .is_some()
    {
        return Err(AppError::BadRequest(
            "upstream subscription url already exists".to_string(),
        ));
    }

    let now = now_rfc3339();
    let record = upstream_subscription_repo::insert(
        &state.db,
        &NewUpstreamSubscriptionRecord {
            name: &name,
            url: &url,
            group_id: payload.group_id,
            enabled: bool_to_db(payload.enabled.unwrap_or(true)),
            remark: payload.remark.as_deref().unwrap_or("").trim(),
            created_at: &now,
            updated_at: &now,
        },
    )
    .await?;

    Ok(UpstreamSubscriptionResponse {
        code: "00000",
        data: record.into(),
    })
}

pub async fn update(
    state: &AppState,
    id: i64,
    payload: UpstreamSubscriptionPayload,
) -> Result<UpstreamSubscriptionResponse, AppError> {
    upstream_subscription_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("upstream subscription not found".to_string()))?;
    let name = validate_name(&payload.name)?;
    let url = validate_url(&payload.url)?;
    ensure_group_exists(state, payload.group_id).await?;
    if let Some(duplicate) = upstream_subscription_repo::find_by_url(&state.db, &url).await?
        && duplicate.id != id
    {
        return Err(AppError::BadRequest(
            "upstream subscription url already exists".to_string(),
        ));
    }

    let now = now_rfc3339();
    let record = upstream_subscription_repo::update(
        &state.db,
        id,
        &UpdateUpstreamSubscriptionRecord {
            name: &name,
            url: &url,
            group_id: payload.group_id,
            enabled: bool_to_db(payload.enabled.unwrap_or(true)),
            remark: payload.remark.as_deref().unwrap_or("").trim(),
            updated_at: &now,
        },
    )
    .await?;

    Ok(UpstreamSubscriptionResponse {
        code: "00000",
        data: record.into(),
    })
}

pub async fn delete(state: &AppState, id: i64) -> Result<(), AppError> {
    upstream_subscription_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("upstream subscription not found".to_string()))?;
    upstream_subscription_repo::delete(&state.db, id).await?;
    Ok(())
}

pub async fn import(
    state: &AppState,
    id: i64,
) -> Result<UpstreamSubscriptionImportResponse, AppError> {
    let record = upstream_subscription_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("upstream subscription not found".to_string()))?;

    let import_result = node_service::import_nodes_from_subscription(
        state,
        ImportNodesFromSubscriptionRequest {
            url: record.url.clone(),
            group_id: record.group_id,
            remark: if record.remark.trim().is_empty() {
                None
            } else {
                Some(record.remark.clone())
            },
        },
    )
    .await;

    match import_result {
        Ok(import) => {
            let refreshed = upstream_subscription_repo::find_by_id(&state.db, id)
                .await?
                .ok_or_else(|| AppError::NotFound("upstream subscription not found".to_string()))?;
            Ok(UpstreamSubscriptionImportResponse {
                code: "00000",
                data: refreshed.into(),
                import,
            })
        }
        Err(error) => {
            let now = now_rfc3339();
            let message = error.to_string();
            let _ = upstream_subscription_repo::update_import_result(
                &state.db,
                id,
                &ImportResultRecord {
                    status: "error",
                    message: &message,
                    imported: 0,
                    skipped: 0,
                    failed: 0,
                    template_id: record.template_id,
                    template_name: record.template_name.as_deref(),
                    imported_at: &now,
                },
            )
            .await;
            Err(error)
        }
    }
}

async fn ensure_group_exists(state: &AppState, group_id: Option<i64>) -> Result<(), AppError> {
    if let Some(group_id) = group_id
        && group_repo::find_by_id(&state.db, GroupTable::Node, group_id)
            .await?
            .is_none()
    {
        return Err(AppError::BadRequest(format!(
            "node group not found: {}",
            group_id
        )));
    }

    Ok(())
}

fn validate_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "upstream subscription name is required".to_string(),
        ));
    }
    if name.chars().count() > 191 {
        return Err(AppError::BadRequest(
            "upstream subscription name is too long".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn validate_url(url: &str) -> Result<String, AppError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(AppError::BadRequest(
            "upstream subscription url is required".to_string(),
        ));
    }
    if url.len() > 2048 {
        return Err(AppError::BadRequest(
            "upstream subscription url is too long".to_string(),
        ));
    }
    let parsed = Url::parse(url)
        .map_err(|_| AppError::BadRequest("upstream subscription url is invalid".to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(
            "upstream subscription url must be http or https".to_string(),
        ));
    }
    Ok(url.to_string())
}

fn bool_to_db(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

async fn backfill_existing_node_source_refs(state: &AppState) -> Result<(), AppError> {
    let now = now_rfc3339();
    for source in upstream_subscription_repo::list_existing_node_source_refs(&state.db).await? {
        if upstream_subscription_repo::find_by_url(&state.db, &source.url)
            .await?
            .is_some()
        {
            continue;
        }
        let name = default_name_from_url(&source.url);
        let message = format!("discovered from {} imported nodes", source.node_count);
        upstream_subscription_repo::upsert_import_result_by_url(
            &state.db,
            &name,
            &source.url,
            source.group_id,
            "",
            &ImportResultRecord {
                status: "ok",
                message: &message,
                imported: source.node_count,
                skipped: 0,
                failed: 0,
                template_id: None,
                template_name: None,
                imported_at: &now,
            },
        )
        .await?;
    }

    Ok(())
}

pub fn default_name_from_url(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|parsed| {
            let host = parsed.host_str()?;
            let path = parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .filter(|value| !value.is_empty())
                .unwrap_or("subscription");
            Some(format!("{} {}", host, path))
        })
        .unwrap_or_else(|| "Upstream Subscription".to_string())
}
