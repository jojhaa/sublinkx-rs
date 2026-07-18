use axum::{
    body::{Body, Bytes},
    http::HeaderMap,
};
use base64::{Engine as _, engine::general_purpose};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    fs,
    net::TcpListener,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{process::Command, sync::mpsc, task::JoinSet, time::sleep};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    domain::node::NodeView,
    dto::nodes::{
        CreateNodeRequest, ImportNodesFromSubscriptionRequest, MoveNodesRequest,
        NodeFidelityWarning, NodeImportFailure, NodeImportResponse, NodeLatencyBatchRequest,
        NodeLatencyBatchResponse, NodeLatencyResponse, NodeLatencyResult, NodeListResponse,
        NodeResponse, UpdateNodeRequest,
    },
    errors::AppError,
    repository::{
        group_repo::{self, GroupTable},
        node_repo::{self, NewNodeRecord, UpdateNodeRecord},
        template_repo,
        upstream_subscription_repo::{self, ImportResultRecord},
    },
    state::{AppState, LatencyManualBeginError},
    utils::time::now_rfc3339,
};

use super::{
    auth_service, export_service, mihomo_core_service, protocol_parser_service, settings_service,
    upstream_subscription_service, url_safety,
};

const MAX_SUBSCRIPTION_BODY_BYTES: usize = 5 * 1024 * 1024;
const MAX_IMPORT_SUBSCRIPTION_NODES: usize = 1000;
const MAX_LATENCY_BATCH_SIZE: usize = 50;
const LATENCY_RUN_COOLDOWN: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionImportMode {
    AddOnly,
    SyncOverwrite,
}

pub async fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    auth_service::require_user(state, headers).await.map(|_| ())
}

pub async fn list_nodes(state: &AppState) -> Result<NodeListResponse, AppError> {
    let records = node_repo::list(&state.db).await?;
    let data = records
        .into_iter()
        .map(NodeView::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::Internal)?;

    Ok(NodeListResponse {
        code: "00000",
        data,
    })
}

pub async fn get_node(state: &AppState, id: i64) -> Result<NodeResponse, AppError> {
    let record = node_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("node not found".to_string()))?;
    let data = NodeView::try_from(record).map_err(|_| AppError::Internal)?;

    Ok(NodeResponse {
        code: "00000",
        data,
    })
}

pub async fn create_node(
    state: &AppState,
    payload: CreateNodeRequest,
) -> Result<NodeResponse, AppError> {
    if payload.raw_link.trim().is_empty() {
        return Err(AppError::BadRequest("raw_link is required".to_string()));
    }

    let parsed =
        protocol_parser_service::parse_raw_link(&payload.raw_link, payload.name.as_deref())?;
    ensure_group_exists(state, payload.group_id).await?;
    if node_repo::find_by_fingerprint_in_group(&state.db, &parsed.fingerprint, payload.group_id)
        .await?
        .is_some()
    {
        return Err(AppError::BadRequest(
            "node already exists in this group".to_string(),
        ));
    }

    let now = now_rfc3339();
    let settings_json = serde_json::to_string(&parsed.settings).map_err(|_| AppError::Internal)?;
    let record = node_repo::insert(
        &state.db,
        &NewNodeRecord {
            name: &parsed.name,
            protocol: parsed.protocol.as_str(),
            raw_link: payload.raw_link.trim(),
            server: &parsed.server,
            port: i64::from(parsed.port),
            enabled: bool_to_db(true),
            group_id: payload.group_id,
            fingerprint_scope: node_repo::fingerprint_scope(payload.group_id),
            source_type: "manual",
            source_ref: None,
            upstream_missing: bool_to_db(false),
            fingerprint: &parsed.fingerprint,
            settings_json: &settings_json,
            remark: payload.remark.as_deref().unwrap_or(""),
            created_at: &now,
            updated_at: &now,
        },
    )
    .await?;

    let data = NodeView::try_from(record).map_err(|_| AppError::Internal)?;
    Ok(NodeResponse {
        code: "00000",
        data,
    })
}

pub async fn import_nodes_from_subscription(
    state: &AppState,
    payload: ImportNodesFromSubscriptionRequest,
) -> Result<NodeImportResponse, AppError> {
    import_nodes_from_subscription_with_mode(state, payload, SubscriptionImportMode::AddOnly).await
}

pub async fn sync_nodes_from_subscription(
    state: &AppState,
    payload: ImportNodesFromSubscriptionRequest,
) -> Result<NodeImportResponse, AppError> {
    import_nodes_from_subscription_with_mode(state, payload, SubscriptionImportMode::SyncOverwrite)
        .await
}

async fn import_nodes_from_subscription_with_mode(
    state: &AppState,
    payload: ImportNodesFromSubscriptionRequest,
    mode: SubscriptionImportMode,
) -> Result<NodeImportResponse, AppError> {
    let url = payload.url.trim();
    if url.is_empty() {
        return Err(AppError::BadRequest(
            "subscription url is required".to_string(),
        ));
    }
    ensure_group_exists(state, payload.group_id).await?;

    let body = fetch_subscription_body(url).await?;
    let fidelity_warnings = check_mihomo_conversion_fidelity(&body);
    let saved_template =
        save_upstream_template_if_mihomo_yaml(state, url, &body, payload.group_id).await?;
    let raw_links = extract_subscription_links(&body)?;
    if raw_links.is_empty() {
        return Err(AppError::BadRequest(
            "no supported node links found in subscription".to_string(),
        ));
    }
    if raw_links.len() > MAX_IMPORT_SUBSCRIPTION_NODES {
        return Err(AppError::BadRequest(format!(
            "at most {MAX_IMPORT_SUBSCRIPTION_NODES} nodes can be imported at once"
        )));
    }

    let now = now_rfc3339();
    let mut imported = Vec::new();
    let mut failures = Vec::new();
    let mut skipped = 0usize;
    let mut imported_count = 0usize;
    let mut updated_count = 0usize;
    let mut disabled_count = 0usize;
    let mut parsed_links = Vec::new();
    let mut seen_fingerprints = HashSet::new();
    let mut name_counts = HashMap::<String, usize>::new();

    for raw_link in raw_links {
        match protocol_parser_service::parse_raw_link(&raw_link, None) {
            Ok(parsed) => {
                if is_subscription_info_name(&parsed.name) {
                    skipped += 1;
                    continue;
                }

                if !seen_fingerprints.insert(parsed.fingerprint.clone()) {
                    skipped += 1;
                    continue;
                }

                *name_counts.entry(parsed.name.clone()).or_default() += 1;
                parsed_links.push((raw_link, parsed));
            }
            Err(error) => failures.push(NodeImportFailure {
                source: truncate_source(&raw_link),
                reason: error.to_string(),
            }),
        }
    }

    let fingerprints = parsed_links
        .iter()
        .map(|(_, parsed)| parsed.fingerprint.clone())
        .collect::<Vec<_>>();
    let existing_fingerprints =
        node_repo::existing_fingerprints_in_group(&state.db, &fingerprints, payload.group_id)
            .await?;

    for (raw_link, parsed) in parsed_links {
        if mode == SubscriptionImportMode::SyncOverwrite
            && let Some(existing) = find_existing_upstream_node_for_sync(
                state,
                url,
                &parsed.name,
                &parsed.fingerprint,
                name_counts.get(&parsed.name).copied().unwrap_or_default() == 1,
                &existing_fingerprints,
            )
            .await?
        {
            let settings_json =
                serde_json::to_string(&parsed.settings).map_err(|_| AppError::Internal)?;
            match node_repo::update(
                &state.db,
                existing.id,
                &UpdateNodeRecord {
                    name: &parsed.name,
                    protocol: parsed.protocol.as_str(),
                    raw_link: raw_link.trim(),
                    server: &parsed.server,
                    port: i64::from(parsed.port),
                    enabled: if existing.upstream_missing != 0 {
                        bool_to_db(true)
                    } else {
                        existing.enabled
                    },
                    group_id: payload.group_id,
                    fingerprint_scope: node_repo::fingerprint_scope(payload.group_id),
                    upstream_missing: bool_to_db(false),
                    fingerprint: &parsed.fingerprint,
                    settings_json: &settings_json,
                    remark: payload.remark.as_deref().unwrap_or(&existing.remark),
                    updated_at: &now,
                },
            )
            .await
            {
                Ok(record) => {
                    let view = NodeView::try_from(record).map_err(|_| AppError::Internal)?;
                    updated_count += 1;
                    imported.push(view);
                }
                Err(error) => failures.push(NodeImportFailure {
                    source: truncate_source(&raw_link),
                    reason: error.to_string(),
                }),
            }
            continue;
        }

        if existing_fingerprints.contains(&parsed.fingerprint) {
            skipped += 1;
            continue;
        }

        let settings_json =
            serde_json::to_string(&parsed.settings).map_err(|_| AppError::Internal)?;
        match node_repo::insert(
            &state.db,
            &NewNodeRecord {
                name: &parsed.name,
                protocol: parsed.protocol.as_str(),
                raw_link: raw_link.trim(),
                server: &parsed.server,
                port: i64::from(parsed.port),
                enabled: bool_to_db(true),
                group_id: payload.group_id,
                fingerprint_scope: node_repo::fingerprint_scope(payload.group_id),
                source_type: "upstream_subscription",
                source_ref: Some(url),
                upstream_missing: bool_to_db(false),
                fingerprint: &parsed.fingerprint,
                settings_json: &settings_json,
                remark: payload.remark.as_deref().unwrap_or(""),
                created_at: &now,
                updated_at: &now,
            },
        )
        .await
        {
            Ok(record) => {
                let view = NodeView::try_from(record).map_err(|_| AppError::Internal)?;
                imported_count += 1;
                imported.push(view);
            }
            Err(error) => failures.push(NodeImportFailure {
                source: truncate_source(&raw_link),
                reason: error.to_string(),
            }),
        }
    }

    if mode == SubscriptionImportMode::SyncOverwrite && failures.is_empty() {
        disabled_count =
            node_repo::disable_stale_upstream_nodes(&state.db, url, &fingerprints, &now).await?
                as usize;
    }

    let response = NodeImportResponse {
        code: "00000",
        imported: imported_count,
        updated: updated_count,
        disabled: disabled_count,
        skipped,
        failed: failures.len(),
        template_id: saved_template.as_ref().map(|template| template.id),
        template_name: saved_template
            .as_ref()
            .map(|template| template.name.clone()),
        fidelity_warnings,
        data: imported,
        failures,
    };

    remember_upstream_subscription(
        state,
        url,
        payload.group_id,
        payload.remark.as_deref(),
        &response,
    )
    .await?;

    Ok(response)
}

