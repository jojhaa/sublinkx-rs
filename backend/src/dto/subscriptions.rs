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
    pub include_rules: Option<bool>,
    pub portal_enabled: Option<bool>,
    pub portal_access_code: Option<String>,
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
    pub include_rules: Option<bool>,
    pub portal_enabled: Option<bool>,
    pub portal_access_code: Option<String>,
    #[serde(default)]
    pub node_group_ids: Vec<i64>,
    pub node_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RenewSubscriptionRequest {
    pub days: i64,
}

#[derive(Debug, Deserialize)]
pub struct UnlockSubscriptionPortalRequest {
    pub access_code: String,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionPortalLink {
    pub target: String,
    pub label: String,
    pub path: String,
    pub full_profile: bool,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionPortalData {
    pub include_rules: bool,
    pub name: String,
    pub description: String,
    pub expires_at: Option<String>,
    pub default_client: Option<String>,
    pub links: Vec<SubscriptionPortalLink>,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionPortalResponse {
    pub code: &'static str,
    pub data: SubscriptionPortalData,
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

#[cfg(test)]
mod content_choice_tests {
    #[test]
    fn old_requests_leave_content_choice_unspecified() {
        let old = serde_json::json!({"name":"test", "node_ids":[1]});
        let create: super::CreateSubscriptionRequest = serde_json::from_value(old.clone()).unwrap();
        let update: super::UpdateSubscriptionRequest = serde_json::from_value(old).unwrap();
        assert_eq!(create.include_rules, None);
        assert_eq!(update.include_rules, None);
        assert!(
            serde_json::from_value::<super::CreateSubscriptionRequest>(
                serde_json::json!({"name":"test","node_ids":[],"include_rules":"false"})
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn sqlite_upgrade_preserves_old_tokens_and_defaults_to_nodes_only() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql("CREATE TABLE subscriptions (id INTEGER PRIMARY KEY, token TEXT); INSERT INTO subscriptions VALUES (1,'unchanged');").execute(&pool).await.unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/0023_subscription_include_rules.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let value: (String, i64) =
            sqlx::query_as("SELECT token,include_rules FROM subscriptions WHERE id=1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(value, ("unchanged".to_string(), 0));
    }
}
