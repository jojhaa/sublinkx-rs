use crate::config::AppConfig;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: SqlitePool,
}

impl AppState {
    pub fn new(config: AppConfig, db: SqlitePool) -> Self {
        Self { config, db }
    }
}