async fn find_existing_upstream_node_for_sync(
    state: &AppState,
    url: &str,
    name: &str,
    fingerprint: &str,
    incoming_name_is_unique: bool,
    existing_fingerprints: &HashSet<String>,
) -> Result<Option<crate::domain::node::NodeRecord>, AppError> {
    if let Some(existing) =
        node_repo::find_upstream_by_fingerprint(&state.db, url, fingerprint).await?
    {
        return Ok(Some(existing));
    }

    if !incoming_name_is_unique || existing_fingerprints.contains(fingerprint) {
        return Ok(None);
    }

    node_repo::find_unique_upstream_by_name(&state.db, url, name)
        .await
        .map_err(Into::into)
}

async fn remember_upstream_subscription(
    state: &AppState,
    url: &str,
    group_id: Option<i64>,
    remark: Option<&str>,
    response: &NodeImportResponse,
) -> Result<(), AppError> {
    let now = now_rfc3339();
    let status = if response.failed == 0 {
        "ok"
    } else if response.imported > 0
        || response.updated > 0
        || response.disabled > 0
        || response.skipped > 0
    {
        "partial"
    } else {
        "error"
    };
    let message = format!(
        "imported {}, updated {}, disabled {}, skipped {}, failed {}",
        response.imported, response.updated, response.disabled, response.skipped, response.failed
    );
    let name = upstream_subscription_service::default_name_from_url(url);
    upstream_subscription_repo::upsert_import_result_by_url(
        &state.db,
        &name,
        url,
        group_id,
        remark.unwrap_or("").trim(),
        &ImportResultRecord {
            status,
            message: &message,
            imported: response.imported as i64,
            updated: response.updated as i64,
            disabled: response.disabled as i64,
            skipped: response.skipped as i64,
            failed: response.failed as i64,
            template_id: response.template_id,
            template_name: response.template_name.as_deref(),
            imported_at: &now,
        },
    )
    .await?;

    Ok(())
}

pub async fn update_node(
    state: &AppState,
    id: i64,
    payload: UpdateNodeRequest,
) -> Result<NodeResponse, AppError> {
    let existing = node_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("node not found".to_string()))?;
    let parsed =
        protocol_parser_service::parse_raw_link(&payload.raw_link, payload.name.as_deref())?;
    ensure_group_exists(state, payload.group_id).await?;

    if let Some(duplicate) =
        node_repo::find_by_fingerprint_in_group(&state.db, &parsed.fingerprint, payload.group_id)
            .await?
        && duplicate.id != id
    {
        return Err(AppError::BadRequest(
            "node already exists in this group".to_string(),
        ));
    }

    let settings_json = serde_json::to_string(&parsed.settings).map_err(|_| AppError::Internal)?;
    let record = node_repo::update(
        &state.db,
        id,
        &UpdateNodeRecord {
            name: &parsed.name,
            protocol: parsed.protocol.as_str(),
            raw_link: payload.raw_link.trim(),
            server: &parsed.server,
            port: i64::from(parsed.port),
            enabled: payload.enabled.map(bool_to_db).unwrap_or(existing.enabled),
            group_id: payload.group_id,
            fingerprint_scope: node_repo::fingerprint_scope(payload.group_id),
            upstream_missing: bool_to_db(false),
            fingerprint: &parsed.fingerprint,
            settings_json: &settings_json,
            remark: payload.remark.as_deref().unwrap_or(&existing.remark),
            updated_at: &now_rfc3339(),
        },
    )
    .await?;

    let data = NodeView::try_from(record).map_err(|_| AppError::Internal)?;
    Ok(NodeResponse {
        code: "00000",
        data,
    })
}

pub async fn test_node_latency(state: &AppState, id: i64) -> Result<NodeLatencyResponse, AppError> {
    begin_latency_run(state).await?;
    let result = test_node_latency_inner(state, id).await;
    state.finish_manual_latency_run().await;
    result
}

async fn test_node_latency_inner(
    state: &AppState,
    id: i64,
) -> Result<NodeLatencyResponse, AppError> {
    let node = node_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("node not found".to_string()))?;
    let settings = settings_service::load_settings(state).await?;
    ensure_mihomo_core_ready(&settings).await?;
    let data = real_latency_for_node(state, &node, &settings).await;
    persist_latency_result(state, &data).await?;
    Ok(NodeLatencyResponse {
        code: "00000",
        data,
    })
}

pub async fn move_nodes(
    state: &AppState,
    payload: MoveNodesRequest,
) -> Result<NodeListResponse, AppError> {
    if payload.ids.is_empty() {
        return Ok(NodeListResponse {
            code: "00000",
            data: Vec::new(),
        });
    }
    if payload.ids.len() > 500 {
        return Err(AppError::BadRequest(
            "at most 500 nodes can be moved at once".to_string(),
        ));
    }

    ensure_group_exists(state, payload.group_id).await?;
    let mut ids = payload.ids;
    ids.sort_unstable();
    ids.dedup();

    let records =
        node_repo::update_group_for_ids(&state.db, &ids, payload.group_id, &now_rfc3339()).await?;
    let data = records
        .into_iter()
        .map(NodeView::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::Internal)?;

    Ok(NodeListResponse {
        code: "00000",
        data,
    })
}

pub async fn test_node_latency_batch(
    state: &AppState,
    payload: NodeLatencyBatchRequest,
) -> Result<NodeLatencyBatchResponse, AppError> {
    begin_latency_run(state).await?;
    let result = test_node_latency_batch_inner(state, payload).await;
    state.finish_manual_latency_run().await;
    result
}

async fn test_node_latency_batch_inner(
    state: &AppState,
    payload: NodeLatencyBatchRequest,
) -> Result<NodeLatencyBatchResponse, AppError> {
    if payload.ids.is_empty() {
        return Ok(NodeLatencyBatchResponse {
            code: "00000",
            data: Vec::new(),
        });
    }
    if payload.ids.len() > MAX_LATENCY_BATCH_SIZE {
        return Err(AppError::BadRequest(format!(
            "at most {MAX_LATENCY_BATCH_SIZE} nodes can be tested at once"
        )));
    }

    let settings = settings_service::load_settings(state).await?;
    ensure_mihomo_core_ready(&settings).await?;
    let mut prepared = Vec::with_capacity(payload.ids.len());
    for (index, id) in payload.ids.into_iter().enumerate() {
        prepared.push((index, id, node_repo::find_by_id(&state.db, id).await?));
    }

    let mut indexed_results = Vec::with_capacity(prepared.len());
    for chunk in prepared.chunks(latency_concurrency(&settings)) {
        let mut tasks = JoinSet::new();
        for (index, id, node) in chunk.iter().cloned() {
            let settings = settings.clone();
            let permit = state
                .latency_test_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| AppError::Internal)?;
            tasks.spawn(async move {
                let _permit = permit;
                let result = match node {
                    Some(node) => real_latency_for_node_with_settings(&node, &settings).await,
                    None => NodeLatencyResult {
                        id,
                        status: "error".to_string(),
                        latency_ms: None,
                        message: Some("node not found".to_string()),
                        tested_at: now_rfc3339(),
                    },
                };
                (index, result)
            });
        }

        while let Some(joined) = tasks.join_next().await {
            let (index, result) = joined.map_err(|_| AppError::Internal)?;
            persist_latency_result(state, &result).await?;
            indexed_results.push((index, result));
        }
    }
    indexed_results.sort_by_key(|(index, _)| *index);
    let data = indexed_results
        .into_iter()
        .map(|(_, result)| result)
        .collect();

    Ok(NodeLatencyBatchResponse {
        code: "00000",
        data,
    })
}

