use serde::Serialize;
use sqlx::FromRow;

use crate::protocols::{Protocol, SupportLevel};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Node {
    pub id: i64,
    pub name: String,
    pub protocol: Protocol,
    pub server: String,
    pub port: u16,
    pub enabled: bool,
    pub compatibility: NodeCompatibility,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeCompatibility {
    pub mihomo: SupportLevel,
    pub stash: SupportLevel,
    pub surge: SupportLevel,
    pub xray_family: SupportLevel,
    pub sing_box_family: SupportLevel,
    pub shadowrocket: SupportLevel,
}

#[derive(Debug, Clone, FromRow)]
pub struct NodeRecord {
    pub id: i64,
    pub name: String,
    pub protocol: String,
    pub raw_link: String,
    pub server: String,
    pub port: i64,
    pub enabled: bool,
    pub group_id: Option<i64>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub fingerprint: String,
    pub settings_json: String,
    pub remark: String,
    pub last_latency_ms: Option<i64>,
    pub last_latency_status: Option<String>,
    pub last_latency_message: Option<String>,
    pub last_latency_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeView {
    pub id: i64,
    pub name: String,
    pub protocol: String,
    pub raw_link: String,
    pub server: String,
    pub port: i64,
    pub enabled: bool,
    pub group_id: Option<i64>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub fingerprint: String,
    pub settings: serde_json::Value,
    pub remark: String,
    pub last_latency_ms: Option<i64>,
    pub last_latency_status: Option<String>,
    pub last_latency_message: Option<String>,
    pub last_latency_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<NodeRecord> for NodeView {
    type Error = serde_json::Error;

    fn try_from(value: NodeRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            name: value.name,
            protocol: value.protocol,
            raw_link: value.raw_link,
            server: value.server,
            port: value.port,
            enabled: value.enabled,
            group_id: value.group_id,
            source_type: value.source_type,
            source_ref: value.source_ref,
            fingerprint: value.fingerprint,
            settings: serde_json::from_str(&value.settings_json)?,
            remark: value.remark,
            last_latency_ms: value.last_latency_ms,
            last_latency_status: value.last_latency_status,
            last_latency_message: value.last_latency_message,
            last_latency_tested_at: value.last_latency_tested_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}
