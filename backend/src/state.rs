use std::time::Instant;

use crate::config::AppConfig;
use crate::db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: DbPool,
    pub started_at: Instant,
}

impl AppState {
    pub fn new(config: AppConfig, db: DbPool) -> Self {
        Self {
            config,
            db,
            started_at: Instant::now(),
        }
    }
}