pub async fn test_node_latency_batch_stream(
    state: AppState,
    payload: NodeLatencyBatchRequest,
) -> Result<Body, AppError> {
    begin_latency_run(&state).await?;
    let prepared = match prepare_latency_batch(&state, payload).await {
        Ok(prepared) => prepared,
        Err(error) => {
            state.finish_manual_latency_run().await;
            return Err(error);
        }
    };
    let settings = match settings_service::load_settings(&state).await {
        Ok(settings) => settings,
        Err(error) => {
            state.finish_manual_latency_run().await;
            return Err(error);
        }
    };
    if let Err(error) = ensure_mihomo_core_ready(&settings).await {
        state.finish_manual_latency_run().await;
        return Err(error);
    }

    let (sender, receiver) = mpsc::channel::<Result<Bytes, Infallible>>(16);
    tokio::spawn(async move {
        for chunk in prepared.chunks(latency_concurrency(&settings)) {
            let mut tasks = JoinSet::new();
            for (index, id, node) in chunk.iter().cloned() {
                let settings = settings.clone();
                let permit = match state.latency_test_semaphore.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        let result = NodeLatencyResult {
                            id,
                            status: "error".to_string(),
                            latency_ms: None,
                            message: Some("latency test limiter is unavailable".to_string()),
                            tested_at: now_rfc3339(),
                        };
                        tasks.spawn(async move { (index, result) });
                        continue;
                    }
                };
                tasks.spawn(async move {
                    let _permit = permit;
                    let result = match node {
                        Some(node) => real_latency_for_node_with_settings(&node, &settings).await,
                        None => NodeLatencyResult {
                            id,
                            status: "error".to_string(),
                            latency_ms: None,
                            message: Some("node not found".to_string()),
                            tested_at: now_rfc3339(),
                        },
                    };
                    (index, result)
                });
            }

            while let Some(joined) = tasks.join_next().await {
                let Ok((_, result)) = joined else {
                    continue;
                };
                let _ = persist_latency_result(&state, &result).await;
                let Ok(line) = serde_json::to_string(&result).map(|value| format!("{value}\n"))
                else {
                    continue;
                };
                if sender.send(Ok(Bytes::from(line))).await.is_err() {
                    state.finish_manual_latency_run().await;
                    return;
                }
            }
        }

        state.finish_manual_latency_run().await;
    });

    Ok(Body::from_stream(ReceiverStream::new(receiver)))
}

async fn prepare_latency_batch(
    state: &AppState,
    payload: NodeLatencyBatchRequest,
) -> Result<Vec<(usize, i64, Option<crate::domain::node::NodeRecord>)>, AppError> {
    if payload.ids.is_empty() {
        return Ok(Vec::new());
    }
    if payload.ids.len() > MAX_LATENCY_BATCH_SIZE {
        return Err(AppError::BadRequest(format!(
            "at most {MAX_LATENCY_BATCH_SIZE} nodes can be tested at once"
        )));
    }

    let mut prepared = Vec::with_capacity(payload.ids.len());
    for (index, id) in payload.ids.into_iter().enumerate() {
        prepared.push((index, id, node_repo::find_by_id(&state.db, id).await?));
    }
    Ok(prepared)
}

pub async fn test_all_enabled_node_latencies(
    state: &AppState,
) -> Result<Vec<NodeLatencyResult>, AppError> {
    if !state.try_begin_auto_latency_run().await {
        return Err(AppError::TooManyRequests(
            "latency testing is already running".to_string(),
        ));
    }

    let result = test_all_enabled_node_latencies_inner(state).await;
    state.finish_auto_latency_run().await;
    result
}

async fn test_all_enabled_node_latencies_inner(
    state: &AppState,
) -> Result<Vec<NodeLatencyResult>, AppError> {
    let settings = settings_service::load_settings(state).await?;
    ensure_mihomo_core_ready(&settings).await?;
    let nodes = node_repo::list_enabled(&state.db).await?;
    let mut indexed_results = Vec::with_capacity(nodes.len());

    for chunk in nodes
        .into_iter()
        .enumerate()
        .collect::<Vec<_>>()
        .chunks(latency_concurrency(&settings))
    {
        if state.is_auto_latency_cancel_requested().await {
            break;
        }

        let mut tasks = JoinSet::new();
        for (index, node) in chunk.iter().cloned() {
            let settings = settings.clone();
            let state = state.clone();
            let permit = state
                .latency_test_semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| AppError::Internal)?;
            tasks.spawn(async move {
                let _permit = permit;
                let result = real_latency_for_auto_node(&state, &node, &settings).await;
                (index, result)
            });
        }

        while let Some(joined) = tasks.join_next().await {
            let (index, result) = joined.map_err(|_| AppError::Internal)?;
            if let Some(result) = result {
                persist_latency_result(state, &result).await?;
                indexed_results.push((index, result));
            }
        }
    }

    indexed_results.sort_by_key(|(index, _)| *index);
    Ok(indexed_results
        .into_iter()
        .map(|(_, result)| result)
        .collect())
}

async fn begin_latency_run(state: &AppState) -> Result<(), AppError> {
    state
        .try_begin_manual_latency_run(LATENCY_RUN_COOLDOWN)
        .await
        .map_err(|error| match error {
            LatencyManualBeginError::AutoRunning => {
                AppError::LatencyAutoRunning("background latency testing is running".to_string())
            }
            LatencyManualBeginError::ManualRunning => {
                AppError::TooManyRequests("latency testing is already running".to_string())
            }
            LatencyManualBeginError::Cooldown => AppError::TooManyRequests(
                "latency testing was started recently; try again later".to_string(),
            ),
        })
}

pub async fn cancel_auto_latency_run(state: &AppState) -> Result<serde_json::Value, AppError> {
    let cancelled = state.request_cancel_auto_latency_run().await;
    Ok(serde_json::json!({
        "code": "00000",
        "cancelled": cancelled,
    }))
}

fn latency_concurrency(settings: &crate::domain::settings::AppSettingsView) -> usize {
    usize::try_from(
        settings
            .latency_concurrency
            .clamp(1, settings_service::MAX_LATENCY_CONCURRENCY),
    )
    .unwrap_or(1)
}

async fn ensure_mihomo_core_ready(
    settings: &crate::domain::settings::AppSettingsView,
) -> Result<PathBuf, AppError> {
    let Some(binary) = mihomo_core_service::resolve_existing_binary(&settings.latency_core_path)
    else {
        return Err(mihomo_core_missing_error());
    };
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new(&binary).arg("-v").kill_on_drop(true).output(),
    )
    .await
    .map_err(|_| {
        AppError::BadRequest(format!(
            "Mihomo core check timed out: {}. Please download or configure the Mihomo core in Settings.",
            binary.display()
        ))
    })?
    .map_err(|error| {
        AppError::BadRequest(format!(
            "Mihomo core is unavailable: {} ({}). Please download or reconfigure the Mihomo core in Settings.",
            binary.display(),
            error
        ))
    })?;

    if !output.status.success() {
        return Err(AppError::BadRequest(format!(
            "Mihomo core execution failed: {}. Please download or reconfigure the Mihomo core in Settings.",
            binary.display()
        )));
    }
    Ok(binary)
}

fn mihomo_core_missing_error() -> AppError {
    AppError::BadRequest(
        "Mihomo core was not found, so real-link latency testing cannot run. Download the core in Settings or configure backend/mihomo/mihomo manually.".to_string(),
    )
}

async fn persist_latency_result(
    state: &AppState,
    result: &NodeLatencyResult,
) -> Result<(), AppError> {
    let latency_ms = result
        .latency_ms
        .and_then(|value| i64::try_from(value).ok());
    node_repo::update_latency(
        &state.db,
        result.id,
        latency_ms,
        &result.status,
        result.message.as_deref(),
        &result.tested_at,
    )
    .await?;
    Ok(())
}

