use serde::{Deserialize, Serialize};

use crate::domain::upstream_subscription::UpstreamSubscriptionView;

use super::nodes::NodeImportResponse;

#[derive(Debug, Deserialize)]
pub struct UpstreamSubscriptionPayload {
    pub name: String,
    pub url: String,
    pub group_id: Option<i64>,
    pub enabled: Option<bool>,
    pub remark: Option<String>,
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
