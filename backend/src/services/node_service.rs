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
        NodeLatencyBatchResponse, NodeLatencyResponse, NodeLatencyResult, NodeListQuery,
        NodeListResponse, NodeResponse, UpdateNodeRequest,
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
const MIHOMO_SUBSCRIPTION_USER_AGENT: &str = "mihomo/1.19.10";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionImportMode {
    AddOnly,
    SyncOverwrite,
}

struct ExtractedSubscription {
    raw_links: Vec<String>,
    failures: Vec<NodeImportFailure>,
}

pub async fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    auth_service::require_user(state, headers).await.map(|_| ())
}

pub async fn list_nodes(
    state: &AppState,
    query: NodeListQuery,
) -> Result<NodeListResponse, AppError> {
    let page = query.pagination().normalized();
    let total =
        node_repo::count_filtered(&state.db, query.group_id, query.ungrouped, query.enabled)
            .await?;
    let records = node_repo::list_page(
        &state.db,
        query.group_id,
        query.ungrouped,
        query.enabled,
        query.compact,
        i64::from(page.page_size),
        page.offset,
    )
    .await?;
    let data = records
        .into_iter()
        .map(NodeView::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::Internal)?;

    Ok(NodeListResponse {
        code: "00000",
        data,
        pagination: Some(page.meta(total)),
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
    let extracted = extract_subscription_links(&body)?;
    let raw_links = extracted.raw_links;
    if raw_links.is_empty() {
        let detail = extracted
            .failures
            .first()
            .map(|failure| format!(": {}", failure.reason))
            .unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "no supported node links found in subscription{detail}"
        )));
    }
    if raw_links.len() > MAX_IMPORT_SUBSCRIPTION_NODES {
        return Err(AppError::BadRequest(format!(
            "at most {MAX_IMPORT_SUBSCRIPTION_NODES} nodes can be imported at once"
        )));
    }

    let now = now_rfc3339();
    let mut imported = Vec::new();
    let mut failures = extracted.failures;
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

    if mode == SubscriptionImportMode::SyncOverwrite {
        state.clear_public_export_cache().await;
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
    super::node_ip_probe_scheduler_service::spawn_after_upstream_import(
        state.clone(),
        url.to_string(),
    );

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
            pagination: None,
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
        pagination: None,
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
    let prepared = prepare_latency_batch(state, payload).await?;

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

    let ids = payload.ids;
    let nodes = node_repo::find_by_ids(&state.db, &ids).await?;
    let mut nodes_by_id: HashMap<i64, crate::domain::node::NodeRecord> =
        nodes.into_iter().map(|node| (node.id, node)).collect();
    let prepared = ids
        .into_iter()
        .enumerate()
        .map(|(index, id)| (index, id, nodes_by_id.remove(&id)))
        .collect();
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

pub(crate) async fn ensure_mihomo_core_ready(
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
        .header(reqwest::header::USER_AGENT, MIHOMO_SUBSCRIPTION_USER_AGENT)
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

fn extract_subscription_links(body: &str) -> Result<ExtractedSubscription, AppError> {
    let mut candidates = Vec::with_capacity(3);
    let mut candidate = body
        .trim()
        .trim_start_matches('\u{feff}')
        .trim()
        .to_string();
    candidates.push(candidate.clone());
    for _ in 0..2 {
        let Some(decoded) = decode_base64_text(&candidate) else {
            break;
        };
        let decoded = decoded
            .trim()
            .trim_start_matches('\u{feff}')
            .trim()
            .to_string();
        if decoded.is_empty() || decoded == candidate {
            break;
        }
        candidates.push(decoded.clone());
        candidate = decoded;
    }

    for candidate in &candidates {
        let raw_links = extract_raw_links(candidate);
        if !raw_links.is_empty() {
            return Ok(ExtractedSubscription {
                raw_links,
                failures: Vec::new(),
            });
        }
        if has_mihomo_proxy_sequence(candidate) {
            return extract_mihomo_yaml_links(candidate);
        }
        if let Some(extracted) = extract_json_subscription(candidate)? {
            return Ok(extracted);
        }
    }

    let reason = describe_unrecognized_subscription(
        candidates.last().map(String::as_str).unwrap_or_default(),
    );
    Ok(ExtractedSubscription {
        raw_links: Vec::new(),
        failures: vec![NodeImportFailure {
            source: "upstream subscription".to_string(),
            reason,
        }],
    })
}

fn extract_raw_links(value: &str) -> Vec<String> {
    value
        .lines()
        .flat_map(|line| line.split_whitespace())
        .map(|value| value.trim_matches([',', '"', '\'', '[', ']']))
        .filter(|value| is_supported_raw_link(value))
        .map(str::to_string)
        .collect()
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
    let Ok(raw_link) = mihomo_proxy_to_raw_link(proxy) else {
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
            || (target == "quanx-tls"
                && (rendered.contains("over-tls=true") || rendered.contains("obfs=wss")))
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
            ("flow", "vless-flow="),
            ("network", "obfs=ws"),
            ("tls", "quanx-tls"),
            ("skip-cert-verify", "tls-verification=false"),
            ("servername", "tls-host="),
            ("sni", "tls-host="),
            ("reality-opts", "reality-base64-pubkey="),
            ("ws-opts", "obfs-uri="),
        ],
        "anytls" | "any-tls" => &[
            ("password", "password="),
            ("sni", "tls-host="),
            ("skip-cert-verify", "tls-verification=false"),
            ("udp", "udp-relay=true"),
        ],
        "trojan" => &[
            ("password", "password="),
            ("network", "obfs=ws"),
            ("tls", "quanx-tls"),
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
            "client-fingerprint",
            "fingerprint",
            "alpn",
            "name-cert-verify",
            "reality-opts",
            "ech-opts",
            "shadow-tls-opts",
            "restls-opts",
            "jls-opts",
            "ss-opts",
            "smux",
            "ip-version",
            "tfo",
            "mptcp",
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
            "client-fingerprint",
            "alpn",
            "ws-opts",
            "grpc-opts",
            "udp",
        ],
        "anytls" | "any-tls" => &[
            "type",
            "server",
            "port",
            "password",
            "client-fingerprint",
            "udp",
            "idle-session-check-interval",
            "idle-session-timeout",
            "min-idle-session",
            "sni",
            "alpn",
            "skip-cert-verify",
            "name-cert-verify",
            "shadow-tls-opts",
            "restls-opts",
            "jls-opts",
        ],
        "tuic" => &[
            "type",
            "server",
            "port",
            "token",
            "uuid",
            "password",
            "ip",
            "heartbeat-interval",
            "alpn",
            "disable-sni",
            "reduce-rtt",
            "request-timeout",
            "udp-relay-mode",
            "congestion-controller",
            "bbr-profile",
            "max-udp-relay-packet-size",
            "fast-open",
            "skip-cert-verify",
            "name-cert-verify",
            "max-open-streams",
            "sni",
        ],
        "wireguard" | "wg" => &[
            "type",
            "server",
            "port",
            "ip",
            "private-key",
            "public-key",
            "allowed-ips",
            "pre-shared-key",
            "reserved",
            "persistent-keepalive",
            "mtu",
            "remote-dns-resolve",
            "dns",
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
    let (name, source_digest) = upstream_template_identity(state, url, group_id).await?;
    let content = mark_upstream_template(&template_body);
    let result = template_repo::upsert_latest_upstream_passthrough(
        &state.db,
        &source_digest,
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

    if result.removed_templates > 0 {
        tracing::info!(
            source_digest,
            template_id = result.template.id,
            removed_templates = result.removed_templates,
            "consolidated upstream passthrough templates"
        );
    }
    state.clear_public_export_cache().await;

    Ok(Some(result.template))
}

async fn upstream_template_identity(
    state: &AppState,
    url: &str,
    group_id: Option<i64>,
) -> Result<(String, String), AppError> {
    let digest = &hex::encode(Sha256::digest(url.as_bytes()))[..8];
    let group_name = upstream_template_group_name(state, group_id).await?;
    let base = truncate_template_name_base(&format!("Upstream Mihomo {group_name} {digest}"));
    Ok((base, digest.to_string()))
}

pub async fn reconcile_upstream_passthrough_templates(state: &AppState) -> Result<(), AppError> {
    let templates = template_repo::list_upstream_passthrough(&state.db).await?;
    let mut latest_by_digest = HashMap::new();

    for template in templates {
        let Some((base_name, source_digest)) = upstream_template_name_parts(&template.name) else {
            continue;
        };
        latest_by_digest
            .entry(source_digest)
            .and_modify(|(_, _, count)| *count += 1)
            .or_insert((base_name, template.content, 1_usize));
    }

    let now = now_rfc3339();
    for (source_digest, (name, content, count)) in latest_by_digest {
        if count < 2 {
            continue;
        }
        let result = template_repo::upsert_latest_upstream_passthrough(
            &state.db,
            &source_digest,
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
        if result.removed_templates > 0 {
            tracing::info!(
                source_digest,
                template_id = result.template.id,
                removed_templates = result.removed_templates,
                "reconciled historical upstream passthrough templates"
            );
        }
    }

    Ok(())
}

fn upstream_template_name_parts(name: &str) -> Option<(String, String)> {
    let base_name = match name.rsplit_once(" #") {
        Some((base, revision))
            if !revision.is_empty()
                && revision.chars().all(|character| character.is_ascii_digit()) =>
        {
            base
        }
        _ => name,
    };
    if !base_name.starts_with("Upstream Mihomo ") {
        return None;
    }

    let source_digest = base_name.split_whitespace().next_back()?;
    if source_digest.len() != 8
        || !source_digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }

    Some((base_name.to_string(), source_digest.to_string()))
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

fn has_mihomo_proxy_sequence(body: &str) -> bool {
    serde_yaml::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            let mapping = value.as_mapping()?;
            mihomo_proxy_sequence(mapping).map(|_| true)
        })
        .unwrap_or(false)
}

fn mihomo_proxy_sequence(mapping: &Mapping) -> Option<&Vec<Value>> {
    ["proxies", "payload"].into_iter().find_map(|key| {
        mapping
            .get(Value::String(key.to_string()))
            .and_then(Value::as_sequence)
    })
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
        "导航页",
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

fn extract_mihomo_yaml_links(body: &str) -> Result<ExtractedSubscription, AppError> {
    let root = serde_yaml::from_str::<Value>(body)
        .map_err(|_| AppError::BadRequest("subscription is not valid YAML".to_string()))?;
    let proxies = root
        .as_mapping()
        .and_then(mihomo_proxy_sequence)
        .ok_or_else(|| {
            AppError::BadRequest("YAML subscription missing proxies or payload".to_string())
        })?;
    if proxies.is_empty() {
        return Ok(ExtractedSubscription {
            raw_links: Vec::new(),
            failures: vec![NodeImportFailure {
                source: "Mihomo YAML".to_string(),
                reason: "YAML subscription contains no proxy entries".to_string(),
            }],
        });
    }
    if proxies.len() > MAX_IMPORT_SUBSCRIPTION_NODES {
        return Err(AppError::BadRequest(format!(
            "at most {MAX_IMPORT_SUBSCRIPTION_NODES} nodes can be imported at once"
        )));
    }

    let mut links = Vec::new();
    let mut failures = Vec::new();
    for proxy in proxies {
        let Some(mapping) = proxy.as_mapping() else {
            failures.push(NodeImportFailure {
                source: "Mihomo YAML proxy".to_string(),
                reason: "proxy entry must be a mapping".to_string(),
            });
            continue;
        };
        match mihomo_proxy_to_raw_link(mapping) {
            Ok(link) => links.push(link),
            Err(reason) => failures.push(NodeImportFailure {
                source: mihomo_proxy_source(mapping),
                reason,
            }),
        }
    }
    Ok(ExtractedSubscription {
        raw_links: links,
        failures,
    })
}

fn extract_json_subscription(body: &str) -> Result<Option<ExtractedSubscription>, AppError> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Ok(None);
    };

    let mut raw_links = Vec::new();
    collect_json_raw_links(&value, &mut raw_links);
    if !raw_links.is_empty() {
        raw_links.sort();
        raw_links.dedup();
        return Ok(Some(ExtractedSubscription {
            raw_links,
            failures: Vec::new(),
        }));
    }

    let Some(servers) = value.get("servers").and_then(serde_json::Value::as_array) else {
        return Ok(Some(ExtractedSubscription {
            raw_links: Vec::new(),
            failures: vec![NodeImportFailure {
                source: "JSON subscription".to_string(),
                reason: "JSON subscription does not contain supported node links or SIP008 servers"
                    .to_string(),
            }],
        }));
    };
    if servers.is_empty() {
        return Ok(Some(ExtractedSubscription {
            raw_links: Vec::new(),
            failures: vec![NodeImportFailure {
                source: "SIP008 subscription".to_string(),
                reason: "SIP008 subscription contains no servers".to_string(),
            }],
        }));
    }

    let mut failures = Vec::new();
    for (index, server) in servers.iter().enumerate() {
        match sip008_server_to_raw_link(server) {
            Ok(link) => raw_links.push(link),
            Err(reason) => failures.push(NodeImportFailure {
                source: format!("SIP008 server #{}", index + 1),
                reason,
            }),
        }
    }
    Ok(Some(ExtractedSubscription {
        raw_links,
        failures,
    }))
}

fn collect_json_raw_links(value: &serde_json::Value, links: &mut Vec<String>) {
    match value {
        serde_json::Value::String(value) if is_supported_raw_link(value.trim()) => {
            links.push(value.trim().to_string());
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_json_raw_links(value, links);
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                collect_json_raw_links(value, links);
            }
        }
        _ => {}
    }
}

fn sip008_server_to_raw_link(server: &serde_json::Value) -> Result<String, String> {
    let object = server
        .as_object()
        .ok_or_else(|| "SIP008 server entry must be an object".to_string())?;
    let address = object
        .get("server")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "SIP008 server is missing server".to_string())?;
    let port = object
        .get("server_port")
        .or_else(|| object.get("port"))
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value > 0 && *value <= u16::MAX as u64)
        .ok_or_else(|| "SIP008 server is missing a valid server_port".to_string())?;
    let method = object
        .get("method")
        .or_else(|| object.get("cipher"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "SIP008 server is missing method".to_string())?;
    let password = object
        .get("password")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "SIP008 server is missing password".to_string())?;
    let name = object
        .get("remarks")
        .or_else(|| object.get("name"))
        .or_else(|| object.get("id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Shadowsocks");
    let credentials = general_purpose::URL_SAFE_NO_PAD.encode(format!("{method}:{password}"));
    let plugin = object
        .get("plugin")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|plugin| {
            let options = object
                .get("plugin_opts")
                .or_else(|| object.get("plugin-opts"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty());
            options
                .map(|options| format!("{plugin};{options}"))
                .unwrap_or_else(|| plugin.to_string())
        });
    let host = if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]")
    } else {
        address.to_string()
    };
    let query = plugin
        .map(|plugin| format!("?plugin={}", encode_uri_component(&plugin)))
        .unwrap_or_default();
    Ok(format!(
        "ss://{credentials}@{host}:{port}{query}#{}",
        encode_uri_component(name)
    ))
}