async fn real_latency_for_node(
    state: &AppState,
    node: &crate::domain::node::NodeRecord,
    settings: &crate::domain::settings::AppSettingsView,
) -> NodeLatencyResult {
    let Ok(permit) = state.latency_test_semaphore.clone().acquire_owned().await else {
        return NodeLatencyResult {
            id: node.id,
            status: "error".to_string(),
            latency_ms: None,
            message: Some("latency test limiter is unavailable".to_string()),
            tested_at: now_rfc3339(),
        };
    };
    let _permit = permit;
    real_latency_for_node_with_settings(node, settings).await
}

async fn real_latency_for_node_with_settings(
    node: &crate::domain::node::NodeRecord,
    settings: &crate::domain::settings::AppSettingsView,
) -> NodeLatencyResult {
    if node.port <= 0 || node.port > u16::MAX as i64 {
        return NodeLatencyResult {
            id: node.id,
            status: "error".to_string(),
            latency_ms: None,
            message: Some("invalid port".to_string()),
            tested_at: now_rfc3339(),
        };
    }

    match mihomo_real_latency_for_node(node, settings, None).await {
        Ok(delay) => NodeLatencyResult {
            id: node.id,
            status: "ok".to_string(),
            latency_ms: Some(delay),
            message: None,
            tested_at: now_rfc3339(),
        },
        Err(message) => NodeLatencyResult {
            id: node.id,
            status: if message.contains("timed out") {
                "timeout".to_string()
            } else {
                "error".to_string()
            },
            latency_ms: None,
            message: Some(message),
            tested_at: now_rfc3339(),
        },
    }
}

async fn real_latency_for_auto_node(
    state: &AppState,
    node: &crate::domain::node::NodeRecord,
    settings: &crate::domain::settings::AppSettingsView,
) -> Option<NodeLatencyResult> {
    if state.is_auto_latency_cancel_requested().await {
        return None;
    }

    let result = match mihomo_real_latency_for_node(node, settings, Some(state)).await {
        Ok(delay) => NodeLatencyResult {
            id: node.id,
            status: "ok".to_string(),
            latency_ms: Some(delay),
            message: None,
            tested_at: now_rfc3339(),
        },
        Err(message) => {
            if state.is_auto_latency_cancel_requested().await {
                return None;
            }

            NodeLatencyResult {
                id: node.id,
                status: if message.contains("timed out") {
                    "timeout".to_string()
                } else {
                    "error".to_string()
                },
                latency_ms: None,
                message: Some(message),
                tested_at: now_rfc3339(),
            }
        }
    };

    Some(result)
}

async fn mihomo_real_latency_for_node(
    node: &crate::domain::node::NodeRecord,
    settings: &crate::domain::settings::AppSettingsView,
    cancel_state: Option<&AppState>,
) -> Result<u128, String> {
    if let Some(state) = cancel_state
        && state.is_auto_latency_cancel_requested().await
    {
        return Err("latency test cancelled".to_string());
    }

    let binary = resolve_mihomo_binary(settings)?;
    let mixed_port = allocate_local_port().map_err(|error| error.to_string())?;
    let controller_port = allocate_local_port().map_err(|error| error.to_string())?;
    let config_path = write_mihomo_latency_config(node, mixed_port, controller_port)?;
    let _config_guard = TempFileGuard(config_path.clone());
    let timeout_ms = settings.latency_timeout_secs.clamp(3, 60) * 1000;

    let mut child = Command::new(&binary)
        .arg("-f")
        .arg(&config_path)
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            format!(
                "failed to start Mihomo core '{}': {}. 请在系统设置中配�?mihomo.exe 路径",
                binary.display(),
                error
            )
        })?;

    if let Some(state) = cancel_state
        && state.is_auto_latency_cancel_requested().await
    {
        let _ = child.kill().await;
        return Err("latency test cancelled".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(
            settings.latency_timeout_secs.clamp(3, 60) as u64 + 5,
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let proxy_name = yaml_safe_proxy_name(node);
    let delay_url = format!(
        "http://127.0.0.1:{}/proxies/{}/delay?timeout={}&url={}",
        controller_port,
        encode_uri_component(&proxy_name),
        timeout_ms,
        encode_uri_component(&settings.latency_test_url)
    );

    let response = {
        let mut last_error = String::new();
        let mut response = None;
        for _ in 0..30 {
            let request = client.get(&delay_url).send();
            tokio::pin!(request);
            let request_result = loop {
                tokio::select! {
                    result = &mut request => {
                        break result;
                    }
                    _ = sleep(Duration::from_millis(200)), if cancel_state.is_some() => {
                        if let Some(state) = cancel_state
                            && state.is_auto_latency_cancel_requested().await
                        {
                            let _ = child.kill().await;
                            return Err("latency test cancelled".to_string());
                        }
                    }
                }
            };

            match request_result {
                Ok(value) => {
                    response = Some(value);
                    break;
                }
                Err(error) => {
                    last_error = error.to_string();
                    sleep(Duration::from_millis(150)).await;
                }
            }
        }
        response.ok_or_else(|| format!("mihomo controller unavailable: {last_error}"))?
    };

    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    let _ = child.kill().await;

    if !status.is_success() {
        return Err(format!("mihomo delay api returned {status}: {body}"));
    }

    let payload =
        serde_json::from_str::<serde_json::Value>(&body).map_err(|error| error.to_string())?;
    payload
        .get("delay")
        .and_then(|value| value.as_u64())
        .map(u128::from)
        .ok_or_else(|| format!("mihomo delay response missing delay: {body}"))
}

fn resolve_mihomo_binary(
    settings: &crate::domain::settings::AppSettingsView,
) -> Result<PathBuf, String> {
    Ok(
        mihomo_core_service::resolve_existing_binary(&settings.latency_core_path)
            .unwrap_or_else(mihomo_core_service::fallback_binary_path),
    )
}

fn write_mihomo_latency_config(
    node: &crate::domain::node::NodeRecord,
    mixed_port: u16,
    controller_port: u16,
) -> Result<PathBuf, String> {
    let view = NodeView::try_from(node.clone()).map_err(|error| error.to_string())?;
    let proxy = export_service::render_mihomo_proxy(&view).map_err(|error| error.to_string())?;
    let proxy_name = yaml_safe_proxy_name(node);
    let mut proxy = proxy;
    proxy.insert(
        Value::String("name".to_string()),
        Value::String(proxy_name.clone()),
    );

    let mut root = Mapping::new();
    root.insert(
        Value::String("mixed-port".to_string()),
        Value::Number(i64::from(mixed_port).into()),
    );
    root.insert(Value::String("allow-lan".to_string()), Value::Bool(false));
    root.insert(
        Value::String("mode".to_string()),
        Value::String("rule".to_string()),
    );
    root.insert(
        Value::String("log-level".to_string()),
        Value::String("error".to_string()),
    );
    root.insert(
        Value::String("external-controller".to_string()),
        Value::String(format!("127.0.0.1:{controller_port}")),
    );
    root.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(vec![Value::Mapping(proxy)]),
    );
    root.insert(
        Value::String("proxy-groups".to_string()),
        Value::Sequence(vec![Value::Mapping({
            let mut group = Mapping::new();
            group.insert(
                Value::String("name".to_string()),
                Value::String("PROXY".to_string()),
            );
            group.insert(
                Value::String("type".to_string()),
                Value::String("select".to_string()),
            );
            group.insert(
                Value::String("proxies".to_string()),
                Value::Sequence(vec![Value::String(proxy_name)]),
            );
            group
        })]),
    );
    root.insert(
        Value::String("rules".to_string()),
        Value::Sequence(vec![Value::String("MATCH,PROXY".to_string())]),
    );

    let yaml = serde_yaml::to_string(&Value::Mapping(root)).map_err(|error| error.to_string())?;
    let path = std::env::temp_dir().join(format!(
        "sublinkx-latency-{}-{}.yaml",
        node.id,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis()
    ));
    fs::write(&path, yaml).map_err(|error| error.to_string())?;
    Ok(path)
}

fn allocate_local_port() -> Result<u16, std::io::Error> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr().map(|addr| addr.port()))
}

struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn yaml_safe_proxy_name(node: &crate::domain::node::NodeRecord) -> String {
    format!(
        "node-{}-{}",
        node.id,
        node.name.replace(['/', '\\', '?', '#'], "_")
    )
}

