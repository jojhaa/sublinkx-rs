use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode},
};
use tokio::sync::{Mutex, Semaphore};

use crate::config::AppConfig;
use crate::db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: DbPool,
    pub latency_test_semaphore: Arc<Semaphore>,
    latency_run_state: Arc<Mutex<LatencyRunState>>,
    public_export_cache: Arc<Mutex<HashMap<String, PublicExportCacheEntry>>>,
    rate_limit_buckets: Arc<Mutex<HashMap<String, RateLimitBucket>>>,
}

#[derive(Debug)]
struct LatencyRunState {
    manual_in_progress: bool,
    auto_in_progress: bool,
    auto_cancel_requested: bool,
    last_manual_started_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyManualBeginError {
    AutoRunning,
    ManualRunning,
    Cooldown,
}

#[derive(Debug, Clone)]
pub struct PublicExportCacheEntry {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub(crate) cached_at: Instant,
}

#[derive(Debug)]
struct RateLimitBucket {
    count: u32,
    reset_at: Instant,
}

impl AppState {
    pub fn new(config: AppConfig, db: DbPool) -> Self {
        Self {
            config,
            db,
            latency_test_semaphore: Arc::new(Semaphore::new(
                crate::services::settings_service::MAX_LATENCY_CONCURRENCY as usize,
            )),
            latency_run_state: Arc::new(Mutex::new(LatencyRunState {
                manual_in_progress: false,
                auto_in_progress: false,
                auto_cancel_requested: false,
                last_manual_started_at: None,
            })),
            public_export_cache: Arc::new(Mutex::new(HashMap::new())),
            rate_limit_buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn try_begin_manual_latency_run(
        &self,
        cooldown: std::time::Duration,
    ) -> Result<(), LatencyManualBeginError> {
        let now = Instant::now();
        let mut state = self.latency_run_state.lock().await;
        if state.auto_in_progress {
            return Err(LatencyManualBeginError::AutoRunning);
        }
        if state.manual_in_progress {
            return Err(LatencyManualBeginError::ManualRunning);
        }
        if let Some(last_started_at) = state.last_manual_started_at
            && now.duration_since(last_started_at) < cooldown
        {
            return Err(LatencyManualBeginError::Cooldown);
        }

        state.manual_in_progress = true;
        state.last_manual_started_at = Some(now);
        Ok(())
    }

    pub async fn finish_manual_latency_run(&self) {
        let mut state = self.latency_run_state.lock().await;
        state.manual_in_progress = false;
    }

    pub async fn try_begin_auto_latency_run(&self) -> bool {
        let mut state = self.latency_run_state.lock().await;
        if state.manual_in_progress || state.auto_in_progress {
            return false;
        }

        state.auto_in_progress = true;
        state.auto_cancel_requested = false;
        true
    }

    pub async fn finish_auto_latency_run(&self) {
        let mut state = self.latency_run_state.lock().await;
        state.auto_in_progress = false;
        state.auto_cancel_requested = false;
    }

    pub async fn request_cancel_auto_latency_run(&self) -> bool {
        let mut state = self.latency_run_state.lock().await;
        if !state.auto_in_progress {
            return false;
        }

        state.auto_cancel_requested = true;
        true
    }

    pub async fn is_auto_latency_cancel_requested(&self) -> bool {
        let state = self.latency_run_state.lock().await;
        state.auto_cancel_requested
    }

    pub async fn get_public_export_cache(
        &self,
        key: &str,
        ttl: Duration,
    ) -> Option<PublicExportCacheEntry> {
        let mut cache = self.public_export_cache.lock().await;
        cache.retain(|_, entry| entry.cached_at.elapsed() < ttl);
        cache.get(key).cloned()
    }

    pub async fn set_public_export_cache(&self, key: String, mut entry: PublicExportCacheEntry) {
        entry.cached_at = Instant::now();
        let mut cache = self.public_export_cache.lock().await;
        cache.insert(key, entry);
    }

    pub async fn check_rate_limit(
        &self,
        key: &str,
        limit: u32,
        window: Duration,
    ) -> Result<(), &'static str> {
        let now = Instant::now();
        let mut buckets = self.rate_limit_buckets.lock().await;
        buckets.retain(|_, bucket| bucket.reset_at > now);
        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| RateLimitBucket {
                count: 0,
                reset_at: now + window,
            });

        if bucket.reset_at <= now {
            bucket.count = 0;
            bucket.reset_at = now + window;
        }

        if bucket.count >= limit {
            return Err("too many requests; try again later");
        }

        bucket.count += 1;
        Ok(())
    }
}
