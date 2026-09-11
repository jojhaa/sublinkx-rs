use std::time::{Duration, Instant};

use tokio::time::sleep;
use tracing::{info, warn};

use crate::{
    repository::{node_ip_probe_repo, node_repo},
    state::AppState,
    utils::time::now_rfc3339,
};

use super::{node_ip_probe_service, settings_service};

const INITIAL_DELAY: Duration = Duration::from_secs(45);
const SETTINGS_POLL_INTERVAL: Duration = Duration::from_secs(60);
const QUEUE_POLL_INTERVAL: Duration = Duration::from_secs(10);
const BUSY_RETRY_DELAY: Duration = Duration::from_secs(30);

pub fn spawn(state: AppState) {
    spawn_queue_worker(state.clone());
    tokio::spawn(async move {
        sleep(INITIAL_DELAY).await;
        let mut last_started_at: Option<Instant> = None;
        loop {
            match settings_service::load_settings(&state).await {
                Ok(settings) if settings.ip_probe_auto_enabled => {
                    let interval = Duration::from_secs(
                        settings.ip_probe_interval_minutes.clamp(
                            settings_service::MIN_IP_PROBE_INTERVAL_MINUTES,
                            settings_service::MAX_IP_PROBE_INTERVAL_MINUTES,
                        ) as u64
                            * 60,
                    );
                    let due = last_started_at.is_none_or(|started| started.elapsed() >= interval);
                    if due {
                        match queue_all_enabled(&state).await {
                            Ok(()) => last_started_at = Some(Instant::now()),
                            Err(error) => warn!(error = %error, "scheduled IP probe scan failed"),
                        }
                    }
                }
                Ok(_) => last_started_at = None,
                Err(error) => warn!(error = %error, "failed to load scheduled IP probe settings"),
            }
            sleep(SETTINGS_POLL_INTERVAL).await;
        }
    });
}

pub fn spawn_after_upstream_import(state: AppState, source_ref: String) {
    tokio::spawn(async move {
        let settings = match settings_service::load_settings(&state).await {
            Ok(settings) => settings,
            Err(error) => {
                warn!(error = %error, "failed to load post-import IP probe settings");
                return;
            }
        };
        let ids = match node_repo::list_enabled_ids_by_upstream_source_ref(&state.db, &source_ref)
            .await
        {
            Ok(ids) => ids,
            Err(error) => {
                warn!(error = %error, "failed to list imported nodes for IP probing");
                return;
            }
        };
        match node_ip_probe_repo::invalidate_risk_for_node_ids(&state.db, &ids, &now_rfc3339())
            .await
        {
            Ok(changed) if changed > 0 => state.clear_public_export_cache().await,
            Ok(_) => {}
            Err(error) => {
                warn!(error = %error, "failed to invalidate imported node risk state");
                return;
            }
        }
        if !settings.ip_probe_after_upstream_import {
            info!(
                invalidated = ids.len(),
                "upstream import risk state invalidated; automatic IP probing is disabled"
            );
            return;
        }
        let queued = state.enqueue_background_ip_probes(ids).await;
        info!(
            queued,
            "upstream import nodes queued for background IP probing"
        );
    });
}

async fn queue_all_enabled(state: &AppState) -> Result<(), String> {
    let ids = node_repo::list_enabled(&state.db)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|node| node.upstream_missing == 0)
        .map(|node| node.id)
        .collect::<Vec<_>>();
    let queued = state.enqueue_background_ip_probes(ids).await;
    info!(queued, "scheduled nodes queued for background IP probing");
    Ok(())
}

fn spawn_queue_worker(state: AppState) {
    tokio::spawn(async move {
        sleep(Duration::from_secs(5)).await;
        loop {
            let ids = state.take_background_ip_probes().await;
            if ids.is_empty() {
                sleep(QUEUE_POLL_INTERVAL).await;
                continue;
            }
            let retry_ids = ids.clone();
            if !run_and_log(&state, ids).await {
                state.enqueue_background_ip_probes(retry_ids).await;
                sleep(BUSY_RETRY_DELAY).await;
            }
        }
    });
}

async fn run_and_log(state: &AppState, ids: Vec<i64>) -> bool {
    match node_ip_probe_service::run_background(state, ids).await {
        Ok(summary) if summary.busy => {
            info!(
                requested = summary.requested,
                "background IP probe deferred because another Mihomo task is running"
            );
            false
        }
        Ok(summary) => {
            info!(
                requested = summary.requested,
                succeeded = summary.succeeded,
                failed = summary.failed,
                batches = summary.batches,
                cancelled = summary.cancelled,
                "background IP probe completed"
            );
            !summary.cancelled
        }
        Err(error) => {
            warn!(error = %error, "background IP probe failed");
            true
        }
    }
}
