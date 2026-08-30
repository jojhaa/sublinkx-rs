use std::{net::IpAddr, time::Duration};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    domain::{node::NodeView, node_ip_probe::NodeIpProbeRecord},
    repository::node_ip_probe_repo,
    state::AppState,
    utils::time::now_rfc3339,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_INTERVAL_SECONDS: i32 = 43_200;

#[derive(Debug, Serialize)]
struct UpsertTargetRequest<'a> {
    source_key: &'a str,
    external_key: String,
    display_name: &'a str,
    target_kind: &'static str,
    target_value: &'a str,
    target_port: u16,
    enabled: bool,
    probe_interval_seconds: i32,
}

#[derive(Debug, Deserialize)]
struct TargetResponse {
    id: i64,
}

#[derive(Debug, Serialize)]
struct SubmitObservationRequest<'a> {
    observation_key: String,
    observations: [ObservationEvidence<'a>; 1],
}

#[derive(Debug, Serialize)]
struct ObservationEvidence<'a> {
    detector_key: &'static str,
    ip: &'a str,
    latency_ms: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct IntelligenceResponse {
    status: String,
    profile: Option<IntelligenceProfile>,
}

#[derive(Debug, Deserialize)]
struct IntelligenceProfile {
    fresh: bool,
    country_code: Option<String>,
    #[serde(default)]
    field_sources: std::collections::HashMap<String, String>,
}

pub fn configured(state: &AppState) -> bool {
    state.config.ip_intelligence.enabled
}

pub async fn check_connection(state: &AppState) -> Result<(), String> {
    if !configured(state) {
        return Err("IP intelligence integration is disabled".to_string());
    }
    request(
        state,
        state.ip_intelligence_client.get(endpoint(state, "stats")),
    )
    .await
    .map(|_| ())
}

pub async fn sync_node(
    state: &AppState,
    node: &NodeView,
    ip: &str,
    latency_ms: Option<u32>,
) -> Result<NodeIpProbeRecord, String> {
    if !configured(state) {
        return mark_status(state, node.id, ip, "disabled", None).await;
    }

    mark_status(state, node.id, ip, "syncing", None).await?;
    let result = sync_node_remote(state, node, ip, latency_ms).await;
    match result {
        Ok(Some((country_code, source))) => {
            let now = now_rfc3339();
            let record = node_ip_probe_repo::update_country(
                &state.db,
                node.id,
                ip,
                &country_code,
                None,
                &source,
                &now,
            )
            .await
            .map_err(|_| "failed to save IP intelligence result".to_string())?
            .ok_or_else(|| "node exit IP changed before intelligence was saved".to_string())?;
            state.clear_public_export_cache().await;
            Ok(record)
        }
        Ok(None) => {
            mark_status(
                state,
                node.id,
                ip,
                "pending",
                Some("country intelligence is not available yet"),
            )
            .await
        }
        Err(message) => {
            let safe_message = truncate_message(&message);
            let _ = mark_status(state, node.id, ip, "error", Some(&safe_message)).await;
            Err(safe_message)
        }
    }
}

