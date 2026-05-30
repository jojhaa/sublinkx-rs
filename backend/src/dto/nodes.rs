use serde::{Deserialize, Serialize};

use crate::domain::node::NodeView;

#[derive(Debug, Deserialize)]
pub struct CreateNodeRequest {
    pub name: Option<String>,
    pub raw_link: String,
    pub group_id: Option<i64>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportNodesFromSubscriptionRequest {
    pub url: String,
    pub group_id: Option<i64>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNodeRequest {
    pub name: Option<String>,
    pub raw_link: String,
    pub group_id: Option<i64>,
    pub enabled: Option<bool>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NodeLatencyBatchRequest {
    pub ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct NodeLatencyResult {
    pub id: i64,
    pub status: String,
    pub latency_ms: Option<u128>,
    pub message: Option<String>,
    pub tested_at: String,
}

#[derive(Debug, Serialize)]
pub struct NodeLatencyResponse {
    pub code: &'static str,
    pub data: NodeLatencyResult,
}

#[derive(Debug, Serialize)]
pub struct NodeLatencyBatchResponse {
    pub code: &'static str,
    pub data: Vec<NodeLatencyResult>,
}

#[derive(Debug, Serialize)]
pub struct NodeResponse {
    pub code: &'static str,
    pub data: NodeView,
}

#[derive(Debug, Serialize)]
pub struct NodeListResponse {
    pub code: &'static str,
    pub data: Vec<NodeView>,
}

#[derive(Debug, Serialize)]
pub struct NodeImportFailure {
    pub source: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct NodeFidelityWarning {
    pub target: String,
    pub name: String,
    pub protocol: String,
    pub missing_fields: Vec<String>,
    pub changed_fields: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct NodeImportResponse {
    pub code: &'static str,
    pub imported: usize,
    pub skipped: usize,
    pub failed: usize,
    pub template_id: Option<i64>,
    pub template_name: Option<String>,
    pub fidelity_warnings: Vec<NodeFidelityWarning>,
    pub data: Vec<NodeView>,
    pub failures: Vec<NodeImportFailure>,
}
