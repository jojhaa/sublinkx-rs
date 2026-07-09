use serde::{Deserialize, Serialize};

use crate::domain::upstream_subscription::UpstreamSubscriptionView;

use super::nodes::NodeImportResponse;

#[derive(Debug, Deserialize)]
pub struct UpstreamSubscriptionPayload {
    pub name: String,
    pub url: String,
    pub enabled: Option<bool>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteUpstreamSubscriptionQuery {
    #[serde(default)]
    pub delete_nodes: bool,
}

#[derive(Debug, Serialize)]
pub struct DeleteUpstreamSubscriptionResponse {
    pub code: &'static str,
    pub message: &'static str,
    pub deleted_nodes: u64,
    pub detached_nodes: u64,
}

#[derive(Debug, Serialize)]
pub struct UpstreamSubscriptionListResponse {
    pub code: &'static str,
    pub data: Vec<UpstreamSubscriptionView>,
}

#[derive(Debug, Serialize)]
pub struct UpstreamSubscriptionResponse {
    pub code: &'static str,
    pub data: UpstreamSubscriptionView,
}

#[derive(Debug, Serialize)]
pub struct UpstreamSubscriptionImportResponse {
    pub code: &'static str,
    pub data: UpstreamSubscriptionView,
    pub import: NodeImportResponse,
}
