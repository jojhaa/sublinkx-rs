use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpListener},
    path::PathBuf,
    process::Stdio,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::body::{Body, Bytes};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use serde_yaml::{Mapping, Value};
use tokio::{process::Command, sync::mpsc, time::sleep};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    domain::{node::NodeView, node_ip_probe::NodeIpProbeRecord},
    dto::node_ip_probes::{
        NodeIpProbeEvent, NodeIpProbeListResponse, NodeIpProbeRequest, NodeIpProbeResponse,
        RefreshNodeIpIntelligenceRequest, RefreshNodeIpIntelligenceResponse,
        UpdateNodeIpCountryRequest,
    },
    errors::AppError,
    repository::{node_ip_probe_repo, node_repo},
    state::{AppState, LatencyManualBeginError},
    utils::time::now_rfc3339,
};

use super::{export_service, ip_intelligence_service, node_service, settings_service};

const MAX_PROBE_NODES: usize = 200;
const IP_ECHO_URL: &str = "https://api64.ipify.org?format=json";
const IP_ECHO_PROVIDER: &str = "ipify";
const SELECTOR_NAME: &str = "IP-PROBE";

#[derive(Clone)]
struct PreparedNode {
    id: i64,
    node: NodeView,
    proxy_name: String,
    proxy: Mapping,
}

struct ProbeResult {
    ip: String,
    latency_ms: u32,
}

struct PreparedRun {
    binary: PathBuf,
    requested_ids: Vec<i64>,
    nodes: Vec<PreparedNode>,
    preflight_errors: HashMap<i64, String>,
    country_detection_auto_enabled: bool,
}

#[derive(Clone, Copy)]
enum ProbeRunMode {
    Manual,
    Background,
}

#[derive(Debug, Default)]
pub struct BackgroundProbeSummary {
    pub requested: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub batches: usize,
    pub cancelled: bool,
    pub busy: bool,
}

#[derive(Deserialize)]
struct IpifyResponse {
    ip: String,
}

pub async fn list(state: &AppState) -> Result<NodeIpProbeListResponse, AppError> {
    Ok(NodeIpProbeListResponse {
        code: "00000",
        data: node_ip_probe_repo::list(&state.db).await?,
    })
}

pub async fn update_country(
    state: &AppState,
    node_id: i64,
    payload: UpdateNodeIpCountryRequest,
) -> Result<NodeIpProbeResponse, AppError> {
    let ip = normalize_public_ip(&payload.ip)?;
    let country_code = payload.country_code.trim().to_ascii_uppercase();
    if country_code.len() != 2
        || !country_code
            .bytes()
            .all(|value| value.is_ascii_alphabetic())
    {
        return Err(AppError::BadRequest(
            "country_code must contain exactly two ASCII letters".to_string(),
        ));
    }
    let country_name = normalize_optional(&payload.country_name, 128, "country_name")?;
    let source = payload.source.trim();
    if source.is_empty() || source.len() > 64 || source.chars().any(char::is_control) {
        return Err(AppError::BadRequest(
            "source must contain 1 to 64 printable characters".to_string(),
        ));
    }

    let record = node_ip_probe_repo::update_country(
        &state.db,
        node_id,
        &ip,
        &country_code,
        country_name.as_deref(),
        source,
        &now_rfc3339(),
    )
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(
            "the submitted IP does not match the node's latest probe result".to_string(),
        )
    })?;
    state.clear_public_export_cache().await;

    Ok(NodeIpProbeResponse {
        code: "00000",
        data: record,
    })
}