async fn fetch_subscription_body(url: &str) -> Result<String, AppError> {
    let validated_url = url_safety::validate_public_http_url(url, "subscription url").await?;

    let client = validated_url
        .pin_reqwest_resolver(reqwest::Client::builder())
        .timeout(Duration::from_secs(25))
        .user_agent("SublinkX-RS/0.1 Mihomo")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::Internal)?;
    let response = client
        .get(validated_url.as_str())
        .header(
            reqwest::header::USER_AGENT,
            "Clash.Meta/1.19.0 mihomo/1.19.0",
        )
        .header(
            reqwest::header::ACCEPT,
            "application/yaml,text/yaml,text/plain,*/*",
        )
        .send()
        .await
        .map_err(|error| AppError::BadRequest(format!("failed to fetch subscription: {error}")))?;
    if response.status().is_redirection() {
        return Err(AppError::BadRequest(
            "upstream subscription redirects are not allowed".to_string(),
        ));
    }
    if !response.status().is_success() {
        return Err(AppError::BadRequest(format!(
            "upstream subscription returned {}",
            response.status()
        )));
    }
    if let Some(length) = response.content_length()
        && length > MAX_SUBSCRIPTION_BODY_BYTES as u64
    {
        return Err(AppError::BadRequest(
            "upstream subscription is too large".to_string(),
        ));
    }

    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| AppError::BadRequest(format!("failed to read subscription: {error}")))?
    {
        if body.len() + chunk.len() > MAX_SUBSCRIPTION_BODY_BYTES {
            return Err(AppError::BadRequest(
                "upstream subscription is too large".to_string(),
            ));
        }
        body.extend_from_slice(&chunk);
    }

    String::from_utf8(body).map_err(|_| {
        AppError::BadRequest("upstream subscription is not valid UTF-8 text".to_string())
    })
}

fn extract_subscription_links(body: &str) -> Result<Vec<String>, AppError> {
    let trimmed = body.trim();
    let candidates = if looks_like_node_lines(trimmed) {
        trimmed.to_string()
    } else if let Some(decoded) = decode_base64_text(trimmed)
        && looks_like_node_lines(&decoded)
    {
        decoded
    } else if let Ok(links) = extract_mihomo_yaml_links(trimmed)
        && !links.is_empty()
    {
        return Ok(links);
    } else {
        trimmed.to_string()
    };

    Ok(candidates
        .lines()
        .flat_map(|line| line.split_whitespace())
        .map(str::trim)
        .filter(|line| is_supported_raw_link(line))
        .map(str::to_string)
        .collect())
}

fn check_mihomo_conversion_fidelity(body: &str) -> Vec<NodeFidelityWarning> {
    let yaml_body = if is_mihomo_profile_yaml(body) {
        Some(body.trim().to_string())
    } else {
        decode_base64_text(body).filter(|decoded| is_mihomo_profile_yaml(decoded))
    };

    let Some(yaml_body) = yaml_body else {
        return Vec::new();
    };

    let Ok(root) = serde_yaml::from_str::<Value>(&yaml_body) else {
        return Vec::new();
    };
    let Some(proxies) = root
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String("proxies".to_string())))
        .and_then(Value::as_sequence)
    else {
        return Vec::new();
    };

    proxies
        .iter()
        .filter_map(Value::as_mapping)
        .flat_map(check_proxy_fidelity_for_targets)
        .collect()
}

fn check_proxy_fidelity_for_targets(proxy: &Mapping) -> Vec<NodeFidelityWarning> {
    let name = yaml_string(proxy, "name").unwrap_or("unnamed").to_string();
    if is_subscription_info_name(&name) {
        return Vec::new();
    }
    let Some(protocol) = yaml_string(proxy, "type").map(str::to_string) else {
        return Vec::new();
    };
    let Some(raw_link) = mihomo_proxy_to_raw_link(proxy) else {
        return Vec::new();
    };
    let Ok(parsed) = protocol_parser_service::parse_raw_link(&raw_link, None) else {
        return Vec::new();
    };
    let node = fidelity_node_from_parsed(raw_link, parsed);

    let mut warnings = Vec::new();
    if let Some(warning) = check_mihomo_proxy_fidelity(proxy, &node, &name, &protocol) {
        warnings.push(warning);
    }
    if let Some(warning) = check_sing_box_proxy_fidelity(proxy, &node, &name, &protocol) {
        warnings.push(warning);
    }
    if let Some(warning) = check_surge_proxy_fidelity(proxy, &node, &name, &protocol) {
        warnings.push(warning);
    }
    if let Some(warning) = check_quanx_proxy_fidelity(proxy, &node, &name, &protocol) {
        warnings.push(warning);
    }
    warnings
}

fn fidelity_node_from_parsed(
    raw_link: String,
    parsed: protocol_parser_service::ParsedNode,
) -> NodeView {
    NodeView {
        id: 0,
        name: parsed.name,
        protocol: parsed.protocol.as_str().to_string(),
        raw_link,
        server: parsed.server,
        port: i64::from(parsed.port),
        enabled: true,
        group_id: None,
        source_type: "fidelity_check".to_string(),
        source_ref: None,
        upstream_missing: false,
        fingerprint: parsed.fingerprint,
        settings: parsed.settings,
        remark: String::new(),
        last_latency_ms: None,
        last_latency_status: None,
        last_latency_message: None,
        last_latency_tested_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn check_mihomo_proxy_fidelity(
    proxy: &Mapping,
    node: &NodeView,
    name: &str,
    protocol: &str,
) -> Option<NodeFidelityWarning> {
    let rendered = export_service::render_mihomo_proxy(node).ok()?;

    let mut missing_fields = Vec::new();
    let mut changed_fields = Vec::new();
    for field in important_mihomo_fields(protocol) {
        let original = proxy.get(Value::String(field.to_string()));
        if original.is_none() {
            continue;
        }

        let rendered_value = rendered.get(Value::String(field.to_string()));
        if rendered_value.is_none() {
            missing_fields.push(field.to_string());
        } else if !yaml_values_equivalent(original, rendered_value) {
            changed_fields.push(field.to_string());
        }
    }

    if missing_fields.is_empty() && changed_fields.is_empty() {
        None
    } else {
        Some(NodeFidelityWarning {
            target: "mihomo".to_string(),
            name: name.to_string(),
            protocol: protocol.to_string(),
            missing_fields,
            changed_fields,
        })
    }
}

fn check_sing_box_proxy_fidelity(
    proxy: &Mapping,
    node: &NodeView,
    name: &str,
    protocol: &str,
) -> Option<NodeFidelityWarning> {
    let rendered = export_service::render_sing_box_outbound(node).ok()?;
    let missing_fields =
        mapped_missing_fields(proxy, sing_box_field_mappings(protocol), |target| {
            json_path_exists(&rendered, target)
        });
    if missing_fields.is_empty() {
        None
    } else {
        Some(NodeFidelityWarning {
            target: "sing-box".to_string(),
            name: name.to_string(),
            protocol: protocol.to_string(),
            missing_fields,
            changed_fields: Vec::new(),
        })
    }
}

fn check_surge_proxy_fidelity(
    proxy: &Mapping,
    node: &NodeView,
    name: &str,
    protocol: &str,
) -> Option<NodeFidelityWarning> {
    let rendered = export_service::render_surge_proxy(node).ok()?;
    let missing_fields = mapped_missing_fields(proxy, surge_field_mappings(protocol), |target| {
        rendered.proxy_line.contains(target)
            || rendered
                .wireguard_section
                .as_deref()
                .is_some_and(|section| section.contains(target))
    });
    if missing_fields.is_empty() {
        None
    } else {
        Some(NodeFidelityWarning {
            target: "surge".to_string(),
            name: name.to_string(),
            protocol: protocol.to_string(),
            missing_fields,
            changed_fields: Vec::new(),
        })
    }
}

fn check_quanx_proxy_fidelity(
    proxy: &Mapping,
    node: &NodeView,
    name: &str,
    protocol: &str,
) -> Option<NodeFidelityWarning> {
    let rendered = export_service::render_quantumult_x_proxy(node).ok()?;
    let missing_fields = mapped_missing_fields(proxy, quanx_field_mappings(protocol), |target| {
        rendered.contains(target)
    });
    if missing_fields.is_empty() {
        None
    } else {
        Some(NodeFidelityWarning {
            target: "quanx".to_string(),
            name: name.to_string(),
            protocol: protocol.to_string(),
            missing_fields,
            changed_fields: Vec::new(),
        })
    }
}

fn mapped_missing_fields<F>(
    proxy: &Mapping,
    mappings: &[(&str, &str)],
    mut rendered_has_field: F,
) -> Vec<String>
where
    F: FnMut(&str) -> bool,
{
    mappings
        .iter()
        .filter_map(|(source, target)| {
            if should_check_mapped_field(proxy, source, target) && !rendered_has_field(target) {
                Some(format!("{source}->{target}"))
            } else {
                None
            }
        })
        .collect()
}

fn should_check_mapped_field(proxy: &Mapping, source: &str, target: &str) -> bool {
    let Some(value) = proxy.get(Value::String(source.to_string())) else {
        return false;
    };

    if matches!(target, "ws=true" | "obfs=ws" | "ws-path=" | "obfs-uri=") {
        return yaml_scalar_as_str(value)
            .map(|value| value.eq_ignore_ascii_case("ws"))
            .unwrap_or(true)
            || source == "ws-opts";
    }
    if matches!(
        target,
        "skip-cert-verify=true" | "tls-verification=false" | "tls.insecure" | "udp-relay=true"
    ) {
        return yaml_bool_value(value).unwrap_or(false);
    }
    if matches!(target, "tls=true" | "over-tls=true" | "tls.enabled") {
        return yaml_bool_value(value).unwrap_or(true);
    }

    true
}

fn yaml_scalar_as_str(value: &Value) -> Option<&str> {
    match value {
        Value::String(value) => Some(value),
        _ => None,
    }
}

fn yaml_bool_value(value: &Value) -> Option<bool> {
    value.as_bool().or_else(|| value.as_str()?.parse().ok())
}

fn sing_box_field_mappings(protocol: &str) -> &'static [(&'static str, &'static str)] {
    match protocol {
        "vless" => &[
            ("server", "server"),
            ("port", "server_port"),
            ("uuid", "uuid"),
            ("flow", "flow"),
            ("network", "transport.type"),
            ("tls", "tls.enabled"),
            ("skip-cert-verify", "tls.insecure"),
            ("servername", "tls.server_name"),
            ("sni", "tls.server_name"),
            ("client-fingerprint", "tls.utls.fingerprint"),
            ("reality-opts", "tls.reality.public_key"),
            ("ws-opts", "transport.path"),
            ("grpc-opts", "transport.service_name"),
            ("packet-encoding", "packet_encoding"),
        ],
        "hysteria2" | "hy2" => &[
            ("server", "server"),
            ("port", "server_port"),
            ("password", "password"),
            ("sni", "tls.server_name"),
            ("skip-cert-verify", "tls.insecure"),
            ("obfs", "obfs.type"),
            ("obfs-password", "obfs.password"),
            ("ports", "server_ports"),
            ("up", "up_mbps"),
            ("down", "down_mbps"),
            ("alpn", "tls.alpn"),
        ],
        "trojan" => &[
            ("server", "server"),
            ("port", "server_port"),
            ("password", "password"),
            ("network", "transport.type"),
            ("tls", "tls.enabled"),
            ("skip-cert-verify", "tls.insecure"),
            ("servername", "tls.server_name"),
            ("sni", "tls.server_name"),
            ("ws-opts", "transport.path"),
            ("grpc-opts", "transport.service_name"),
        ],
        "ss" | "shadowsocks" => &[
            ("server", "server"),
            ("port", "server_port"),
            ("cipher", "method"),
            ("password", "password"),
        ],
        _ => &[],
    }
}

fn surge_field_mappings(protocol: &str) -> &'static [(&'static str, &'static str)] {
    match protocol {
        "vless" => &[
            ("uuid", "username="),
            ("flow", "flow="),
            ("network", "ws=true"),
            ("tls", "tls=true"),
            ("skip-cert-verify", "skip-cert-verify=true"),
            ("servername", "sni="),
            ("sni", "sni="),
            ("client-fingerprint", "client-fingerprint="),
            ("reality-opts", "reality-public-key="),
            ("ws-opts", "ws-path="),
        ],
        "hysteria2" | "hy2" => &[
            ("password", "password="),
            ("sni", "sni="),
            ("skip-cert-verify", "skip-cert-verify=true"),
            ("obfs-password", "salamander-password="),
            ("down", "download-bandwidth="),
        ],
        "trojan" => &[
            ("password", "password="),
            ("network", "ws=true"),
            ("skip-cert-verify", "skip-cert-verify=true"),
            ("servername", "sni="),
            ("sni", "sni="),
            ("ws-opts", "ws-path="),
        ],
        "ss" | "shadowsocks" => &[
            ("cipher", "encrypt-method="),
            ("password", "password="),
            ("udp", "udp-relay=true"),
        ],
        _ => &[],
    }
}

