use serde::{Deserialize, Serialize};

use crate::domain::node_ip_probe::NodeIpProbeRecord;

#[derive(Debug, Deserialize)]
pub struct NodeIpProbeRequest {
    pub ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshNodeIpIntelligenceRequest {
    pub ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BatchNodeRiskScoreRequest {
    pub ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNodeIpCountryRequest {
    pub ip: String,
    pub country_code: String,
    pub country_name: Option<String>,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct NodeIpProbeListResponse {
    pub code: &'static str,
    pub data: Vec<NodeIpProbeRecord>,
}

#[derive(Debug, Serialize)]
pub struct NodeIpProbeResponse {
    pub code: &'static str,
    pub data: NodeIpProbeRecord,
}

#[derive(Debug, Serialize)]
pub struct NodeIpIntelligenceStatusResponse {
    pub code: &'static str,
    pub enabled: bool,
    pub base_url: Option<String>,
    pub source_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RefreshNodeIpIntelligenceResponse {
    pub code: &'static str,
    pub updated: usize,
    pub pending: usize,
    pub failed: usize,
    pub data: Vec<NodeIpProbeRecord>,
}

#[derive(Debug, Serialize)]
pub struct BatchNodeRiskScoreResponse {
    pub code: &'static str,
    pub requested: usize,
    pub unique_ips: usize,
    pub complete: usize,
    pub partial: usize,
    pub pending: usize,
    pub failed: usize,
    pub data: Vec<NodeRiskScoreResult>,
}

#[derive(Debug, Serialize)]
pub struct NodeRiskScoreResult {
    pub node_id: i64,
    pub node_name: Option<String>,
    pub ip: Option<String>,
    pub status: &'static str,
    pub intelligence_cache_status: Option<String>,
    pub scamalytics_fraud_score: Option<u8>,
    pub scamalytics_isp_risk_score: Option<u8>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeIpProbeEvent {
    JobStarted {
        total_nodes: usize,
        provider: &'static str,
    },
    NodeCompleted {
        id: i64,
        status: String,
        ip: Option<String>,
        ip_version: Option<i64>,
        country_code: Option<String>,
        country_name: Option<String>,
        message: Option<String>,
        probed_at: String,
    },
    IntelligenceUpdated {
        id: i64,
        country_code: Option<String>,
        country_name: Option<String>,
        country_source: Option<String>,
        intelligence_status: Option<String>,
        intelligence_message: Option<String>,
        intelligence_updated_at: Option<String>,
        risk_ip: Option<String>,
        risk_status: Option<String>,
        scamalytics_fraud_score: Option<i64>,
        scamalytics_isp_risk_score: Option<i64>,
        risk_checked_at: Option<String>,
        risk_expires_at_unix_ms: Option<i64>,
        risk_message: Option<String>,
    },
    JobCompleted {
        total_nodes: usize,
        succeeded: usize,
        failed: usize,
        cancelled: bool,
    },
}