async fn sync_node_remote(
    state: &AppState,
    node: &NodeView,
    ip: &str,
    latency_ms: Option<u32>,
) -> Result<Option<(String, String)>, String> {
    let target_port = u16::try_from(node.port)
        .map_err(|_| "node port is outside the supported range".to_string())?;
    let server = node.server.trim_matches(['[', ']']);
    let target_kind = if server.parse::<IpAddr>().is_ok() {
        "ip"
    } else {
        "hostname"
    };
    let target = request_json::<TargetResponse>(
        state,
        state
            .ip_intelligence_client
            .post(endpoint(state, "targets"))
            .json(&UpsertTargetRequest {
                source_key: &state.config.ip_intelligence.source_key,
                external_key: format!("node-{}", node.id),
                display_name: &node.name,
                target_kind,
                target_value: server,
                target_port,
                enabled: node.enabled && !node.upstream_missing,
                probe_interval_seconds: PROBE_INTERVAL_SECONDS,
            }),
    )
    .await?;

    let timestamp = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    request(
        state,
        state
            .ip_intelligence_client
            .post(endpoint(
                state,
                &format!("targets/{}/exit-observations", target.id),
            ))
            .json(&SubmitObservationRequest {
                observation_key: format!("sublinkx:{}:{timestamp}", node.id),
                observations: [ObservationEvidence {
                    detector_key: "ipify",
                    ip,
                    latency_ms,
                }],
            }),
    )
    .await?;

    let intelligence = request_json::<IntelligenceResponse>(
        state,
        state
            .ip_intelligence_client
            .get(endpoint(state, &format!("ips/{ip}"))),
    )
    .await?;
    let Some(profile) = intelligence.profile else {
        return Ok(None);
    };
    if intelligence.status != "fresh" || !profile.fresh {
        return Ok(None);
    }
    let Some(country_code) = profile.country_code else {
        return Ok(None);
    };
    if country_code.len() != 2 || !country_code.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err("IP intelligence returned an invalid country code".to_string());
    }
    let source = profile
        .field_sources
        .get("country_code")
        .map(|value| format!("ip-intelligence:{value}"))
        .unwrap_or_else(|| "ip-intelligence-rs".to_string());
    Ok(Some((country_code, source.chars().take(64).collect())))
}

async fn mark_status(
    state: &AppState,
    node_id: i64,
    ip: &str,
    status: &str,
    message: Option<&str>,
) -> Result<NodeIpProbeRecord, String> {
    node_ip_probe_repo::update_intelligence_status(
        &state.db,
        node_id,
        ip,
        status,
        message,
        &now_rfc3339(),
    )
    .await
    .map_err(|_| "failed to save IP intelligence status".to_string())?
    .ok_or_else(|| "node exit IP changed before intelligence status was saved".to_string())
}

async fn request_json<T: for<'de> Deserialize<'de>>(
    state: &AppState,
    builder: reqwest::RequestBuilder,
) -> Result<T, String> {
    let response = request(state, builder).await?;
    response
        .json::<T>()
        .await
        .map_err(|_| "IP intelligence service returned an invalid response".to_string())
}

async fn request(
    state: &AppState,
    builder: reqwest::RequestBuilder,
) -> Result<reqwest::Response, String> {
    let response = builder
        .bearer_auth(&state.config.ip_intelligence.api_token)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|error| format!("IP intelligence service is unavailable: {error}"))?;
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let message = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            "IP intelligence authentication failed".to_string()
        }
        StatusCode::TOO_MANY_REQUESTS => "IP intelligence rate limit was reached".to_string(),
        _ => format!("IP intelligence service returned {status}"),
    };
    Err(message)
}

fn endpoint(state: &AppState, path: &str) -> String {
    format!(
        "{}/{}",
        state.config.ip_intelligence.base_url,
        path.trim_start_matches('/')
    )
}