pub async fn refresh_intelligence(
    state: &AppState,
    payload: RefreshNodeIpIntelligenceRequest,
) -> Result<RefreshNodeIpIntelligenceResponse, AppError> {
    validate_request(&NodeIpProbeRequest {
        ids: payload.ids.clone(),
    })?;
    if !ip_intelligence_service::configured(state) {
        return Err(AppError::BadRequest(
            "IP intelligence integration is disabled".to_string(),
        ));
    }
    ip_intelligence_service::check_connection(state)
        .await
        .map_err(AppError::BadRequest)?;

    let requested_ids = dedupe_ids(&payload.ids);
    let records = node_repo::find_by_ids(&state.db, &requested_ids).await?;
    let mut nodes = HashMap::new();
    for record in records {
        let view = NodeView::try_from(record).map_err(|_| AppError::Internal)?;
        nodes.insert(view.id, view);
    }
    let probes = node_ip_probe_repo::list(&state.db)
        .await?
        .into_iter()
        .map(|probe| (probe.node_id, probe))
        .collect::<HashMap<_, _>>();

    let mut updated = 0;
    let mut pending = 0;
    let mut failed = 0;
    let mut data = Vec::new();
    for id in requested_ids {
        let Some(node) = nodes.get(&id) else {
            failed += 1;
            continue;
        };
        let Some(ip) = probes
            .get(&id)
            .filter(|probe| probe.status == "ok")
            .and_then(|probe| probe.ip.as_deref())
        else {
            failed += 1;
            continue;
        };
        match ip_intelligence_service::sync_node(state, node, ip, None).await {
            Ok(record) => {
                if record.country_code.is_some() {
                    updated += 1;
                } else {
                    pending += 1;
                }
                data.push(record);
            }
            Err(_) => failed += 1,
        }
    }
    Ok(RefreshNodeIpIntelligenceResponse {
        code: "00000",
        updated,
        pending,
        failed,
        data,
    })
}

pub async fn stream(state: AppState, payload: NodeIpProbeRequest) -> Result<Body, AppError> {
    validate_request(&payload)?;
    begin_manual_run(&state).await?;

    let prepared = match prepare_run(&state, payload).await {
        Ok(prepared) => prepared,
        Err(error) => {
            state.finish_manual_latency_run().await;
            return Err(error);
        }
    };
    let (sender, receiver) = mpsc::channel::<Result<Bytes, Infallible>>(32);
    tokio::spawn(async move {
        run_stream(&state, prepared, sender, ProbeRunMode::Manual).await;
        state.finish_manual_latency_run().await;
    });

    Ok(Body::from_stream(ReceiverStream::new(receiver)))
}

pub async fn run_background(
    state: &AppState,
    ids: Vec<i64>,
) -> Result<BackgroundProbeSummary, AppError> {
    let requested_ids = dedupe_ids(&ids);
    if requested_ids.is_empty() {
        return Ok(BackgroundProbeSummary::default());
    }
    if !state.try_begin_auto_latency_run().await {
        return Ok(BackgroundProbeSummary {
            requested: requested_ids.len(),
            busy: true,
            ..BackgroundProbeSummary::default()
        });
    }

    let result = run_background_batches(state, &requested_ids).await;
    state.finish_auto_latency_run().await;
    result
}

async fn run_background_batches(
    state: &AppState,
    requested_ids: &[i64],
) -> Result<BackgroundProbeSummary, AppError> {
    let mut summary = BackgroundProbeSummary {
        requested: requested_ids.len(),
        ..BackgroundProbeSummary::default()
    };
    for batch in requested_ids.chunks(MAX_PROBE_NODES) {
        if state.is_auto_latency_cancel_requested().await {
            summary.cancelled = true;
            break;
        }
        let payload = NodeIpProbeRequest {
            ids: batch.to_vec(),
        };
        validate_request(&payload)?;
        let prepared = prepare_run(state, payload).await?;
        let (sender, mut receiver) = mpsc::channel::<Result<Bytes, Infallible>>(32);
        let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
        let batch_summary = run_stream(state, prepared, sender, ProbeRunMode::Background).await;
        let _ = drain.await;
        summary.succeeded += batch_summary.succeeded;
        summary.failed += batch_summary.failed;
        summary.batches += 1;
        if batch_summary.cancelled {
            summary.cancelled = true;
            break;
        }
    }
    Ok(summary)
}

async fn prepare_run(
    state: &AppState,
    payload: NodeIpProbeRequest,
) -> Result<PreparedRun, AppError> {
    let settings = settings_service::load_settings(state).await?;
    let binary = node_service::ensure_mihomo_core_ready(&settings).await?;
    let requested_ids = dedupe_ids(&payload.ids);
    let records = node_repo::find_by_ids(&state.db, &requested_ids).await?;
    let mut records_by_id = records
        .into_iter()
        .map(|record| (record.id, record))
        .collect::<HashMap<_, _>>();
    let mut nodes = Vec::with_capacity(requested_ids.len());
    let mut preflight_errors = HashMap::new();

    for id in &requested_ids {
        let Some(record) = records_by_id.remove(id) else {
            preflight_errors.insert(*id, "node not found".to_string());
            continue;
        };
        let view = match NodeView::try_from(record) {
            Ok(view) => view,
            Err(error) => {
                preflight_errors.insert(*id, truncate_message(&error.to_string()));
                continue;
            }
        };
        match render_proxy(view) {
            Ok(node) => nodes.push(node),
            Err(message) => {
                preflight_errors.insert(*id, truncate_message(&message));
            }
        }
    }

    Ok(PreparedRun {
        binary,
        requested_ids,
        nodes,
        preflight_errors,
        country_detection_auto_enabled: settings.country_detection_auto_enabled,
    })
}

