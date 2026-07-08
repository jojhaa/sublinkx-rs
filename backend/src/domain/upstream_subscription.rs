use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct UpstreamSubscriptionRecord {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub group_id: Option<i64>,
    pub enabled: i64,
    pub remark: String,
    pub last_imported_at: Option<String>,
    pub last_import_status: Option<String>,
    pub last_import_message: Option<String>,
    pub last_import_imported: i64,
    pub last_import_skipped: i64,
    pub last_import_failed: i64,
    pub template_id: Option<i64>,
    pub template_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpstreamSubscriptionView {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub group_id: Option<i64>,
    pub enabled: bool,
    pub remark: String,
    pub last_imported_at: Option<String>,
    pub last_import_status: Option<String>,
    pub last_import_message: Option<String>,
    pub last_import_imported: i64,
    pub last_import_skipped: i64,
    pub last_import_failed: i64,
    pub template_id: Option<i64>,
    pub template_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<UpstreamSubscriptionRecord> for UpstreamSubscriptionView {
    fn from(value: UpstreamSubscriptionRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            url: value.url,
            group_id: value.group_id,
            enabled: value.enabled != 0,
            remark: value.remark,
            last_imported_at: value.last_imported_at,
            last_import_status: value.last_import_status,
            last_import_message: value.last_import_message,
            last_import_imported: value.last_import_imported,
            last_import_skipped: value.last_import_skipped,
            last_import_failed: value.last_import_failed,
            template_id: value.template_id,
            template_name: value.template_name,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
