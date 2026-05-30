use std::time::Duration;

use tokio::time::sleep;
use tracing::{debug, warn};

use crate::state::AppState;

use super::{node_service, settings_service};

pub fn spawn_auto_latency_tester(state: AppState) {
    tokio::spawn(async move {
        sleep(Duration::from_secs(10)).await;

        loop {
            match settings_service::load_settings(&state).await {
                Ok(settings) => {
                    if settings.latency_auto_enabled {
                        match node_service::test_all_enabled_node_latencies(&state).await {
                            Ok(results) => {
                                debug!("auto latency tested {} enabled nodes", results.len());
                            }
                            Err(error) => {
                                warn!("auto latency test failed: {}", error);
                            }
                        }
                    }

                    let minutes = settings.latency_interval_minutes.clamp(5, 1440) as u64;
                    sleep(Duration::from_secs(minutes * 60)).await;
                }
                Err(error) => {
                    warn!("failed to load latency settings: {}", error);
                    sleep(Duration::from_secs(30 * 60)).await;
                }
            }
        }
    });
}
