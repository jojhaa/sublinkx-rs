use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub nodes: i64,
    pub subscriptions: i64,
    pub templates: i64,
    pub upstream_subscriptions: i64,
}

#[derive(Debug, Serialize)]
pub struct DashboardStatsResponse {
    pub code: &'static str,
    pub data: DashboardStats,
}