fn truncate_message(message: &str) -> String {
    message.chars().take(300).collect()
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::SystemTime};

    use axum::{
        Json, Router,
        http::HeaderMap,
        routing::{get, post},
    };
    use serde_json::{Value, json};

    use super::{sync_node, truncate_message};
    use crate::{
        config::{AppConfig, DatabaseConfig, IpIntelligenceConfig, SecurityConfig, ServerConfig},
        db::new_database_pool,
        domain::node::NodeView,
        repository::node_ip_probe_repo,
        state::AppState,
    };

    #[test]
    fn bounds_integration_errors() {
        assert_eq!(truncate_message(&"x".repeat(400)).len(), 300);
    }

    #[tokio::test]
    async fn submits_exit_observation_and_persists_fresh_country() {
        let token = Arc::<str>::from("0123456789abcdef0123456789abcdef");
        let assert_auth = {
            let token = token.clone();
            move |headers: &HeaderMap| {
                assert_eq!(
                    headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok()),
                    Some(format!("Bearer {token}").as_str())
                );
            }
        };
        let target_auth = assert_auth.clone();
        let observation_auth = assert_auth.clone();
        let lookup_auth = assert_auth;
        let app = Router::new()
            .route(
                "/api/v1/targets",
                post(
                    move |headers: HeaderMap, Json(body): Json<Value>| async move {
                        target_auth(&headers);
                        assert_eq!(body["external_key"], "node-1");
                        Json(json!({ "id": 7 }))
                    },
                ),
            )
            .route(
                "/api/v1/targets/7/exit-observations",
                post(
                    move |headers: HeaderMap, Json(body): Json<Value>| async move {
                        observation_auth(&headers);
                        assert_eq!(body["observations"][0]["detector_key"], "ipify");
                        assert_eq!(body["observations"][0]["ip"], "1.1.1.1");
                        Json(json!({ "created": true }))
                    },
                ),
            )
            .route(
                "/api/v1/ips/1.1.1.1",
                get(move |headers: HeaderMap| async move {
                    lookup_auth(&headers);
                    Json(json!({
                        "status": "fresh",
                        "profile": {
                            "fresh": true,
                            "country_code": "JP",
                            "field_sources": { "country_code": "ipinfo-lite" }
                        }
                    }))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let relative_path = format!("backend/target/test-data/ip-intelligence-{nonce}.db");
        let absolute_path = std::env::current_dir().unwrap().join(&relative_path);
        let pool = new_database_pool(&format!("sqlite://{relative_path}"))
            .await
            .unwrap();
        let now = "2026-08-29T00:00:00Z";
        sqlx::query(
            r#"
            INSERT INTO nodes (
              id, name, protocol, raw_link, server, port, enabled, group_id, source_type,
              source_ref, upstream_missing, fingerprint, fingerprint_scope, settings_json,
              remark, created_at, updated_at
            ) VALUES (1, 'JP-01', 'vless', 'vless://test', 'node.example', 443, 1, NULL,
              'manual', NULL, 0, ?, 0, '{}', '', ?, ?)
            "#,
        )
        .bind(format!("ip-intelligence-{nonce}"))
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
        node_ip_probe_repo::save_success(&pool, 1, "1.1.1.1", 4, now)
            .await
            .unwrap();
        let config = AppConfig {
            server: ServerConfig {
                port: 0,
                environment: "test".to_string(),
            },
            database: DatabaseConfig {
                url: format!("sqlite://{relative_path}"),
            },
            security: SecurityConfig {
                jwt_secret: "test".repeat(16),
                jwt_exp_hours: 24,
                bootstrap_admin_username: "admin".to_string(),
                bootstrap_admin_password: "password".to_string(),
                trust_proxy_headers: false,
                auth_cookie_secure: false,
            },
            ip_intelligence: IpIntelligenceConfig {
                enabled: true,
                base_url: format!("http://{address}/api/v1"),
                api_token: token.to_string(),
                source_key: "sublinkx-rs:test".to_string(),
            },
        };
        let state = AppState::new(config, pool.clone());
        let node = NodeView {
            id: 1,
            name: "JP-01".to_string(),
            protocol: "vless".to_string(),
            raw_link: String::new(),
            server: "node.example".to_string(),
            port: 443,
            enabled: true,
            group_id: None,
            source_type: "manual".to_string(),
            source_ref: None,
            upstream_missing: false,
            fingerprint: "test".to_string(),
            settings: json!({}),
            remark: String::new(),
            last_latency_ms: None,
            last_latency_status: None,
            last_latency_message: None,
            last_latency_tested_at: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        };

        let record = sync_node(&state, &node, "1.1.1.1", Some(25)).await.unwrap();
        assert_eq!(record.country_code.as_deref(), Some("JP"));
        assert_eq!(record.intelligence_status.as_deref(), Some("enriched"));
        assert_eq!(
            record.country_source.as_deref(),
            Some("ip-intelligence:ipinfo-lite")
        );

        pool.close().await;
        server.abort();
        let _ = std::fs::remove_file(absolute_path);
    }
}
