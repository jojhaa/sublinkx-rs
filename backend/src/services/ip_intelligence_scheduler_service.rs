use std::time::{Duration, Instant};

use time::OffsetDateTime;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::{
    dto::node_ip_probes::RefreshNodeIpIntelligenceRequest, repository::node_ip_probe_repo,
    state::AppState,
};

use super::{ip_intelligence_service, node_ip_probe_service, settings_service};

const INITIAL_DELAY: Duration = Duration::from_secs(30);
const SETTINGS_POLL_INTERVAL: Duration = Duration::from_secs(60);
const REFRESH_BATCH_SIZE: i64 = 50;

pub fn spawn(state: AppState) {
    if !ip_intelligence_service::configured(&state) {
        return;
    }
    tokio::spawn(async move {
        sleep(INITIAL_DELAY).await;
        let mut last_started_at: Option<Instant> = None;
        loop {
            match settings_service::load_settings(&state).await {
                Ok(settings) if settings.country_detection_auto_enabled => {
                    let interval = Duration::from_secs(
                        settings.country_detection_interval_minutes.clamp(
                            settings_service::MIN_COUNTRY_DETECTION_INTERVAL_MINUTES,
                            settings_service::MAX_COUNTRY_DETECTION_INTERVAL_MINUTES,
                        ) as u64
                            * 60,
                    );
                    let due = last_started_at.is_none_or(|started| started.elapsed() >= interval);
                    if due {
                        match reconcile(&state).await {
                            Ok(()) => last_started_at = Some(Instant::now()),
                            Err(error) => {
                                warn!(error = %error, "IP intelligence reconciliation failed")
                            }
                        }
                    }
                }
                Ok(_) => last_started_at = None,
                Err(error) => warn!(error = %error, "failed to load country detection settings"),
            }
            sleep(SETTINGS_POLL_INTERVAL).await;
        }
    });
}

async fn reconcile(state: &AppState) -> Result<(), String> {
    ip_intelligence_service::check_connection(state).await?;
    let stale_before = (OffsetDateTime::now_utc() - time::Duration::hours(24))
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|error| error.to_string())?;
    let ids = node_ip_probe_repo::intelligence_refresh_candidates(
        &state.db,
        &stale_before,
        REFRESH_BATCH_SIZE,
    )
    .await
    .map_err(|error| error.to_string())?;
    if ids.is_empty() {
        return Ok(());
    }
    let summary = node_ip_probe_service::refresh_intelligence(
        state,
        RefreshNodeIpIntelligenceRequest { ids },
    )
    .await
    .map_err(|error| error.to_string())?;
    info!(
        updated = summary.updated,
        pending = summary.pending,
        failed = summary.failed,
        "IP intelligence reconciliation completed"
    );
    Ok(())
}