async fn run_stream(
    state: &AppState,
    prepared: PreparedRun,
    sender: mpsc::Sender<Result<Bytes, Infallible>>,
    mode: ProbeRunMode,
) -> BackgroundProbeSummary {
    if !send_event(
        &sender,
        &NodeIpProbeEvent::JobStarted {
            total_nodes: prepared.requested_ids.len(),
            provider: IP_ECHO_PROVIDER,
        },
    )
    .await
    {
        return BackgroundProbeSummary {
            requested: prepared.requested_ids.len(),
            cancelled: true,
            ..BackgroundProbeSummary::default()
        };
    }

    let mut succeeded = 0_usize;
    let mut failed = 0_usize;
    for (id, message) in &prepared.preflight_errors {
        if persist_and_send_failure(state, &sender, *id, message).await {
            failed += 1;
        }
    }

    let mut cancelled = false;
    if !prepared.nodes.is_empty() {
        match run_mihomo_probes(
            state,
            &prepared.nodes,
            &prepared.binary,
            &sender,
            mode,
            prepared.country_detection_auto_enabled,
        )
        .await
        {
            Ok(outcomes) => {
                for outcome in outcomes {
                    if outcome.status == "ok" {
                        succeeded += 1;
                    } else if outcome.status == "cancelled" {
                        cancelled = true;
                    } else {
                        failed += 1;
                    }
                }
            }
            Err(message) if message == "probe cancelled" => cancelled = true,
            Err(message) => {
                for node in &prepared.nodes {
                    if persist_and_send_failure(state, &sender, node.id, &message).await {
                        failed += 1;
                    }
                }
            }
        }
    }

    let _ = send_event(
        &sender,
        &NodeIpProbeEvent::JobCompleted {
            total_nodes: prepared.requested_ids.len(),
            succeeded,
            failed,
            cancelled,
        },
    )
    .await;
    BackgroundProbeSummary {
        requested: prepared.requested_ids.len(),
        succeeded,
        failed,
        batches: 1,
        cancelled,
        busy: false,
    }
}

