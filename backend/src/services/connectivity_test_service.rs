use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    fs,
    net::TcpListener,
    path::PathBuf,
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::body::{Body, Bytes};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_yaml::{Mapping, Value};
use tokio::{process::Command, sync::mpsc, task::JoinSet, time::sleep};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    domain::node::{NodeRecord, NodeView},
    dto::connectivity_tests::{
        ConnectivityTestEvent, ConnectivityTestPreset, ConnectivityTestPresetResponse,
        ConnectivityTestRequest,
    },
    errors::AppError,
    repository::node_repo,
    state::{AppState, LatencyManualBeginError},
    utils::time::now_rfc3339,
};

use super::{export_service, node_service, settings_service, url_safety};

const MAX_CONNECTIVITY_TEST_NODES: usize = 200;
const TARGET_SYSTEM_DEFAULT: &str = "system_default";
const TARGET_CLOUDFLARE: &str = "cloudflare_204";
const TARGET_GOOGLE: &str = "google_204";
const CLOUDFLARE_TEST_URL: &str = "https://cp.cloudflare.com/generate_204";
const GOOGLE_TEST_URL: &str = "https://www.gstatic.com/generate_204";

#[derive(Clone)]
struct PreparedNode {
    record: NodeRecord,
    proxy_name: String,
    proxy: Mapping,
}

#[derive(Debug, Clone)]
struct SampleOutcome {
    latency_ms: Option<u128>,
    message: Option<String>,
    cancelled: bool,
}

#[derive(Debug)]
struct NodeSummary {
    status: String,
    min_ms: Option<u128>,
    average_ms: Option<u128>,
    max_ms: Option<u128>,
    jitter_ms: Option<u128>,
    success_rate: u8,
    succeeded: u8,
    failed: u8,
    message: Option<String>,
    tested_at: String,
}

struct PreparedRun {
    binary: PathBuf,
    requested_ids: Vec<i64>,
    nodes: Vec<PreparedNode>,
    preflight_errors: HashMap<i64, String>,
    target_id: String,
    target_url: String,
    rounds: u8,
    concurrency: usize,
    timeout_ms: u64,
    sync_last_latency: bool,
}

pub async fn presets(state: &AppState) -> Result<ConnectivityTestPresetResponse, AppError> {
    let settings = settings_service::load_settings(state).await?;
    Ok(ConnectivityTestPresetResponse {
        code: "00000",
        data: vec![
            ConnectivityTestPreset {
                id: TARGET_SYSTEM_DEFAULT.to_string(),
                name: "System default".to_string(),
                url: settings.latency_test_url,
                is_system_default: true,
            },
            ConnectivityTestPreset {
                id: TARGET_CLOUDFLARE.to_string(),
                name: "Cloudflare 204".to_string(),
                url: CLOUDFLARE_TEST_URL.to_string(),
                is_system_default: false,
            },
            ConnectivityTestPreset {
                id: TARGET_GOOGLE.to_string(),
                name: "Google 204".to_string(),
                url: GOOGLE_TEST_URL.to_string(),
                is_system_default: false,
            },
        ],
    })
}

pub async fn stream(state: AppState, payload: ConnectivityTestRequest) -> Result<Body, AppError> {
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
        run_stream(&state, prepared, sender).await;
        state.finish_manual_latency_run().await;
    });

    Ok(Body::from_stream(ReceiverStream::new(receiver)))
}

async fn prepare_run(
    state: &AppState,
    payload: ConnectivityTestRequest,
) -> Result<PreparedRun, AppError> {
    let settings = settings_service::load_settings(state).await?;
    let binary = node_service::ensure_mihomo_core_ready(&settings).await?;
    let target_url = resolve_target_url(&payload.target_id, &settings.latency_test_url)?;
    url_safety::validate_public_http_url(&target_url, "connectivity test target").await?;

    let requested_ids = dedupe_ids(&payload.ids);
    let records = node_repo::find_by_ids(&state.db, &requested_ids).await?;
    let mut records_by_id: HashMap<i64, NodeRecord> =
        records.into_iter().map(|node| (node.id, node)).collect();
    let mut nodes = Vec::with_capacity(requested_ids.len());
    let mut preflight_errors = HashMap::new();

    for id in &requested_ids {
        let Some(record) = records_by_id.remove(id) else {
            preflight_errors.insert(*id, "node not found".to_string());
            continue;
        };
        match render_proxy(record) {
            Ok(node) => nodes.push(node),
            Err(message) => {
                preflight_errors.insert(*id, truncate_message(&message));
            }
        }
    }

    let concurrency = usize::try_from(
        settings
            .latency_concurrency
            .clamp(1, settings_service::MAX_LATENCY_CONCURRENCY),
    )
    .unwrap_or(1);

    Ok(PreparedRun {
        binary,
        requested_ids,
        nodes,
        preflight_errors,
        target_id: payload.target_id,
        target_url,
        rounds: payload.rounds,
        concurrency,
        timeout_ms: settings.latency_timeout_secs.clamp(3, 60) as u64 * 1000,
        sync_last_latency: payload.sync_last_latency,
    })
}

