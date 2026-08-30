use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ConnectivityTestRequest {
    pub ids: Vec<i64>,
    pub target_id: String,
    pub rounds: u8,
    #[serde(default)]
    pub sync_last_latency: bool,
}

#[derive(Debug, Serialize)]
pub struct ConnectivityTestPreset {
    pub id: String,
    pub name: String,
    pub url: String,
    pub is_system_default: bool,
}

#[derive(Debug, Serialize)]
pub struct ConnectivityTestPresetResponse {
    pub code: &'static str,
    pub data: Vec<ConnectivityTestPreset>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConnectivityTestEvent {
    JobStarted {
        total_nodes: usize,
        rounds: u8,
        target_id: String,
        target_url: String,
        concurrency: usize,
    },
    SampleCompleted {
        id: i64,
        round: u8,
        status: String,
        latency_ms: Option<u128>,
        message: Option<String>,
    },
    NodeCompleted {
        id: i64,
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
    },
    JobCompleted {
        total_nodes: usize,
        succeeded: usize,
        failed: usize,
        cancelled: bool,
    },
}
