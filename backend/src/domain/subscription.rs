use serde::Serialize;
use sqlx::FromRow;

use crate::domain::node::NodeView;

#[derive(Debug, Clone, FromRow)]
pub struct SubscriptionRecord {
    pub id: i64,
    pub name: String,
    pub token: String,
    pub description: String,
    pub default_client: Option<String>,
    pub template_id: Option<i64>,
    pub group_id: Option<i64>,
    pub enabled: bool,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct SubscriptionNodeRecord {
    pub subscription_id: i64,
    pub node_id: i64,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionView {
    pub id: i64,
    pub name: String,
    pub token: String,
    pub description: String,
    pub default_client: Option<String>,
    pub template_id: Option<i64>,
    pub group_id: Option<i64>,
    pub enabled: bool,
    pub expires_at: Option<String>,
    pub status: String,
    pub node_ids: Vec<i64>,
    pub nodes: Vec<NodeView>,
    pub created_at: String,
    pub updated_at: String,
}