fn quanx_field_mappings(protocol: &str) -> &'static [(&'static str, &'static str)] {
    match protocol {
        "vless" => &[
            ("uuid", "password="),
            ("flow", "flow="),
            ("network", "obfs=ws"),
            ("tls", "over-tls=true"),
            ("skip-cert-verify", "tls-verification=false"),
            ("servername", "tls-host="),
            ("sni", "tls-host="),
            ("reality-opts", "reality-public-key="),
            ("ws-opts", "obfs-uri="),
        ],
        "hysteria2" | "hy2" => &[("password", "password="), ("sni", "server_check_url=")],
        "trojan" => &[
            ("password", "password="),
            ("network", "obfs=ws"),
            ("tls", "over-tls=true"),
            ("skip-cert-verify", "tls-verification=false"),
            ("servername", "tls-host="),
            ("sni", "tls-host="),
            ("ws-opts", "obfs-uri="),
        ],
        "ss" | "shadowsocks" => &[("cipher", "method="), ("password", "password=")],
        _ => &[],
    }
}

fn json_path_exists(value: &serde_json::Value, path: &str) -> bool {
    let mut current = value;
    for segment in path.split('.') {
        let Some(next) = current.get(segment) else {
            return false;
        };
        current = next;
    }

    !current.is_null()
}

fn important_mihomo_fields(protocol: &str) -> &'static [&'static str] {
    match protocol {
        "vless" => &[
            "type",
            "server",
            "port",
            "uuid",
            "encryption",
            "flow",
            "network",
            "tls",
            "skip-cert-verify",
            "servername",
            "client-fingerprint",
            "reality-opts",
            "ws-opts",
            "grpc-opts",
            "packet-encoding",
            "udp",
        ],
        "hysteria2" | "hy2" => &[
            "type",
            "server",
            "port",
            "password",
            "sni",
            "skip-cert-verify",
            "obfs",
            "obfs-password",
            "ports",
            "up",
            "down",
            "alpn",
        ],
        "trojan" => &[
            "type",
            "server",
            "port",
            "password",
            "network",
            "tls",
            "skip-cert-verify",
            "servername",
            "sni",
            "ws-opts",
            "grpc-opts",
            "udp",
        ],
        "vmess" => &[
            "type",
            "server",
            "port",
            "uuid",
            "alterId",
            "cipher",
            "network",
            "tls",
            "skip-cert-verify",
            "servername",
            "ws-opts",
            "grpc-opts",
            "udp",
        ],
        "ss" | "shadowsocks" => &["type", "server", "port", "cipher", "password", "udp"],
        _ => &["type", "server", "port"],
    }
}

fn yaml_values_equivalent(left: Option<&Value>, right: Option<&Value>) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };

    normalize_yaml_value(left) == normalize_yaml_value(right)
}

fn normalize_yaml_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.trim().to_string(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => serde_yaml::to_string(value).unwrap_or_default(),
    }
}

async fn save_upstream_template_if_mihomo_yaml(
    state: &AppState,
    url: &str,
    body: &str,
    group_id: Option<i64>,
) -> Result<Option<crate::domain::template::TemplateRecord>, AppError> {
    let template_body = if is_mihomo_profile_yaml(body) {
        sanitize_mihomo_profile_yaml(body)?
    } else if let Some(decoded) = decode_base64_text(body)
        && is_mihomo_profile_yaml(&decoded)
    {
        sanitize_mihomo_profile_yaml(&decoded)?
    } else {
        return Ok(None);
    };

    let now = now_rfc3339();
    let name = unique_upstream_template_name(state, url, group_id).await?;
    let content = mark_upstream_template(&template_body);
    let template = template_repo::insert(
        &state.db,
        &template_repo::NewTemplateRecord {
            name: &name,
            kind: "mihomo",
            content: &content,
            is_builtin: 0,
            created_at: &now,
            updated_at: &now,
        },
    )
    .await?;

    Ok(Some(template))
}

async fn unique_upstream_template_name(
    state: &AppState,
    url: &str,
    group_id: Option<i64>,
) -> Result<String, AppError> {
    let digest = &hex::encode(Sha256::digest(url.as_bytes()))[..8];
    let group_name = upstream_template_group_name(state, group_id).await?;
    let base = truncate_template_name_base(&format!("Upstream Mihomo {group_name} {digest}"));
    if template_repo::find_by_name(&state.db, &base)
        .await?
        .is_none()
    {
        return Ok(base);
    }

    for index in 2..100 {
        let candidate = format!("{base} #{index}");
        if template_repo::find_by_name(&state.db, &candidate)
            .await?
            .is_none()
        {
            return Ok(candidate);
        }
    }

    Err(AppError::Internal)
}

async fn upstream_template_group_name(
    state: &AppState,
    group_id: Option<i64>,
) -> Result<String, AppError> {
    let Some(group_id) = group_id else {
        return Ok("Ungrouped".to_string());
    };

    let group = group_repo::find_by_id(&state.db, GroupTable::Node, group_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("node group not found: {group_id}")))?;
    Ok(sanitize_template_name_part(&group.name))
}

fn sanitize_template_name_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if sanitized.is_empty() {
        "Group".to_string()
    } else {
        sanitized
    }
}