fn describe_unrecognized_subscription(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "upstream returned an empty subscription".to_string();
    }
    let lowercase = trimmed
        .chars()
        .take(256)
        .collect::<String>()
        .to_ascii_lowercase();
    if lowercase.starts_with("<!doctype html") || lowercase.starts_with("<html") {
        return "upstream returned HTML instead of a node subscription".to_string();
    }
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return "upstream returned an unsupported JSON subscription format".to_string();
    }
    if let Some(mapping) = serde_yaml::from_str::<Value>(trimmed)
        .ok()
        .and_then(|value| value.as_mapping().cloned())
    {
        for key in ["proxies", "payload"] {
            if let Some(value) = mapping.get(Value::String(key.to_string()))
                && !value.is_sequence()
            {
                return format!(
                    "YAML {key} must be a node list; upstream returned a node-free client profile"
                );
            }
        }
        return "YAML subscription is missing a proxies or payload node list".to_string();
    }
    "subscription format is not recognized after Base64 decoding".to_string()
}

fn mihomo_proxy_source(proxy: &Mapping) -> String {
    let name = yaml_string(proxy, "name").unwrap_or("unnamed");
    let proxy_type = yaml_string(proxy, "type").unwrap_or("unknown");
    truncate_source(&format!("Mihomo YAML: {name} ({proxy_type})"))
}

