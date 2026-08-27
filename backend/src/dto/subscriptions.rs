use serde::{Deserialize, Serialize};

use crate::{
    domain::subscription::{SubscriptionListItem, SubscriptionView},
    dto::common::{PaginationMeta, PaginationQuery},
};

#[derive(Debug, Default, Deserialize)]
pub struct SubscriptionListQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub group_id: Option<i64>,
    #[serde(default)]
    pub ungrouped: bool,
}

impl SubscriptionListQuery {
    pub fn pagination(&self) -> PaginationQuery {
        PaginationQuery::from_options(self.page, self.page_size)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSubscriptionRequest {
    pub name: String,
    pub description: Option<String>,
    pub default_client: Option<String>,
    pub template_id: Option<i64>,
    pub group_id: Option<i64>,
    pub enabled: Option<bool>,
    pub expires_at: Option<String>,
    #[serde(default)]
    pub node_group_ids: Vec<i64>,
    pub node_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubscriptionRequest {
    pub name: String,
    pub description: Option<String>,
    pub default_client: Option<String>,
    pub template_id: Option<i64>,
    pub group_id: Option<i64>,
    pub enabled: Option<bool>,
    pub expires_at: Option<String>,
    #[serde(default)]
    pub node_group_ids: Vec<i64>,
    pub node_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RenewSubscriptionRequest {
    pub days: i64,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionResponse {
    pub code: &'static str,
    pub data: SubscriptionView,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionListResponse {
    pub code: &'static str,
    pub data: Vec<SubscriptionListItem>,
    pub pagination: PaginationMeta,
}
