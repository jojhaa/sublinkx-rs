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
    },
    JobCompleted {
        total_nodes: usize,
        succeeded: usize,
        failed: usize,
        cancelled: bool,
    },
}