async fn run_stream(
    state: &AppState,
    prepared: PreparedRun,
    sender: mpsc::Sender<Result<Bytes, Infallible>>,
) {
    if !send_event(
        &sender,
        &ConnectivityTestEvent::JobStarted {
            total_nodes: prepared.requested_ids.len(),
            rounds: prepared.rounds,
            target_id: prepared.target_id.clone(),
            target_url: prepared.target_url.clone(),
            concurrency: prepared.concurrency,
        },
    )
    .await
    {
        return;
    }

    let mut samples: HashMap<i64, Vec<SampleOutcome>> = HashMap::new();
    for (id, message) in &prepared.preflight_errors {
        for round in 1..=prepared.rounds {
            let outcome = SampleOutcome {
                latency_ms: None,
                message: Some(message.clone()),
                cancelled: false,
            };
            samples.entry(*id).or_default().push(outcome.clone());
            if !send_sample(&sender, *id, round, &outcome).await {
                return;
            }
        }
    }

    let mut cancelled = false;
    if !prepared.nodes.is_empty() {
        cancelled = run_mihomo_samples(state, &prepared, &sender, &mut samples).await;
    }

    let mut succeeded_nodes = 0_usize;
    let mut failed_nodes = 0_usize;
    for id in &prepared.requested_ids {
        let summary = summarize(
            samples.get(id).map(Vec::as_slice).unwrap_or_default(),
            prepared.rounds,
            cancelled,
        );
        if summary.succeeded > 0 {
            succeeded_nodes += 1;
        } else {
            failed_nodes += 1;
        }

        if prepared.sync_last_latency
            && prepared.target_id == TARGET_SYSTEM_DEFAULT
            && (summary.succeeded > 0 || !cancelled)
        {
            let persisted_status = if summary.succeeded > 0 { "ok" } else { "error" };
            let latency_ms = summary
                .average_ms
                .and_then(|value| i64::try_from(value).ok());
            let _ = node_repo::update_latency(
                &state.db,
                *id,
                latency_ms,
                persisted_status,
                summary.message.as_deref(),
                &summary.tested_at,
            )
            .await;
        }

        if !send_event(
            &sender,
            &ConnectivityTestEvent::NodeCompleted {
                id: *id,
                status: summary.status,
                min_ms: summary.min_ms,
                average_ms: summary.average_ms,
                max_ms: summary.max_ms,
                jitter_ms: summary.jitter_ms,
                success_rate: summary.success_rate,
                succeeded: summary.succeeded,
                failed: summary.failed,
                message: summary.message,
                tested_at: summary.tested_at,
            },
        )
        .await
        {
            return;
        }
    }

    let _ = send_event(
        &sender,
        &ConnectivityTestEvent::JobCompleted {
            total_nodes: prepared.requested_ids.len(),
            succeeded: succeeded_nodes,
            failed: failed_nodes,
            cancelled,
        },
    )
    .await;
}