fn truncate_template_name_base(value: &str) -> String {
    const MAX_BASE_CHARS: usize = 170;
    value.chars().take(MAX_BASE_CHARS).collect()
}

fn mark_upstream_template(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.contains("x-sublinkx-upstream-template: true") {
        return trimmed.to_string();
    }
    format!("x-sublinkx-upstream-template: true\n{trimmed}\n")
}

fn is_mihomo_profile_yaml(body: &str) -> bool {
    serde_yaml::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            let mapping = value.as_mapping()?;
            let proxies = mapping
                .get(Value::String("proxies".to_string()))
                .and_then(Value::as_sequence)?;
            Some(!proxies.is_empty())
        })
        .unwrap_or(false)
}

fn sanitize_mihomo_profile_yaml(body: &str) -> Result<String, AppError> {
    let mut root = serde_yaml::from_str::<Value>(body)
        .map_err(|_| AppError::BadRequest("subscription is not valid YAML".to_string()))?;
    let Some(mapping) = root.as_mapping_mut() else {
        return Err(AppError::BadRequest(
            "YAML subscription must be a mapping".to_string(),
        ));
    };

    let proxies_key = Value::String("proxies".to_string());
    let Some(proxies) = mapping
        .get_mut(&proxies_key)
        .and_then(Value::as_sequence_mut)
    else {
        return Err(AppError::BadRequest(
            "YAML subscription missing proxies".to_string(),
        ));
    };

    let mut removed_names = Vec::new();
    proxies.retain(|proxy| {
        let keep = proxy
            .as_mapping()
            .and_then(|item| yaml_string(item, "name"))
            .map(|name| !is_subscription_info_name(name))
            .unwrap_or(false);
        if !keep
            && let Some(name) = proxy
                .as_mapping()
                .and_then(|item| yaml_string(item, "name"))
        {
            removed_names.push(name.to_string());
        }
        keep
    });

    if !removed_names.is_empty() {
        remove_proxy_group_members(mapping, &removed_names);
    }

    export_service::dedupe_mihomo_proxy_names(mapping);

    serde_yaml::to_string(&root).map_err(|_| AppError::Internal)
}

fn remove_proxy_group_members(root: &mut Mapping, removed_names: &[String]) {
    let Some(groups) = root
        .get_mut(Value::String("proxy-groups".to_string()))
        .and_then(Value::as_sequence_mut)
    else {
        return;
    };

    for group in groups {
        let Some(group_mapping) = group.as_mapping_mut() else {
            continue;
        };
        let Some(members) = group_mapping
            .get_mut(Value::String("proxies".to_string()))
            .and_then(Value::as_sequence_mut)
        else {
            continue;
        };

        members.retain(|member| {
            member
                .as_str()
                .map(|name| !removed_names.iter().any(|removed| removed == name))
                .unwrap_or(true)
        });
    }
}

fn is_subscription_info_name(name: &str) -> bool {
    let normalized = name.trim().to_lowercase();
    let info_markers = [
        "剩余流量",
        "套餐",
        "到期",
        "过期",
        "官网",
        "网址",
        "订阅",
        "重置",
        "流量",
        "expire",
        "expired",
        "traffic",
        "remaining",
        "reset",
        "官网地址",
        "更新订阅",
    ];

    info_markers
        .iter()
        .any(|marker| normalized.contains(&marker.to_lowercase()))
}

fn looks_like_node_lines(value: &str) -> bool {
    value.lines().any(|line| is_supported_raw_link(line.trim()))
}

fn is_supported_raw_link(value: &str) -> bool {
    matches!(
        value.split_once("://").map(|(scheme, _)| scheme),
        Some(
            "ss" | "vmess"
                | "vless"
                | "trojan"
                | "hy2"
                | "hysteria2"
                | "tuic"
                | "wireguard"
                | "anytls"
                | "any-tls"
        )
    )
}

fn decode_base64_text(value: &str) -> Option<String> {
    let compact = value.lines().map(str::trim).collect::<String>();
    general_purpose::STANDARD
        .decode(pad_base64(&compact))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn pad_base64(input: &str) -> String {
    let mut padded = input.replace('-', "+").replace('_', "/");
    let rem = padded.len() % 4;
    if rem != 0 {
        padded.push_str(&"=".repeat(4 - rem));
    }
    padded
}

fn extract_mihomo_yaml_links(body: &str) -> Result<Vec<String>, AppError> {
    let root = serde_yaml::from_str::<Value>(body)
        .map_err(|_| AppError::BadRequest("subscription is not valid YAML".to_string()))?;
    let proxies = root
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String("proxies".to_string())))
        .and_then(Value::as_sequence)
        .ok_or_else(|| AppError::BadRequest("YAML subscription missing proxies".to_string()))?;

    let mut links = Vec::new();
    for proxy in proxies {
        if let Some(mapping) = proxy.as_mapping()
            && let Some(link) = mihomo_proxy_to_raw_link(mapping)
        {
            links.push(link);
        }
    }
    Ok(links)
}

fn mihomo_proxy_to_raw_link(proxy: &Mapping) -> Option<String> {
    let proxy_type = yaml_string(proxy, "type")?;
    match proxy_type {
        "ss" | "shadowsocks" => mihomo_shadowsocks_to_uri(proxy),
        "vless" => mihomo_vless_to_uri(proxy),
        "trojan" => mihomo_trojan_to_uri(proxy),
        "hysteria2" | "hy2" => mihomo_hysteria2_to_uri(proxy),
        _ => None,
    }
}

fn mihomo_shadowsocks_to_uri(proxy: &Mapping) -> Option<String> {
    let method = yaml_string(proxy, "cipher").or_else(|| yaml_string(proxy, "method"))?;
    let password = yaml_string(proxy, "password")?;
    let server = yaml_string(proxy, "server")?;
    let port = yaml_i64(proxy, "port")?;
    let name = yaml_string(proxy, "name").unwrap_or("shadowsocks");
    let credentials = general_purpose::URL_SAFE_NO_PAD.encode(format!("{method}:{password}"));
    let mut params = Vec::new();
    if let Some(udp) = yaml_bool(proxy, "udp") {
        push_param(&mut params, "udp", if udp { "true" } else { "false" });
    }
    push_optional_param(&mut params, "plugin", yaml_string(proxy, "plugin"));
    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };

    Some(format!(
        "ss://{}@{}:{}{}#{}",
        credentials,
        server,
        port,
        query,
        encode_uri_component(name)
    ))
}

fn mihomo_vless_to_uri(proxy: &Mapping) -> Option<String> {
    let uuid = yaml_string(proxy, "uuid")?;
    let server = yaml_string(proxy, "server")?;
    let port = yaml_i64(proxy, "port")?;
    let name = yaml_string(proxy, "name").unwrap_or("vless");
    let mut params = Vec::new();
    push_param(
        &mut params,
        "encryption",
        yaml_string(proxy, "encryption").unwrap_or("none"),
    );
    push_optional_param(&mut params, "flow", yaml_string(proxy, "flow"));
    let network = yaml_string(proxy, "network");
    push_optional_param(&mut params, "type", network);
    if yaml_bool(proxy, "tls").unwrap_or(false) {
        push_param(
            &mut params,
            "security",
            if yaml_mapping(proxy, "reality-opts").is_some() {
                "reality"
            } else {
                "tls"
            },
        );
    }
    if let Some(insecure) = yaml_bool(proxy, "skip-cert-verify") {
        push_param(&mut params, "insecure", if insecure { "1" } else { "0" });
    }
    push_optional_param(
        &mut params,
        "sni",
        yaml_string(proxy, "servername").or_else(|| yaml_string(proxy, "sni")),
    );
    push_optional_param(&mut params, "fp", yaml_string(proxy, "client-fingerprint"));
    push_optional_param(
        &mut params,
        "packet-encoding",
        yaml_string(proxy, "packet-encoding"),
    );
    if network.is_some_and(|value| value.eq_ignore_ascii_case("ws"))
        && let Some(ws_opts) = yaml_mapping(proxy, "ws-opts")
    {
        push_optional_param(&mut params, "path", yaml_string(ws_opts, "path"));
        if let Some(headers) = yaml_mapping(ws_opts, "headers") {
            push_optional_param(
                &mut params,
                "host",
                yaml_string(headers, "Host").or_else(|| yaml_string(headers, "host")),
            );
        }
    }
    if network.is_some_and(|value| value.eq_ignore_ascii_case("grpc"))
        && let Some(grpc_opts) = yaml_mapping(proxy, "grpc-opts")
    {
        push_optional_param(
            &mut params,
            "serviceName",
            yaml_string(grpc_opts, "grpc-service-name"),
        );
    }
    if let Some(reality) = yaml_mapping(proxy, "reality-opts") {
        push_optional_param(&mut params, "pbk", yaml_string(reality, "public-key"));
        push_optional_param(&mut params, "sid", yaml_string(reality, "short-id"));
    }
    Some(format!(
        "vless://{}@{}:{}?{}#{}",
        uuid,
        server,
        port,
        params.join("&"),
        encode_uri_component(name)
    ))
}