async fn run_mihomo_probes(
    state: &AppState,
    nodes: &[PreparedNode],
    binary: &PathBuf,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    mode: ProbeRunMode,
    country_detection_auto_enabled: bool,
) -> Result<Vec<NodeIpProbeRecord>, String> {
    let mixed_port = allocate_local_port().map_err(|error| error.to_string())?;
    let controller_port = allocate_local_port().map_err(|error| error.to_string())?;
    let config_path = write_config(nodes, mixed_port, controller_port)?;
    let _config_guard = TempFileGuard(config_path.clone());
    let mut child = Command::new(binary)
        .arg("-f")
        .arg(&config_path)
        .kill_on_drop(true)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;

    let controller = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    if let Err(error) = wait_for_controller(state, sender, &controller, controller_port, mode).await
    {
        let _ = child.kill().await;
        return Err(error);
    }

    let intelligence_available =
        if country_detection_auto_enabled && ip_intelligence_service::configured(state) {
            ip_intelligence_service::check_connection(state)
                .await
                .is_ok()
        } else {
            false
        };

    let mut outcomes = Vec::with_capacity(nodes.len());
    for node in nodes {
        if should_cancel(state, sender, mode).await {
            let _ = child.kill().await;
            return Err("probe cancelled".to_string());
        }

        let result = probe_node(
            state,
            sender,
            &controller,
            controller_port,
            mixed_port,
            &node.proxy_name,
            mode,
        )
        .await;
        let now = now_rfc3339();
        let mut probe_latency_ms = None;
        let record = match result {
            Ok(result) => {
                probe_latency_ms = Some(result.latency_ms);
                let version = if result
                    .ip
                    .parse::<IpAddr>()
                    .is_ok_and(|value| value.is_ipv4())
                {
                    4
                } else {
                    6
                };
                node_ip_probe_repo::save_success(&state.db, node.id, &result.ip, version, &now)
                    .await
                    .map_err(|_| "failed to save IP probe result".to_string())?
            }
            Err(message) if message == "probe cancelled" => {
                let _ = child.kill().await;
                return Err(message);
            }
            Err(message) => node_ip_probe_repo::save_failure(
                &state.db,
                node.id,
                &truncate_message(&message),
                &now,
            )
            .await
            .map_err(|_| "failed to save IP probe result".to_string())?,
        };
        if record.status == "ok" {
            state.clear_public_export_cache().await;
        }
        if !send_record(sender, &record).await {
            let _ = child.kill().await;
            return Err("probe cancelled".to_string());
        }
        if record.status == "ok"
            && let Some(ip) = record.ip.as_deref()
        {
            let intelligence_record =
                if country_detection_auto_enabled && ip_intelligence_service::configured(state) {
                    if intelligence_available {
                        ip_intelligence_service::sync_node(state, &node.node, ip, probe_latency_ms)
                            .await
                    } else {
                        node_ip_probe_repo::update_intelligence_status(
                            &state.db,
                            node.id,
                            ip,
                            "error",
                            Some("IP intelligence service is unavailable"),
                            &now_rfc3339(),
                        )
                        .await
                        .map_err(|_| "failed to save IP intelligence status".to_string())?
                        .ok_or_else(|| {
                            "node exit IP changed before intelligence status was saved".to_string()
                        })
                    }
                } else {
                    node_ip_probe_repo::update_intelligence_status(
                        &state.db,
                        node.id,
                        ip,
                        "disabled",
                        None,
                        &now_rfc3339(),
                    )
                    .await
                    .map_err(|_| "failed to save IP intelligence status".to_string())?
                    .ok_or_else(|| {
                        "node exit IP changed before intelligence status was saved".to_string()
                    })
                };
            let intelligence_record = match intelligence_record {
                Ok(record) => Some(record),
                Err(_) => node_ip_probe_repo::find_by_node_id(&state.db, node.id)
                    .await
                    .map_err(|_| "failed to read IP intelligence status".to_string())?,
            };
            if let Some(intelligence_record) = intelligence_record
                && !send_intelligence_record(sender, &intelligence_record).await
            {
                let _ = child.kill().await;
                return Err("probe cancelled".to_string());
            }
        }
        outcomes.push(record);
    }

    let _ = child.kill().await;
    Ok(outcomes)
}

async fn wait_for_controller(
    state: &AppState,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    client: &reqwest::Client,
    controller_port: u16,
    mode: ProbeRunMode,
) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{controller_port}/version");
    let mut last_error = "controller unavailable".to_string();
    for _ in 0..50 {
        if should_cancel(state, sender, mode).await {
            return Err("probe cancelled".to_string());
        }
        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => last_error = format!("controller returned {}", response.status()),
            Err(error) => last_error = error.to_string(),
        }
        sleep(Duration::from_millis(100)).await;
    }
    Err(format!("Mihomo controller unavailable: {last_error}"))
}