async fn run_mihomo_samples(
    state: &AppState,
    prepared: &PreparedRun,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    samples: &mut HashMap<i64, Vec<SampleOutcome>>,
) -> bool {
    let mixed_port = match allocate_local_port() {
        Ok(port) => port,
        Err(error) => {
            record_setup_failure(prepared, sender, samples, &error.to_string()).await;
            return false;
        }
    };
    let controller_port = match allocate_local_port() {
        Ok(port) => port,
        Err(error) => {
            record_setup_failure(prepared, sender, samples, &error.to_string()).await;
            return false;
        }
    };
    let config_path = match write_config(&prepared.nodes, mixed_port, controller_port) {
        Ok(path) => path,
        Err(message) => {
            record_setup_failure(prepared, sender, samples, &message).await;
            return false;
        }
    };
    let _config_guard = TempFileGuard(config_path.clone());
    let mut child = match Command::new(&prepared.binary)
        .arg("-f")
        .arg(&config_path)
        .kill_on_drop(true)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            record_setup_failure(prepared, sender, samples, &error.to_string()).await;
            return false;
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(65))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            let _ = child.kill().await;
            record_setup_failure(prepared, sender, samples, &error.to_string()).await;
            return false;
        }
    };

    if let Err(message) = wait_for_controller(state, sender, &client, controller_port).await {
        let cancelled = message == "test cancelled";
        let _ = child.kill().await;
        if !cancelled {
            record_setup_failure(prepared, sender, samples, &message).await;
        }
        return cancelled;
    }

    let timeout_ms = prepared.timeout_ms;
    let mut cancelled = false;
    for round in 1..=prepared.rounds {
        if should_cancel(state, sender).await {
            cancelled = true;
            break;
        }
        for chunk in prepared.nodes.chunks(prepared.concurrency) {
            let mut tasks = JoinSet::new();
            for node in chunk {
                let client = client.clone();
                let state = state.clone();
                let sender = sender.clone();
                let target_url = prepared.target_url.clone();
                let proxy_name = node.proxy_name.clone();
                let id = node.record.id;
                let permit = match state.latency_test_semaphore.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => continue,
                };
                tasks.spawn(async move {
                    let _permit = permit;
                    let outcome = probe_proxy(
                        &state,
                        &sender,
                        &client,
                        controller_port,
                        &proxy_name,
                        &target_url,
                        timeout_ms,
                    )
                    .await;
                    (id, outcome)
                });
            }

            while let Some(joined) = tasks.join_next().await {
                let Ok((id, outcome)) = joined else {
                    continue;
                };
                cancelled |= outcome.cancelled;
                samples.entry(id).or_default().push(outcome.clone());
                if !send_sample(sender, id, round, &outcome).await {
                    cancelled = true;
                }
            }
            if cancelled {
                break;
            }
        }
        if cancelled {
            break;
        }
    }

    let _ = child.kill().await;
    cancelled
}

async fn wait_for_controller(
    state: &AppState,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    client: &reqwest::Client,
    controller_port: u16,
) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{controller_port}/version");
    let mut last_error = "controller unavailable".to_string();
    for _ in 0..50 {
        if should_cancel(state, sender).await {
            return Err("test cancelled".to_string());
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

async fn probe_proxy(
    state: &AppState,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    client: &reqwest::Client,
    controller_port: u16,
    proxy_name: &str,
    target_url: &str,
    timeout_ms: u64,
) -> SampleOutcome {
    let delay_url = format!(
        "http://127.0.0.1:{}/proxies/{}/delay?timeout={}&url={}",
        controller_port,
        encode(proxy_name),
        timeout_ms,
        encode(target_url)
    );
    let request = client.get(delay_url).send();
    tokio::pin!(request);
    let cancel = wait_for_cancel(state, sender);
    tokio::pin!(cancel);

    let response = tokio::select! {
        result = &mut request => result,
        () = &mut cancel => {
            return SampleOutcome {
                latency_ms: None,
                message: Some("test cancelled".to_string()),
                cancelled: true,
            };
        }
    };

    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return SampleOutcome {
                latency_ms: None,
                message: Some(truncate_message(&error.to_string())),
                cancelled: false,
            };
        }
    };
    let status = response.status();
    let body = match response.text().await {
        Ok(body) => body,
        Err(error) => {
            return SampleOutcome {
                latency_ms: None,
                message: Some(truncate_message(&error.to_string())),
                cancelled: false,
            };
        }
    };
    if !status.is_success() {
        return SampleOutcome {
            latency_ms: None,
            message: Some(truncate_message(&format!(
                "Mihomo returned {status}: {body}"
            ))),
            cancelled: false,
        };
    }
    match serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| value.get("delay").and_then(|delay| delay.as_u64()))
    {
        Some(delay) => SampleOutcome {
            latency_ms: Some(u128::from(delay)),
            message: None,
            cancelled: false,
        },
        None => SampleOutcome {
            latency_ms: None,
            message: Some(truncate_message(&format!(
                "Mihomo response did not contain delay: {body}"
            ))),
            cancelled: false,
        },
    }
}

