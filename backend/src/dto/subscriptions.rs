use serde::{Deserialize, Serialize};

use crate::domain::subscription::SubscriptionView;

#[derive(Debug, Deserialize)]
pub struct CreateSubscriptionRequest {
    pub name: String,
    pub description: Option<String>,
    pub default_client: Option<String>,
    pub template_id: Option<i64>,
    pub group_id: Option<i64>,
    pub enabled: Option<bool>,
    pub expires_at: Option<String>,
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
    pub data: Vec<SubscriptionView>,
}
