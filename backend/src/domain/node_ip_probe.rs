use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct NodeIpProbeRecord {
    pub node_id: i64,
    pub status: String,
    pub ip: Option<String>,
    pub ip_version: Option<i64>,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub country_source: Option<String>,
    pub intelligence_status: Option<String>,
    pub intelligence_message: Option<String>,
    pub message: Option<String>,
    pub probed_at: String,
    pub country_updated_at: Option<String>,
    pub intelligence_updated_at: Option<String>,
    pub updated_at: String,
}