async fn wait_for_cancel(state: &AppState, sender: &mpsc::Sender<Result<Bytes, Infallible>>) {
    loop {
        if sender.is_closed() || state.is_manual_latency_cancel_requested().await {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
}

async fn should_cancel(state: &AppState, sender: &mpsc::Sender<Result<Bytes, Infallible>>) -> bool {
    sender.is_closed() || state.is_manual_latency_cancel_requested().await
}

async fn record_setup_failure(
    prepared: &PreparedRun,
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    samples: &mut HashMap<i64, Vec<SampleOutcome>>,
    message: &str,
) {
    let message = truncate_message(message);
    for node in &prepared.nodes {
        for round in 1..=prepared.rounds {
            let outcome = SampleOutcome {
                latency_ms: None,
                message: Some(message.clone()),
                cancelled: false,
            };
            samples
                .entry(node.record.id)
                .or_default()
                .push(outcome.clone());
            if !send_sample(sender, node.record.id, round, &outcome).await {
                return;
            }
        }
    }
}

async fn send_sample(
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    id: i64,
    round: u8,
    outcome: &SampleOutcome,
) -> bool {
    send_event(
        sender,
        &ConnectivityTestEvent::SampleCompleted {
            id,
            round,
            status: if outcome.cancelled {
                "cancelled"
            } else if outcome.latency_ms.is_some() {
                "ok"
            } else {
                "error"
            }
            .to_string(),
            latency_ms: outcome.latency_ms,
            message: outcome.message.clone(),
        },
    )
    .await
}

async fn send_event(
    sender: &mpsc::Sender<Result<Bytes, Infallible>>,
    event: &ConnectivityTestEvent,
) -> bool {
    let Ok(line) = serde_json::to_string(event).map(|value| format!("{value}\n")) else {
        return false;
    };
    sender.send(Ok(Bytes::from(line))).await.is_ok()
}

fn summarize(samples: &[SampleOutcome], rounds: u8, run_cancelled: bool) -> NodeSummary {
    let latencies: Vec<u128> = samples
        .iter()
        .filter_map(|sample| sample.latency_ms)
        .collect();
    let succeeded = u8::try_from(latencies.len()).unwrap_or(rounds).min(rounds);
    let failed = rounds.saturating_sub(succeeded);
    let min_ms = latencies.iter().copied().min();
    let max_ms = latencies.iter().copied().max();
    let average_ms = if latencies.is_empty() {
        None
    } else {
        Some(latencies.iter().sum::<u128>() / latencies.len() as u128)
    };
    let jitter_ms = if latencies.is_empty() {
        None
    } else if latencies.len() == 1 {
        Some(0)
    } else {
        let total_delta = latencies
            .windows(2)
            .map(|pair| pair[0].abs_diff(pair[1]))
            .sum::<u128>();
        Some(total_delta / (latencies.len() - 1) as u128)
    };
    let status = if run_cancelled && succeeded < rounds {
        "cancelled"
    } else if succeeded == rounds {
        "ok"
    } else if succeeded > 0 {
        "partial"
    } else {
        "error"
    }
    .to_string();
    let message = samples
        .iter()
        .rev()
        .find_map(|sample| sample.message.clone())
        .or_else(|| (run_cancelled && succeeded < rounds).then(|| "test cancelled".to_string()));

    NodeSummary {
        status,
        min_ms,
        average_ms,
        max_ms,
        jitter_ms,
        success_rate: if rounds == 0 {
            0
        } else {
            u8::try_from(u16::from(succeeded) * 100 / u16::from(rounds)).unwrap_or(0)
        },
        succeeded,
        failed,
        message,
        tested_at: now_rfc3339(),
    }
}

fn validate_request(payload: &ConnectivityTestRequest) -> Result<(), AppError> {
    if payload.ids.is_empty() {
        return Err(AppError::BadRequest(
            "select at least one node for connectivity testing".to_string(),
        ));
    }
    if payload.ids.len() > MAX_CONNECTIVITY_TEST_NODES {
        return Err(AppError::BadRequest(format!(
            "at most {MAX_CONNECTIVITY_TEST_NODES} nodes can be tested at once"
        )));
    }
    if !matches!(payload.rounds, 1 | 3 | 5) {
        return Err(AppError::BadRequest(
            "connectivity test rounds must be 1, 3, or 5".to_string(),
        ));
    }
    resolve_target_url(&payload.target_id, CLOUDFLARE_TEST_URL).map(|_| ())
}

fn resolve_target_url(target_id: &str, system_default: &str) -> Result<String, AppError> {
    match target_id {
        TARGET_SYSTEM_DEFAULT => Ok(system_default.to_string()),
        TARGET_CLOUDFLARE => Ok(CLOUDFLARE_TEST_URL.to_string()),
        TARGET_GOOGLE => Ok(GOOGLE_TEST_URL.to_string()),
        _ => Err(AppError::BadRequest(
            "unsupported connectivity test target".to_string(),
        )),
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
            LatencyManualBeginError::ManualRunning => {
                AppError::TooManyRequests("latency testing is already running".to_string())
            }
            LatencyManualBeginError::Cooldown => AppError::TooManyRequests(
                "latency testing was started recently; try again later".to_string(),
            ),
        })
}

fn render_proxy(record: NodeRecord) -> Result<PreparedNode, String> {
    let view = NodeView::try_from(record.clone()).map_err(|error| error.to_string())?;
    let mut proxy =
        export_service::render_mihomo_proxy(&view).map_err(|error| error.to_string())?;
    let proxy_name = format!(
        "node-{}-{}",
        record.id,
        record.name.replace(['/', '\\', '?', '#'], "_")
    );
    proxy.insert(
        Value::String("name".to_string()),
        Value::String(proxy_name.clone()),
    );
    Ok(PreparedNode {
        record,
        proxy_name,
        proxy,
    })
}

fn write_config(
    nodes: &[PreparedNode],
    mixed_port: u16,
    controller_port: u16,
) -> Result<PathBuf, String> {
    let proxy_names: Vec<Value> = nodes
        .iter()
        .map(|node| Value::String(node.proxy_name.clone()))
        .collect();
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
                Value::String("CONNECTIVITY".to_string()),
            );
            group.insert(
                Value::String("type".to_string()),
                Value::String("select".to_string()),
            );
            group.insert(
                Value::String("proxies".to_string()),
                Value::Sequence(proxy_names),
            );
            group
        })]),
    );
    root.insert(
        Value::String("rules".to_string()),
        Value::Sequence(vec![Value::String("MATCH,CONNECTIVITY".to_string())]),
    );

    let yaml = serde_yaml::to_string(&Value::Mapping(root)).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let path = std::env::temp_dir().join(format!(
        "sublinkx-connectivity-{}-{timestamp}.yaml",
        std::process::id()
    ));
    fs::write(&path, yaml).map_err(|error| error.to_string())?;
    Ok(path)
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
    use super::{SampleOutcome, summarize, validate_request};
    use crate::dto::connectivity_tests::ConnectivityTestRequest;

    #[test]
    fn rejects_arbitrary_targets_and_invalid_rounds() {
        let invalid_target = ConnectivityTestRequest {
            ids: vec![1],
            target_id: "https://example.com".to_string(),
            rounds: 3,
            sync_last_latency: false,
        };
        assert!(validate_request(&invalid_target).is_err());

        let invalid_rounds = ConnectivityTestRequest {
            ids: vec![1],
            target_id: "system_default".to_string(),
            rounds: 2,
            sync_last_latency: false,
        };
        assert!(validate_request(&invalid_rounds).is_err());
    }

    #[test]
    fn summarizes_latency_and_jitter_across_rounds() {
        let samples = vec![
            SampleOutcome {
                latency_ms: Some(100),
                message: None,
                cancelled: false,
            },
            SampleOutcome {
                latency_ms: Some(140),
                message: None,
                cancelled: false,
            },
            SampleOutcome {
                latency_ms: Some(110),
                message: None,
                cancelled: false,
            },
        ];
        let summary = summarize(&samples, 3, false);

        assert_eq!(summary.status, "ok");
        assert_eq!(summary.min_ms, Some(100));
        assert_eq!(summary.average_ms, Some(116));
        assert_eq!(summary.max_ms, Some(140));
        assert_eq!(summary.jitter_ms, Some(35));
        assert_eq!(summary.success_rate, 100);
    }

    #[test]
    fn reports_partial_success_without_hiding_failures() {
        let samples = vec![
            SampleOutcome {
                latency_ms: Some(120),
                message: None,
                cancelled: false,
            },
            SampleOutcome {
                latency_ms: None,
                message: Some("timeout".to_string()),
                cancelled: false,
            },
        ];
        let summary = summarize(&samples, 3, false);

        assert_eq!(summary.status, "partial");
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.success_rate, 33);
        assert_eq!(summary.message.as_deref(), Some("timeout"));
    }
}
