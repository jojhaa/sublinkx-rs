use serde::{Deserialize, Serialize};

use crate::domain::upstream_subscription::UpstreamSubscriptionView;

use super::common::{PaginationMeta, PaginationQuery};
use super::nodes::NodeImportResponse;

#[derive(Debug, Default, Deserialize)]
pub struct UpstreamSubscriptionListQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl UpstreamSubscriptionListQuery {
    pub fn pagination(&self) -> PaginationQuery {
        PaginationQuery::from_options(self.page, self.page_size)
    }
}

#[derive(Debug, Deserialize)]
pub struct UpstreamSubscriptionPayload {
    pub name: String,
    pub url: String,
    pub enabled: Option<bool>,
    pub sync_enabled: Option<bool>,
    pub sync_interval_minutes: Option<i64>,
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
    pub pagination: PaginationMeta,
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
