use std::time::Duration;

use tokio::time::sleep;
use tracing::{debug, warn};

use crate::{services::upstream_subscription_service, state::AppState};

pub fn spawn_upstream_subscription_syncer(state: AppState) {
    tokio::spawn(async move {
        sleep(Duration::from_secs(20)).await;

        loop {
            match upstream_subscription_service::sync_due_subscriptions(&state).await {
                Ok(count) if count > 0 => {
                    debug!(
                        synced = count,
                        "scheduled upstream subscription sync completed"
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    warn!(
                        "scheduled upstream subscription sync scan failed: {}",
                        error
                    );
                }
            }

            sleep(Duration::from_secs(60)).await;
        }
    });
}