fn mihomo_proxy_to_raw_link(proxy: &Mapping) -> Result<String, String> {
    let proxy_type = yaml_string(proxy, "type")
        .ok_or_else(|| "mihomo proxy missing type".to_string())?
        .to_ascii_lowercase();
    let converted = match proxy_type.as_str() {
        "ss" | "shadowsocks" => mihomo_shadowsocks_to_uri(proxy),
        "vmess" => mihomo_vmess_to_uri(proxy),
        "vless" => mihomo_vless_to_uri(proxy),
        "trojan" => mihomo_trojan_to_uri(proxy),
        "hysteria2" | "hy2" => mihomo_hysteria2_to_uri(proxy),
        "tuic" => mihomo_tuic_to_uri(proxy),
        "wireguard" | "wg" => {
            if yaml_sequence_len(proxy, "peers").is_some_and(|count| count > 1) {
                return Err(
                    "mihomo wireguard multi-peer nodes cannot be represented safely".to_string(),
                );
            }
            mihomo_wireguard_to_uri(proxy)
        }
        "anytls" | "any-tls" => mihomo_anytls_to_uri(proxy),
        _ => {
            return Err(format!("unsupported mihomo proxy type: {proxy_type}"));
        }
    };

    converted.ok_or_else(|| format!("mihomo {proxy_type} proxy is missing required fields"))
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

fn mihomo_vmess_to_uri(proxy: &Mapping) -> Option<String> {
    let uuid = yaml_string(proxy, "uuid")?;
    let server = yaml_string(proxy, "server")?;
    let port = yaml_i64(proxy, "port")?;
    let name = yaml_string(proxy, "name").unwrap_or("vmess");
    let network = yaml_string(proxy, "network").unwrap_or("tcp");
    let mut payload = serde_json::Map::new();
    payload.insert("v".to_string(), serde_json::json!("2"));
    payload.insert("ps".to_string(), serde_json::json!(name));
    payload.insert("add".to_string(), serde_json::json!(server));
    payload.insert("port".to_string(), serde_json::json!(port));
    payload.insert("id".to_string(), serde_json::json!(uuid));
    payload.insert(
        "aid".to_string(),
        serde_json::json!(yaml_i64(proxy, "alterId").unwrap_or(0)),
    );
    payload.insert(
        "scy".to_string(),
        serde_json::json!(yaml_string(proxy, "cipher").unwrap_or("auto")),
    );
    payload.insert("net".to_string(), serde_json::json!(network));
    payload.insert(
        "tls".to_string(),
        serde_json::json!(if yaml_bool(proxy, "tls").unwrap_or(false) {
            "tls"
        } else {
            ""
        }),
    );

    if let Some(sni) = yaml_string(proxy, "servername").or_else(|| yaml_string(proxy, "sni")) {
        payload.insert("sni".to_string(), serde_json::json!(sni));
    }
    if let Some(fp) = yaml_string(proxy, "client-fingerprint") {
        payload.insert("fp".to_string(), serde_json::json!(fp));
    }
    if let Some(insecure) = yaml_bool(proxy, "skip-cert-verify") {
        payload.insert("insecure".to_string(), serde_json::json!(insecure));
    }
    if let Some(udp) = yaml_bool(proxy, "udp") {
        payload.insert("udp".to_string(), serde_json::json!(udp));
    }
    if let Some(alpn) = yaml_string_or_csv(proxy, "alpn") {
        payload.insert("alpn".to_string(), serde_json::json!(alpn));
    }
    if network.eq_ignore_ascii_case("ws")
        && let Some(ws_opts) = yaml_mapping(proxy, "ws-opts")
    {
        if let Some(path) = yaml_string(ws_opts, "path") {
            payload.insert("path".to_string(), serde_json::json!(path));
        }
        if let Some(headers) = yaml_mapping(ws_opts, "headers")
            && let Some(host) =
                yaml_string(headers, "Host").or_else(|| yaml_string(headers, "host"))
        {
            payload.insert("host".to_string(), serde_json::json!(host));
        }
    }
    if network.eq_ignore_ascii_case("grpc")
        && let Some(grpc_opts) = yaml_mapping(proxy, "grpc-opts")
        && let Some(service_name) = yaml_string(grpc_opts, "grpc-service-name")
    {
        payload.insert(
            "grpc-service-name".to_string(),
            serde_json::json!(service_name),
        );
    }

    let encoded = general_purpose::STANDARD.encode(serde_json::to_vec(&payload).ok()?);
    Some(format!("vmess://{encoded}"))
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
    if let Some(alpn) = yaml_string_or_csv(proxy, "alpn") {
        push_param(&mut params, "alpn", &alpn);
    }
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
    push_param(
        &mut params,
        "security",
        if yaml_mapping(proxy, "reality-opts").is_some() {
            "reality"
        } else {
            "tls"
        },
    );
    push_optional_param(
        &mut params,
        "sni",
        yaml_string(proxy, "servername").or_else(|| yaml_string(proxy, "sni")),
    );
    push_optional_param(&mut params, "fp", yaml_string(proxy, "client-fingerprint"));
    push_optional_param(
        &mut params,
        "certificate-fingerprint",
        yaml_string(proxy, "fingerprint"),
    );
    if let Some(alpn) = yaml_string_or_csv(proxy, "alpn") {
        push_param(&mut params, "alpn", &alpn);
    }
    if let Some(insecure) = yaml_bool(proxy, "skip-cert-verify") {
        push_param(&mut params, "insecure", if insecure { "1" } else { "0" });
    }
    if let Some(udp) = yaml_bool(proxy, "udp") {
        push_param(&mut params, "udp", if udp { "true" } else { "false" });
    }
    push_optional_param(
        &mut params,
        "name-cert-verify",
        yaml_string(proxy, "name-cert-verify"),
    );
    push_optional_param(&mut params, "ip-version", yaml_string(proxy, "ip-version"));
    for key in ["tfo", "mptcp"] {
        if let Some(value) = yaml_bool(proxy, key) {
            push_param(&mut params, key, if value { "true" } else { "false" });
        }
    }
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
        if let Some(value) = yaml_json_string(proxy, "ws-opts") {
            push_param(&mut params, "ws-opts", &value);
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
        if let Some(value) = yaml_json_string(proxy, "grpc-opts") {
            push_param(&mut params, "grpc-opts", &value);
        }
    }
    if let Some(reality) = yaml_mapping(proxy, "reality-opts") {
        push_optional_param(&mut params, "pbk", yaml_string(reality, "public-key"));
        push_optional_param(&mut params, "sid", yaml_string(reality, "short-id"));
    }
    for key in [
        "reality-opts",
        "ech-opts",
        "shadow-tls-opts",
        "restls-opts",
        "jls-opts",
        "ss-opts",
        "smux",
    ] {
        if let Some(value) = yaml_json_string(proxy, key) {
            push_param(&mut params, key, &value);
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

fn mihomo_tuic_to_uri(proxy: &Mapping) -> Option<String> {
    let server = yaml_string(proxy, "server")?;
    let port = yaml_i64(proxy, "port")?;
    let name = yaml_string(proxy, "name").unwrap_or("tuic");
    let token = yaml_string(proxy, "token");
    let uuid = yaml_string(proxy, "uuid");
    let password = yaml_string(proxy, "password");
    let authority = if let Some(token) = token {
        let _ = token;
        String::new()
    } else {
        format!(
            "{}:{}@",
            encode_uri_userinfo(uuid?),
            encode_uri_userinfo(password?)
        )
    };
    let mut params = Vec::new();
    push_optional_param(&mut params, "token", token);
    push_optional_param(
        &mut params,
        "sni",
        yaml_string(proxy, "sni").or_else(|| yaml_string(proxy, "servername")),
    );
    if let Some(insecure) = yaml_bool(proxy, "skip-cert-verify") {
        push_param(&mut params, "insecure", if insecure { "1" } else { "0" });
    }
    if let Some(alpn) = yaml_string_or_csv(proxy, "alpn") {
        push_param(&mut params, "alpn", &alpn);
    }
    for (query_key, yaml_key) in [
        ("ip", "ip"),
        ("congestion-controller", "congestion-controller"),
        ("udp-relay-mode", "udp-relay-mode"),
        ("bbr-profile", "bbr-profile"),
        ("name-cert-verify", "name-cert-verify"),
    ] {
        push_optional_param(&mut params, query_key, yaml_string(proxy, yaml_key));
    }
    for (query_key, yaml_key) in [
        ("heartbeat-interval", "heartbeat-interval"),
        ("request-timeout", "request-timeout"),
        ("max-udp-relay-packet-size", "max-udp-relay-packet-size"),
        ("max-open-streams", "max-open-streams"),
    ] {
        if let Some(value) = yaml_i64(proxy, yaml_key) {
            push_param(&mut params, query_key, &value.to_string());
        }
    }
    for (query_key, yaml_key) in [
        ("disable-sni", "disable-sni"),
        ("reduce-rtt", "reduce-rtt"),
        ("fast-open", "fast-open"),
    ] {
        if let Some(value) = yaml_bool(proxy, yaml_key) {
            push_param(&mut params, query_key, if value { "true" } else { "false" });
        }
    }

    Some(format!(
        "tuic://{authority}{server}:{port}?{}#{}",
        params.join("&"),
        encode_uri_component(name)
    ))
}

fn mihomo_wireguard_to_uri(proxy: &Mapping) -> Option<String> {
    let peer = yaml_first_mapping(proxy, "peers");
    let public_key = yaml_string(proxy, "public-key")
        .or_else(|| peer.and_then(|mapping| yaml_string(mapping, "public-key")))?;
    let server = yaml_string(proxy, "server")
        .or_else(|| peer.and_then(|mapping| yaml_string(mapping, "server")))?;
    let port =
        yaml_i64(proxy, "port").or_else(|| peer.and_then(|mapping| yaml_i64(mapping, "port")))?;
    let name = yaml_string(proxy, "name").unwrap_or("wireguard");
    let mut params = Vec::new();
    push_optional_param(
        &mut params,
        "private-key",
        yaml_string(proxy, "private-key"),
    );
    push_optional_param(
        &mut params,
        "pre-shared-key",
        yaml_string(proxy, "pre-shared-key")
            .or_else(|| peer.and_then(|mapping| yaml_string(mapping, "pre-shared-key"))),
    );
    let addresses = [
        yaml_string_or_csv(proxy, "ip"),
        yaml_string_or_csv(proxy, "ipv6"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(",");
    if !addresses.is_empty() {
        let (ipv6, ipv4): (Vec<_>, Vec<_>) = addresses
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .partition(|value| value.contains(':'));
        if !ipv4.is_empty() {
            push_param(&mut params, "ip", &ipv4.join(","));
        }
        if !ipv6.is_empty() {
            push_param(&mut params, "ipv6", &ipv6.join(","));
        }
    }
    for key in ["allowed-ips", "reserved"] {
        let value = yaml_string_or_csv(proxy, key)
            .or_else(|| peer.and_then(|mapping| yaml_string_or_csv(mapping, key)));
        if let Some(value) = value {
            push_param(&mut params, key, &value);
        }
    }
    push_optional_param(&mut params, "dns", yaml_string(proxy, "dns"));
    if let Some(dns) = yaml_string_or_csv(proxy, "dns") {
        params.retain(|value| !value.starts_with("dns="));
        push_param(&mut params, "dns", &dns);
    }
    if let Some(keepalive) = yaml_i64(proxy, "persistent-keepalive") {
        push_param(&mut params, "persistent-keepalive", &keepalive.to_string());
    }
    if let Some(remote_dns) = yaml_bool(proxy, "remote-dns-resolve") {
        push_param(
            &mut params,
            "remote-dns-resolve",
            if remote_dns { "true" } else { "false" },
        );
    }
    if let Some(mtu) = yaml_i64(proxy, "mtu") {
        push_param(&mut params, "mtu", &mtu.to_string());
    }
    if let Some(udp) = yaml_bool(proxy, "udp") {
        push_param(&mut params, "udp", if udp { "true" } else { "false" });
    }

    Some(format!(
        "wireguard://{}@{}:{}?{}#{}",
        encode_uri_userinfo(public_key),
        server,
        port,
        params.join("&"),
        encode_uri_component(name)
    ))
}

fn mihomo_anytls_to_uri(proxy: &Mapping) -> Option<String> {
    let password = yaml_string(proxy, "password")?;
    let server = yaml_string(proxy, "server")?;
    let port = yaml_i64(proxy, "port")?;
    let name = yaml_string(proxy, "name").unwrap_or("anytls");
    let mut params = Vec::new();
    push_optional_param(
        &mut params,
        "sni",
        yaml_string(proxy, "sni").or_else(|| yaml_string(proxy, "servername")),
    );
    if let Some(alpn) = yaml_string_or_csv(proxy, "alpn") {
        push_param(&mut params, "alpn", &alpn);
    }
    push_optional_param(&mut params, "fp", yaml_string(proxy, "client-fingerprint"));
    if let Some(insecure) = yaml_bool(proxy, "skip-cert-verify") {
        push_param(&mut params, "insecure", if insecure { "1" } else { "0" });
    }
    if let Some(udp) = yaml_bool(proxy, "udp") {
        push_param(&mut params, "udp", if udp { "true" } else { "false" });
    }
    push_optional_param(
        &mut params,
        "name-cert-verify",
        yaml_string(proxy, "name-cert-verify"),
    );
    for key in ["shadow-tls-opts", "restls-opts", "jls-opts"] {
        if let Some(value) = yaml_json_string(proxy, key) {
            push_param(&mut params, key, &value);
        }
    }
    for key in [
        "idle-session-check-interval",
        "idle-session-timeout",
        "min-idle-session",
    ] {
        if let Some(value) = yaml_i64(proxy, key) {
            push_param(&mut params, key, &value.to_string());
        }
    }

    Some(format!(
        "anytls://{}@{}:{}?{}#{}",
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

fn yaml_first_mapping<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_sequence)
        .and_then(|items| items.first())
        .and_then(Value::as_mapping)
}

fn yaml_sequence_len(mapping: &Mapping, key: &str) -> Option<usize> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_sequence)
        .map(Vec::len)
}

fn yaml_json_string(mapping: &Mapping, key: &str) -> Option<String> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(|value| serde_json::to_string(value).ok())
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
    let subscription_count =
        node_repo::count_subscriptions_using_node(&state.db, existing.id).await?;
    if subscription_count > 0 {
        return Err(AppError::BadRequest(format!(
            "node is used by {subscription_count} subscription(s); remove it from subscriptions or disable it instead"
        )));
    }
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
        MIHOMO_SUBSCRIPTION_USER_AGENT, check_mihomo_conversion_fidelity,
        extract_subscription_links, is_subscription_info_name, mihomo_proxy_to_raw_link,
        sanitize_mihomo_profile_yaml, upstream_template_name_parts,
    };
    use crate::services::url_safety::validate_public_http_url;
    use base64::{Engine as _, engine::general_purpose};
    use serde_yaml::Value;
    use std::collections::HashSet;

    #[test]
    fn extracts_clash_provider_payload_yaml() {
        let yaml = r#"
payload:
  - name: Provider SS
    type: ss
    server: ss.example.com
    port: 8388
    cipher: aes-256-gcm
    password: secret
"#;

        let extracted = extract_subscription_links(yaml).expect("provider YAML should extract");
        assert_eq!(extracted.raw_links.len(), 1);
        assert!(extracted.failures.is_empty());
        let parsed =
            crate::services::protocol_parser_service::parse_raw_link(&extracted.raw_links[0], None)
                .expect("provider node should parse");
        assert_eq!(parsed.protocol.as_str(), "shadowsocks");
        assert_eq!(parsed.name, "Provider SS");
    }

    #[test]
    fn extracts_sip008_shadowsocks_json() {
        let json = r#"{
          "version": 1,
          "servers": [{
            "id": "sip008-node",
            "remarks": "SIP008 SS",
            "server": "2001:db8::1",
            "server_port": 8388,
            "method": "aes-256-gcm",
            "password": "secret",
            "plugin": "v2ray-plugin",
            "plugin_opts": "mode=websocket;host=edge.example.com"
          }]
        }"#;

        let extracted = extract_subscription_links(json).expect("SIP008 JSON should extract");
        assert_eq!(extracted.raw_links.len(), 1);
        assert!(extracted.failures.is_empty());
        let parsed =
            crate::services::protocol_parser_service::parse_raw_link(&extracted.raw_links[0], None)
                .expect("SIP008 node should parse");
        assert_eq!(parsed.name, "SIP008 SS");
        assert_eq!(parsed.server, "2001:db8::1");
        assert_eq!(parsed.port, 8388);
        assert_eq!(
            parsed
                .settings
                .get("plugin")
                .and_then(|value| value.as_str()),
            Some("v2ray-plugin;mode=websocket;host=edge.example.com")
        );
    }

    #[test]
    fn extracts_double_base64_encoded_node_list() {
        let raw =
            "vless://4c374a1d-e334-4ec1-b010-489bfa360ba9@example.com:443?security=tls#Nested";
        let once = general_purpose::STANDARD.encode(raw);
        let twice = general_purpose::STANDARD.encode(once);

        let extracted = extract_subscription_links(&twice).expect("nested Base64 should extract");
        assert_eq!(extracted.raw_links, vec![raw]);
    }

    #[test]
    fn diagnoses_html_subscription_responses_without_echoing_body() {
        let extracted = extract_subscription_links(
            "<!doctype html><html><body>token=secret-value</body></html>",
        )
        .expect("HTML should produce a diagnostic");

        assert!(extracted.raw_links.is_empty());
        assert_eq!(extracted.failures.len(), 1);
        assert!(extracted.failures[0].reason.contains("returned HTML"));
        assert!(!extracted.failures[0].reason.contains("secret-value"));
    }

    #[test]
    fn uses_plain_mihomo_user_agent_for_format_negotiation() {
        assert_eq!(MIHOMO_SUBSCRIPTION_USER_AGENT, "mihomo/1.19.10");
        assert!(!MIHOMO_SUBSCRIPTION_USER_AGENT.contains("Clash"));
        assert!(!MIHOMO_SUBSCRIPTION_USER_AGENT.contains(' '));
    }

    #[test]
    fn extracts_stable_identity_from_upstream_template_revisions() {
        assert_eq!(
            upstream_template_name_parts("Upstream Mihomo GAT 0f512f8a #12"),
            Some((
                "Upstream Mihomo GAT 0f512f8a".to_string(),
                "0f512f8a".to_string()
            ))
        );
        assert_eq!(
            upstream_template_name_parts("Upstream Mihomo GAT 0F512F8A"),
            Some((
                "Upstream Mihomo GAT 0F512F8A".to_string(),
                "0F512F8A".to_string()
            ))
        );
        assert!(upstream_template_name_parts("Built-in Mihomo Policy").is_none());
        assert!(upstream_template_name_parts("Upstream Mihomo missing-digest").is_none());
    }

    #[test]
    fn filters_provider_navigation_page_nodes() {
        assert!(is_subscription_info_name("导航页：www.可乐云.net"));
        assert!(is_subscription_info_name(" 导航页: example.com "));
        assert!(!is_subscription_info_name("香港导航节点"));
    }

    #[test]
    fn diagnoses_node_free_mihomo_profiles() {
        let extracted = extract_subscription_links("proxies: {}\nrules: []")
            .expect("node-free profile should produce a diagnostic");

        assert!(extracted.raw_links.is_empty());
        assert_eq!(extracted.failures.len(), 1);
        assert!(
            extracted.failures[0]
                .reason
                .contains("node-free client profile")
        );
    }

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
    fn checks_hysteria2_across_supported_client_renderers_without_missing_fields() {
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

    #[test]
    fn extracts_all_currently_exportable_protocols_from_mihomo_yaml() {
        let yaml = r#"
proxies:
  - { name: SS, type: ss, server: ss.example.com, port: 8388, cipher: aes-256-gcm, password: secret }
  - { name: VMess, type: vmess, server: vmess.example.com, port: 443, uuid: 4c374a1d-e334-4ec1-b010-489bfa360ba9, alterId: 0, cipher: auto, network: ws, tls: true }
  - { name: VLESS, type: vless, server: vless.example.com, port: 443, uuid: 4c374a1d-e334-4ec1-b010-489bfa360ba9, encryption: none, tls: true }
  - { name: Trojan, type: trojan, server: trojan.example.com, port: 443, password: secret }
  - { name: HY2, type: hysteria2, server: hy2.example.com, port: 443, password: secret }
  - { name: TUIC, type: tuic, server: tuic.example.com, port: 443, uuid: 4c374a1d-e334-4ec1-b010-489bfa360ba9, password: secret }
  - name: WireGuard
    type: wireguard
    server: wg.example.com
    port: 51820
    public-key: public-key-value
    private-key: private-key-value
    ip: [10.0.0.2/32, 2001:db8::2/128]
  - name: AnyTLS
    type: anytls
    server: anytls.example.com
    port: 443
    password: secret
    sni: edge.example.com
    alpn: [h2, http/1.1]
    client-fingerprint: chrome
  - { name: SSH, type: ssh, server: ssh.example.com, port: 22, username: root, password: secret }
"#;

        let extracted = extract_subscription_links(yaml).expect("mihomo YAML should extract");
        let nodes = extracted
            .raw_links
            .iter()
            .map(|link| {
                let parsed = crate::services::protocol_parser_service::parse_raw_link(link, None)
                    .expect("converted URI should parse");
                let node = super::fidelity_node_from_parsed(link.clone(), parsed);
                crate::services::export_service::render_mihomo_proxy(&node)
                    .expect("converted node should render back to Mihomo");
                node
            })
            .collect::<Vec<_>>();
        let protocols = nodes
            .iter()
            .map(|node| node.protocol.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(nodes.len(), 8);
        assert_eq!(
            protocols,
            HashSet::from([
                "shadowsocks",
                "vmess",
                "vless",
                "trojan",
                "hysteria2",
                "tuic",
                "wireguard",
                "anytls",
            ])
        );
        assert_eq!(extracted.failures.len(), 1);
        assert!(extracted.failures[0].source.contains("SSH (ssh)"));
        assert!(
            extracted.failures[0]
                .reason
                .contains("unsupported mihomo proxy type")
        );
    }

    #[test]
    fn preserves_mihomo_anytls_fields_during_yaml_conversion() {
        let yaml = r#"
name: AnyTLS Full
type: anytls
server: anytls.example.com
port: 443
password: secret
client-fingerprint: chrome
udp: true
idle-session-check-interval: 30
idle-session-timeout: 45
min-idle-session: 2
sni: edge.example.com
alpn: [h2, http/1.1]
skip-cert-verify: true
name-cert-verify: cert.example.com
shadow-tls-opts:
  version: 3
  password: shadow-secret
"#;
        let proxy = serde_yaml::from_str::<Value>(yaml).unwrap();
        let mapping = proxy.as_mapping().unwrap();
        let link = mihomo_proxy_to_raw_link(mapping).expect("anytls proxy should convert");
        let parsed = crate::services::protocol_parser_service::parse_raw_link(&link, None)
            .expect("converted anytls URI should parse");

        assert_eq!(parsed.protocol.as_str(), "anytls");
        assert_eq!(parsed.name, "AnyTLS Full");
        assert_eq!(parsed.settings["password"], "secret");
        assert_eq!(parsed.settings["fp"], "chrome");
        assert_eq!(parsed.settings["alpn"], "h2,http/1.1");
        assert_eq!(parsed.settings["idle-session-timeout"], "45");
        assert_eq!(parsed.settings["name-cert-verify"], "cert.example.com");
        assert!(
            parsed.settings["shadow-tls-opts"]
                .as_str()
                .is_some_and(|value| value.contains("shadow-secret"))
        );
        let node = super::fidelity_node_from_parsed(link, parsed);
        let rendered = crate::services::export_service::render_mihomo_proxy(&node)
            .expect("anytls node should render back to Mihomo");
        let shadow_tls = rendered
            .get(Value::String("shadow-tls-opts".to_string()))
            .and_then(Value::as_mapping)
            .expect("shadow-tls-opts should be preserved");
        assert_eq!(
            shadow_tls
                .get(Value::String("password".to_string()))
                .and_then(Value::as_str),
            Some("shadow-secret")
        );
    }

    #[test]
    fn preserves_mihomo_trojan_ws_fields_and_password() {
        let yaml = r#"
name: Trojan WS Full
type: trojan
server: trojan.example.com
port: 443
password: "pa:ss@word%"
network: ws
sni: edge.example.com
client-fingerprint: chrome
fingerprint: certificate-sha256
alpn: [http/1.1]
skip-cert-verify: false
name-cert-verify: cert.example.com
ip-version: ipv4
tfo: true
mptcp: false
udp: true
ws-opts:
  path: /trojan?ed=2048
  headers:
    Host: ws.example.com
  max-early-data: 2048
  early-data-header-name: Sec-WebSocket-Protocol
smux:
  enabled: true
  protocol: smux
"#;
        let proxy = serde_yaml::from_str::<Value>(yaml).unwrap();
        let mapping = proxy.as_mapping().unwrap();
        let link = mihomo_proxy_to_raw_link(mapping).expect("trojan proxy should convert");
        let parsed = crate::services::protocol_parser_service::parse_raw_link(&link, None)
            .expect("converted trojan URI should parse");

        assert_eq!(parsed.settings["password"], "pa:ss@word%");
        assert_eq!(parsed.settings["sni"], "edge.example.com");
        let node = super::fidelity_node_from_parsed(link, parsed);
        let rendered = crate::services::export_service::render_mihomo_proxy(&node)
            .expect("trojan node should render back to Mihomo");

        assert_eq!(
            rendered
                .get(Value::String("sni".to_string()))
                .and_then(Value::as_str),
            Some("edge.example.com")
        );
        assert!(
            rendered
                .get(Value::String("servername".to_string()))
                .is_none()
        );
        assert_eq!(
            rendered
                .get(Value::String("password".to_string()))
                .and_then(Value::as_str),
            Some("pa:ss@word%")
        );
        let ws_opts = rendered
            .get(Value::String("ws-opts".to_string()))
            .and_then(Value::as_mapping)
            .expect("ws-opts should be preserved");
        assert_eq!(
            ws_opts
                .get(Value::String("max-early-data".to_string()))
                .and_then(Value::as_i64),
            Some(2048)
        );
        assert!(
            rendered
                .get(Value::String("smux".to_string()))
                .and_then(Value::as_mapping)
                .is_some()
        );
    }

    #[test]
    fn preserves_mihomo_trojan_reality_options() {
        let yaml = r#"
name: Trojan Reality
type: trojan
server: reality.example.com
port: 443
password: secret
sni: cover.example.com
client-fingerprint: chrome
reality-opts:
  public-key: public-key-value
  short-id: abcd1234
  support-x25519mlkem768: true
"#;
        let proxy = serde_yaml::from_str::<Value>(yaml).unwrap();
        let mapping = proxy.as_mapping().unwrap();
        let link = mihomo_proxy_to_raw_link(mapping).expect("reality trojan should convert");
        let parsed = crate::services::protocol_parser_service::parse_raw_link(&link, None)
            .expect("converted reality trojan URI should parse");

        assert_eq!(parsed.settings["security"], "reality");
        let node = super::fidelity_node_from_parsed(link, parsed);
        let rendered = crate::services::export_service::render_mihomo_proxy(&node)
            .expect("reality trojan should render back to Mihomo");
        let reality = rendered
            .get(Value::String("reality-opts".to_string()))
            .and_then(Value::as_mapping)
            .expect("reality-opts should be preserved");
        assert_eq!(
            reality
                .get(Value::String("public-key".to_string()))
                .and_then(Value::as_str),
            Some("public-key-value")
        );
        assert_eq!(
            reality
                .get(Value::String("support-x25519mlkem768".to_string()))
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn converts_mihomo_tuic_v4_token_proxy() {
        let yaml = r#"
name: TUIC v4
type: tuic
server: tuic.example.com
port: 443
token: token-secret
congestion-controller: bbr
udp-relay-mode: native
reduce-rtt: true
"#;
        let proxy = serde_yaml::from_str::<Value>(yaml).unwrap();
        let mapping = proxy.as_mapping().unwrap();
        let link = mihomo_proxy_to_raw_link(mapping).expect("tuic v4 proxy should convert");
        let parsed = crate::services::protocol_parser_service::parse_raw_link(&link, None)
            .expect("converted tuic v4 URI should parse");

        assert_eq!(parsed.protocol.as_str(), "tuic");
        assert_eq!(parsed.settings["token"], "token-secret");
        assert_eq!(parsed.settings["congestion-controller"], "bbr");
        assert_eq!(parsed.settings["reduce-rtt"], "true");
    }

    #[test]
    fn rejects_mihomo_wireguard_multi_peer_without_silent_truncation() {
        let yaml = r#"
name: WireGuard Multi Peer
type: wireguard
private-key: private-key-value
ip: 10.0.0.2/32
peers:
  - { server: one.example.com, port: 51820, public-key: public-key-one, allowed-ips: [0.0.0.0/1] }
  - { server: two.example.com, port: 51820, public-key: public-key-two, allowed-ips: [128.0.0.0/1] }
"#;
        let proxy = serde_yaml::from_str::<Value>(yaml).unwrap();
        let mapping = proxy.as_mapping().unwrap();
        let error = mihomo_proxy_to_raw_link(mapping)
            .expect_err("multi-peer wireguard must not be truncated to the first peer");

        assert!(error.contains("multi-peer"));
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
