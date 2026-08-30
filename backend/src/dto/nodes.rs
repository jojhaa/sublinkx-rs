use serde::{Deserialize, Serialize};

use crate::domain::node::NodeView;
use crate::dto::common::{PaginationMeta, PaginationQuery};

#[derive(Debug, Default, Deserialize)]
pub struct NodeListQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub group_id: Option<i64>,
    #[serde(default)]
    pub ungrouped: bool,
    pub enabled: Option<bool>,
    #[serde(default)]
    pub compact: bool,
}

impl NodeListQuery {
    pub fn pagination(&self) -> PaginationQuery {
        PaginationQuery::from_options(self.page, self.page_size)
    }
}

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

#[derive(Debug, Deserialize)]
pub struct MoveNodesRequest {
    pub ids: Vec<i64>,
    pub group_id: Option<i64>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationMeta>,
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
    pub updated: usize,
    pub disabled: usize,
    pub skipped: usize,
    pub failed: usize,
    pub template_id: Option<i64>,
    pub template_name: Option<String>,
    pub fidelity_warnings: Vec<NodeFidelityWarning>,
    pub data: Vec<NodeView>,
    pub failures: Vec<NodeImportFailure>,
}