async fn probe_node(
    state: &AppState,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    controller: &reqwest::Client,
    controller_port: u16,
    mixed_port: u16,
    proxy_name: &str,
    mode: ProbeRunMode,
) -> Result<ProbeResult, String> {
    let selector_url = format!(
        "http://127.0.0.1:{}/proxies/{}",
        controller_port,
        encode(SELECTOR_NAME)
    );
    let response = controller
        .put(selector_url)
        .json(&serde_json::json!({ "name": proxy_name }))
        .send()
        .await
        .map_err(|error| truncate_message(&error.to_string()))?;
    if !response.status().is_success() {
        return Err(format!(
            "Mihomo could not select this node: {}",
            response.status()
        ));
    }

    let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{mixed_port}"))
        .map_err(|error| error.to_string())?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .pool_max_idle_per_host(0)
        .timeout(Duration::from_secs(15))
        .user_agent("SublinkX-RS IP Probe")
        .build()
        .map_err(|error| error.to_string())?;
    let started_at = Instant::now();
    let request = client
        .get(IP_ECHO_URL)
        .header("Cache-Control", "no-cache")
        .send();
    tokio::pin!(request);
    let cancel = wait_for_cancel(state, sender, mode);
    tokio::pin!(cancel);
    let response = tokio::select! {
        result = &mut request => result.map_err(|error| truncate_message(&error.to_string()))?,
        () = &mut cancel => return Err("probe cancelled".to_string()),
    };
    if !response.status().is_success() {
        return Err(format!("IP echo provider returned {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > 4096)
    {
        return Err("IP echo response is too large".to_string());
    }
    let body = response
        .bytes()
        .await
        .map_err(|error| truncate_message(&error.to_string()))?;
    if body.len() > 4096 {
        return Err("IP echo response is too large".to_string());
    }
    let payload: IpifyResponse =
        serde_json::from_slice(&body).map_err(|_| "IP echo response is invalid".to_string())?;
    let ip = normalize_public_ip(&payload.ip).map_err(|error| error.to_string())?;
    Ok(ProbeResult {
        ip,
        latency_ms: started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u32::MAX),
    })
}

fn render_proxy(view: NodeView) -> Result<PreparedNode, String> {
    let mut proxy =
        export_service::render_mihomo_proxy(&view).map_err(|error| error.to_string())?;
    let proxy_name = format!(
        "ip-node-{}-{}",
        view.id,
        view.name.replace(['/', '\\', '?', '#'], "_")
    );
    proxy.insert(
        Value::String("name".to_string()),
        Value::String(proxy_name.clone()),
    );
    Ok(PreparedNode {
        id: view.id,
        node: view,
        proxy_name,
        proxy,
    })
}

fn write_config(
    nodes: &[PreparedNode],
    mixed_port: u16,
    controller_port: u16,
) -> Result<PathBuf, String> {
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
        Value::Sequence(
            nodes
                .iter()
                .map(|node| Value::Mapping(node.proxy.clone()))
                .collect(),
        ),
    );
    root.insert(
        Value::String("proxy-groups".to_string()),
        Value::Sequence(vec![Value::Mapping({
            let mut group = Mapping::new();
            group.insert(
                Value::String("name".to_string()),
                Value::String(SELECTOR_NAME.to_string()),
            );
            group.insert(
                Value::String("type".to_string()),
                Value::String("select".to_string()),
            );
            group.insert(
                Value::String("proxies".to_string()),
                Value::Sequence(
                    nodes
                        .iter()
                        .map(|node| Value::String(node.proxy_name.clone()))
                        .collect(),
                ),
            );
            group
        })]),
    );
    root.insert(
        Value::String("rules".to_string()),
        Value::Sequence(vec![Value::String(format!("MATCH,{SELECTOR_NAME}"))]),
    );

    let yaml = serde_yaml::to_string(&Value::Mapping(root)).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let path = std::env::temp_dir().join(format!(
        "sublinkx-ip-probe-{}-{timestamp}.yaml",
        std::process::id()
    ));
    fs::write(&path, yaml).map_err(|error| error.to_string())?;
    Ok(path)
}

async fn persist_and_send_failure(
    state: &AppState,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    id: i64,
    message: &str,
) -> bool {
    let now = now_rfc3339();
    let Ok(record) =
        node_ip_probe_repo::save_failure(&state.db, id, &truncate_message(message), &now).await
    else {
        return false;
    };
    send_record(sender, &record).await
}

async fn send_record(
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    record: &NodeIpProbeRecord,
) -> bool {
    send_event(
        sender,
        &NodeIpProbeEvent::NodeCompleted {
            id: record.node_id,
            status: record.status.clone(),
            ip: record.ip.clone(),
            ip_version: record.ip_version,
            country_code: record.country_code.clone(),
            country_name: record.country_name.clone(),
            message: record.message.clone(),
            probed_at: record.probed_at.clone(),
        },
    )
    .await
}

async fn send_intelligence_record(
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    record: &NodeIpProbeRecord,
) -> bool {
    send_event(
        sender,
        &NodeIpProbeEvent::IntelligenceUpdated {
            id: record.node_id,
            country_code: record.country_code.clone(),
            country_name: record.country_name.clone(),
            country_source: record.country_source.clone(),
            intelligence_status: record.intelligence_status.clone(),
            intelligence_message: record.intelligence_message.clone(),
            intelligence_updated_at: record.intelligence_updated_at.clone(),
        },
    )
    .await
}

async fn send_event(
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    event: &NodeIpProbeEvent,
) -> bool {
    let Ok(line) = serde_json::to_string(event).map(|value| format!("{value}\n")) else {
        return false;
    };
    sender.send(Ok(Bytes::from(line))).await.is_ok()
}

async fn wait_for_cancel(
    state: &AppState,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    mode: ProbeRunMode,
) {
    loop {
        if should_cancel(state, sender, mode).await {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
}

async fn should_cancel(
    state: &AppState,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    mode: ProbeRunMode,
) -> bool {
    match mode {
        ProbeRunMode::Manual => {
            sender.is_closed() || state.is_manual_latency_cancel_requested().await
        }
        ProbeRunMode::Background => state.is_auto_latency_cancel_requested().await,
    }
}

async fn begin_manual_run(state: &AppState) -> Result<(), AppError> {
    state
        .try_begin_manual_latency_run(Duration::from_secs(1))
        .await
        .map_err(|error| match error {
            LatencyManualBeginError::AutoRunning => {
                AppError::LatencyAutoRunning("background latency testing is running".to_string())
            }
            LatencyManualBeginError::ManualRunning => AppError::TooManyRequests(
                "another Mihomo inspection task is already running".to_string(),
            ),
            LatencyManualBeginError::Cooldown => AppError::TooManyRequests(
                "a Mihomo inspection task was started recently; try again later".to_string(),
            ),
        })
}

fn validate_request(payload: &NodeIpProbeRequest) -> Result<(), AppError> {
    if payload.ids.is_empty() {
        return Err(AppError::BadRequest(
            "select at least one node for IP probing".to_string(),
        ));
    }
    if payload.ids.len() > MAX_PROBE_NODES {
        return Err(AppError::BadRequest(format!(
            "at most {MAX_PROBE_NODES} nodes can be probed at once"
        )));
    }
    Ok(())
}

fn normalize_optional(
    value: &Option<String>,
    max_chars: usize,
    field: &str,
) -> Result<Option<String>, AppError> {
    let value = value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if value.is_some_and(|value| {
        value.chars().count() > max_chars || value.chars().any(char::is_control)
    }) {
        return Err(AppError::BadRequest(format!(
            "{field} must contain at most {max_chars} printable characters"
        )));
    }
    Ok(value.map(str::to_string))
}

fn normalize_public_ip(value: &str) -> Result<String, AppError> {
    let ip = value
        .trim()
        .parse::<IpAddr>()
        .map_err(|_| AppError::BadRequest("IP probe result is invalid".to_string()))?;
    if !is_public_ip(ip) {
        return Err(AppError::BadRequest(
            "IP probe result is not a public address".to_string(),
        ));
    }
    Ok(ip.to_string())
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 198 && matches!(octets[1], 18 | 19))
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
        || octets[0] >= 240)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn allocate_local_port() -> Result<u16, std::io::Error> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr().map(|address| address.port()))
}