fn mihomo_trojan_to_uri(proxy: &Mapping) -> Option<String> {
    let password = yaml_string(proxy, "password")?;
    let server = yaml_string(proxy, "server")?;
    let port = yaml_i64(proxy, "port")?;
    let name = yaml_string(proxy, "name").unwrap_or("trojan");
    let mut params = Vec::new();
    let network = yaml_string(proxy, "network");
    push_optional_param(&mut params, "type", network);
    push_optional_param(
        &mut params,
        "sni",
        yaml_string(proxy, "servername").or_else(|| yaml_string(proxy, "sni")),
    );
    if network.is_some_and(|value| value.eq_ignore_ascii_case("ws"))
        && let Some(ws_opts) = yaml_mapping(proxy, "ws-opts")
    {
        push_optional_param(&mut params, "path", yaml_string(ws_opts, "path"));
        if let Some(headers) = yaml_mapping(ws_opts, "headers") {
            push_optional_param(
                &mut params,
                "host",
                yaml_string(headers, "Host").or_else(|| yaml_string(headers, "host")),
            );
        }
    }
    Some(format!(
        "trojan://{}@{}:{}?{}#{}",
        encode_uri_userinfo(password),
        server,
        port,
        params.join("&"),
        encode_uri_component(name)
    ))
}

fn mihomo_hysteria2_to_uri(proxy: &Mapping) -> Option<String> {
    let password = yaml_string(proxy, "password")?;
    let server = yaml_string(proxy, "server")?;
    let port = yaml_i64(proxy, "port")?;
    let name = yaml_string(proxy, "name").unwrap_or("hysteria2");
    let mut params = Vec::new();
    push_optional_param(
        &mut params,
        "sni",
        yaml_string(proxy, "sni").or_else(|| yaml_string(proxy, "servername")),
    );
    if let Some(insecure) = yaml_bool(proxy, "skip-cert-verify") {
        push_param(&mut params, "insecure", if insecure { "1" } else { "0" });
    }
    push_optional_param(&mut params, "obfs", yaml_string(proxy, "obfs"));
    push_optional_param(
        &mut params,
        "obfs-password",
        yaml_string(proxy, "obfs-password"),
    );
    if let Some(alpn) = yaml_string_or_csv(proxy, "alpn") {
        push_param(&mut params, "alpn", &alpn);
    }
    push_optional_param(&mut params, "ports", yaml_string(proxy, "ports"));
    if let Some(up) = yaml_i64(proxy, "up") {
        push_param(&mut params, "up", &up.to_string());
    }
    if let Some(down) = yaml_i64(proxy, "down") {
        push_param(&mut params, "down", &down.to_string());
    }
    Some(format!(
        "hysteria2://{}@{}:{}?{}#{}",
        encode_uri_userinfo(password),
        server,
        port,
        params.join("&"),
        encode_uri_component(name)
    ))
}

fn yaml_string<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a str> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn yaml_string_or_csv(mapping: &Mapping, key: &str) -> Option<String> {
    if let Some(value) = yaml_string(mapping, key) {
        return Some(value.to_string());
    }

    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|value| !value.is_empty())
}

fn yaml_i64(mapping: &Mapping, key: &str) -> Option<i64> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
}

fn yaml_bool(mapping: &Mapping, key: &str) -> Option<bool> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(|value| value.as_bool().or_else(|| value.as_str()?.parse().ok()))
}

fn yaml_mapping<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_mapping)
}

fn push_param(params: &mut Vec<String>, key: &str, value: &str) {
    params.push(format!("{key}={}", encode_uri_component(value)));
}

fn push_optional_param(params: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        push_param(params, key, value);
    }
}

fn encode_uri_component(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}

fn encode_uri_userinfo(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC)
        .to_string()
        .replace("%2D", "-")
        .replace("%2d", "-")
        .replace("%5F", "_")
        .replace("%5f", "_")
        .replace("%2E", ".")
        .replace("%2e", ".")
        .replace("%7E", "~")
        .replace("%7e", "~")
}

fn truncate_source(value: &str) -> String {
    const MAX_LEN: usize = 140;
    if value.chars().count() <= MAX_LEN {
        return value.to_string();
    }
    let mut truncated = value.chars().take(MAX_LEN).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn bool_to_db(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

pub async fn delete_node(state: &AppState, id: i64) -> Result<(), AppError> {
    let existing = node_repo::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("node not found".to_string()))?;
    node_repo::delete(&state.db, existing.id).await?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{
        check_mihomo_conversion_fidelity, mihomo_proxy_to_raw_link, sanitize_mihomo_profile_yaml,
    };
    use crate::services::url_safety::validate_public_http_url;
    use serde_yaml::Value;

    #[test]
    fn checks_vless_reality_across_client_renderers_without_missing_fields() {
        let yaml = r#"
proxies:
  - name: VLESS Reality WS
    type: vless
    server: edge.example.com
    port: 443
    uuid: 4c374a1d-e334-4ec1-b010-489bfa360ba9
    encryption: none
    flow: xtls-rprx-vision
    network: ws
    tls: true
    skip-cert-verify: true
    servername: sni.example.com
    client-fingerprint: chrome
    reality-opts:
      public-key: public-key-value
      short-id: abcd
    ws-opts:
      path: /ws
      headers:
        Host: host.example.com
"#;

        let warnings = check_mihomo_conversion_fidelity(yaml);

        assert!(
            warnings.is_empty(),
            "unexpected fidelity warnings: {warnings:#?}"
        );
    }

    #[test]
    fn checks_hysteria2_across_client_renderers_without_missing_fields() {
        let yaml = r#"
proxies:
  - name: HY2 Full
    type: hysteria2
    server: hy.example.com
    port: 443
    password: secret
    sni: sni.example.com
    skip-cert-verify: true
    obfs: salamander
    obfs-password: obfs-secret
    ports: 20000-30000
    up: 300
    down: 300
    alpn:
      - h3
"#;

        let warnings = check_mihomo_conversion_fidelity(yaml);

        assert!(
            warnings.is_empty(),
            "unexpected fidelity warnings: {warnings:#?}"
        );
    }

    #[test]
    fn converts_mihomo_shadowsocks_proxy_to_importable_uri() {
        let yaml = r#"
name: SS Full
type: ss
server: ss.example.com
port: 8388
cipher: aes-256-gcm
password: secret
udp: true
"#;
        let proxy = serde_yaml::from_str::<Value>(yaml).unwrap();
        let mapping = proxy.as_mapping().unwrap();
        let link = mihomo_proxy_to_raw_link(mapping).expect("ss proxy should convert to URI");
        let parsed = crate::services::protocol_parser_service::parse_raw_link(&link, None)
            .expect("converted ss URI should parse");

        assert_eq!(parsed.protocol.as_str(), "shadowsocks");
        assert_eq!(parsed.name, "SS Full");
        assert_eq!(parsed.server, "ss.example.com");
        assert_eq!(parsed.port, 8388);
        assert_eq!(
            parsed
                .settings
                .get("method")
                .and_then(|value| value.as_str()),
            Some("aes-256-gcm")
        );
        assert_eq!(
            parsed
                .settings
                .get("password")
                .and_then(|value| value.as_str()),
            Some("secret")
        );
        assert_eq!(
            parsed.settings.get("udp").and_then(|value| value.as_str()),
            Some("true")
        );
    }

    #[tokio::test]
    async fn rejects_local_subscription_fetch_url() {
        let result =
            validate_public_http_url("http://127.0.0.1:8080/sub", "subscription url").await;

        assert!(result.is_err());
    }

    #[test]
    fn sanitizes_saved_mihomo_template_duplicate_proxy_names() {
        let yaml = r#"
proxies:
  - name: JP 日本01[HY2]
    type: hysteria2
    server: one.example
    port: 443
  - name: JP 日本01[HY2]
    type: hysteria2
    server: two.example
    port: 443
proxy-groups:
  - name: AUTO
    type: select
    proxies:
      - JP 日本01[HY2]
"#;

        let sanitized = sanitize_mihomo_profile_yaml(yaml).unwrap();
        let root = serde_yaml::from_str::<Value>(&sanitized).unwrap();
        let mapping = root.as_mapping().unwrap();
        let names = mapping
            .get(Value::String("proxies".to_string()))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .map(|proxy| {
                proxy
                    .as_mapping()
                    .unwrap()
                    .get(Value::String("name".to_string()))
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["JP 日本01[HY2]", "JP 日本01[HY2] #2"]);
    }
}