fn dedupe_ids(ids: &[i64]) -> Vec<i64> {
    let mut seen = HashSet::new();
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

fn encode(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}

fn truncate_message(message: &str) -> String {
    message.chars().take(300).collect()
}

struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::{MAX_PROBE_NODES, dedupe_ids, is_public_ip, normalize_public_ip, validate_request};
    use crate::dto::node_ip_probes::NodeIpProbeRequest;

    #[test]
    fn rejects_empty_and_oversized_probe_requests() {
        assert!(validate_request(&NodeIpProbeRequest { ids: vec![] }).is_err());
        assert!(
            validate_request(&NodeIpProbeRequest {
                ids: (1..=201).collect(),
            })
            .is_err()
        );
    }

    #[test]
    fn accepts_public_addresses_and_rejects_reserved_ranges() {
        for value in ["1.1.1.1", "2606:4700:4700::1111"] {
            assert!(is_public_ip(value.parse::<IpAddr>().expect("valid IP")));
            assert_eq!(normalize_public_ip(value).expect("public IP"), value);
        }
        for value in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "192.0.2.1",
            "::1",
            "fc00::1",
            "2001:db8::1",
        ] {
            assert!(!is_public_ip(value.parse::<IpAddr>().expect("valid IP")));
            assert!(normalize_public_ip(value).is_err());
        }
    }

    #[test]
    fn background_probe_ids_are_deduplicated_and_batchable() {
        let ids = (1..=MAX_PROBE_NODES as i64 + 1)
            .chain([1, 2, 3])
            .collect::<Vec<_>>();
        let deduped = dedupe_ids(&ids);
        assert_eq!(deduped.len(), MAX_PROBE_NODES + 1);
        assert_eq!(deduped.chunks(MAX_PROBE_NODES).count(), 2);
    }
}
