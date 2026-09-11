use std::{collections::HashMap, net::IpAddr, time::Duration};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        node::NodeView,
        node_ip_probe::{NodeIpProbeRecord, NodeRiskTraits},
    },
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

#[derive(Debug, Serialize)]
struct BatchIntelligenceRequest<'a> {
    ips: Vec<&'a str>,
}

#[derive(Debug, Deserialize)]
struct BatchIntelligenceResponse {
    items: Vec<BatchIntelligenceItem>,
}

#[derive(Debug, Deserialize)]
struct BatchIntelligenceItem {
    ip: String,
    status: String,
    profile: Option<IntelligenceProfile>,
    scamalytics: ScamalyticsCacheResponse,
}

#[derive(Debug, Deserialize)]
struct IntelligenceProfile {
    fresh: bool,
    country_code: Option<String>,
    usage_type: Option<String>,
    is_proxy: Option<bool>,
    is_vpn: Option<bool>,
    is_tor: Option<bool>,
    is_hosting: Option<bool>,
    is_abuser: Option<bool>,
    is_relay: Option<bool>,
    threat_level: Option<String>,
    is_botnet_c2: Option<bool>,
    #[serde(default)]
    field_sources: std::collections::HashMap<String, String>,
    #[serde(default)]
    conflicts: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ScamalyticsCacheResponse {
    status: String,
    fraud_score: Option<u8>,
    isp_risk_score: Option<u8>,
    expires_at_unix_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct IpRiskScoreLookup {
    pub cache_status: String,
    pub scamalytics_fraud_score: Option<u8>,
    pub scamalytics_isp_risk_score: Option<u8>,
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

#[cfg(test)]
async fn lookup_risk_scores(state: &AppState, ip: &str) -> Result<IpRiskScoreLookup, String> {
    if !configured(state) {
        return Err("IP intelligence integration is disabled".to_string());
    }
    let mut items = lookup_batch(state, &[ip]).await?;
    let intelligence = items
        .pop()
        .ok_or_else(|| "IP intelligence batch response omitted the requested IP".to_string())?;
    let fraud_score = intelligence.scamalytics.fraud_score;
    let isp_risk_score = intelligence.scamalytics.isp_risk_score;
    if fraud_score.is_some_and(|score| score > 100)
        || isp_risk_score.is_some_and(|score| score > 100)
    {
        return Err("IP intelligence returned a risk score outside 0 to 100".to_string());
    }

    Ok(IpRiskScoreLookup {
        cache_status: intelligence.scamalytics.status,
        scamalytics_fraud_score: fraud_score,
        scamalytics_isp_risk_score: isp_risk_score,
    })
}

pub async fn lookup_risk_scores_batch(
    state: &AppState,
    ips: &[String],
) -> Result<HashMap<String, IpRiskScoreLookup>, String> {
    if !configured(state) {
        return Err("IP intelligence integration is disabled".to_string());
    }
    let references = ips.iter().map(String::as_str).collect::<Vec<_>>();
    let items = lookup_batch(state, &references).await?;
    Ok(items
        .into_iter()
        .map(|item| {
            (
                item.ip,
                IpRiskScoreLookup {
                    cache_status: item.scamalytics.status,
                    scamalytics_fraud_score: item.scamalytics.fraud_score,
                    scamalytics_isp_risk_score: item.scamalytics.isp_risk_score,
                },
            )
        })
        .collect())
}

async fn lookup_batch(
    state: &AppState,
    ips: &[&str],
) -> Result<Vec<BatchIntelligenceItem>, String> {
    let response = request_json::<BatchIntelligenceResponse>(
        state,
        state
            .ip_intelligence_client
            .post(endpoint(state, "ips/batch"))
            .json(&BatchIntelligenceRequest { ips: ips.to_vec() }),
    )
    .await?;
    if response.items.len() != ips.len() {
        return Err("IP intelligence batch response has an unexpected item count".to_string());
    }
    for item in &response.items {
        if !matches!(item.status.as_str(), "fresh" | "stale" | "missing")
            || !matches!(
                item.scamalytics.status.as_str(),
                "fresh" | "stale" | "missing"
            )
            || item
                .scamalytics
                .fraud_score
                .is_some_and(|score| score > 100)
            || item
                .scamalytics
                .isp_risk_score
                .is_some_and(|score| score > 100)
        {
            return Err("IP intelligence returned an invalid risk response".to_string());
        }
    }
    Ok(response.items)
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

    let current = node_ip_probe_repo::find_by_node_id(&state.db, node.id)
        .await
        .map_err(|_| "failed to read node IP state".to_string())?
        .filter(|record| record.status == "ok" && record.ip.as_deref() == Some(ip))
        .ok_or_else(|| "node exit IP changed before intelligence sync started".to_string())?;
    let lookup_needed = current.country_code.is_none()
        || !local_risk_cache_is_fresh(&current, ip)
        || !local_risk_traits_cache_is_fresh(&current);
    mark_status(state, node.id, ip, "syncing", None).await?;
    let result = sync_node_remote(state, node, ip, latency_ms, lookup_needed).await;
    let intelligence = match result {
        Ok(result) => result,
        Err(message) => {
            let safe_message = truncate_message(&message);
            let _ = node_ip_probe_repo::update_risk_refresh_message(
                &state.db,
                node.id,
                ip,
                current.exit_ip_revision,
                &safe_message,
                &now_rfc3339(),
            )
            .await;
            let _ = mark_status(state, node.id, ip, "error", Some(&safe_message)).await;
            return Err(safe_message);
        }
    };
    let Some(intelligence) = intelligence else {
        return mark_status(state, node.id, ip, "enriched", None).await;
    };
    if intelligence.ip != ip {
        return Err("IP intelligence batch response returned a different IP".to_string());
    }

    let now = now_rfc3339();
    let mut country_available = current.country_code.is_some();
    let mut risk_decision_available = matches!(
        current.risk_status.as_deref(),
        Some("zero" | "low" | "normal" | "blocked")
    ) && current.risk_ip.as_deref() == Some(ip);
    let mut changed = false;
    if intelligence.status == "fresh"
        && let Some(profile) = intelligence.profile.as_ref()
        && profile.fresh
    {
        let traits = risk_traits_from_profile(profile)?;
        let traits_json = serde_json::to_string(&traits)
            .map_err(|_| "failed to serialize IP risk traits".to_string())?;
        changed |= current.risk_traits_json.as_deref() != Some(traits_json.as_str());
        node_ip_probe_repo::update_risk_traits(
            &state.db,
            node.id,
            ip,
            current.exit_ip_revision,
            node_ip_probe_repo::RiskTraitsSnapshot {
                json: &traits_json,
                expires_at_unix_ms: unix_time_millis() + i64::from(PROBE_INTERVAL_SECONDS) * 1_000,
                now: &now,
            },
        )
        .await
        .map_err(|_| "failed to save IP risk traits".to_string())?
        .ok_or_else(|| "node exit IP changed before risk traits were saved".to_string())?;

        if let Some(country_code) = profile.country_code.as_deref() {
            if country_code.len() != 2
                || !country_code.bytes().all(|byte| byte.is_ascii_uppercase())
            {
                return Err("IP intelligence returned an invalid country code".to_string());
            }
            let source = profile
                .field_sources
                .get("country_code")
                .map(|value| format!("ip-intelligence:{value}"))
                .unwrap_or_else(|| "ip-intelligence-rs".to_string());
            node_ip_probe_repo::update_country(
                &state.db,
                node.id,
                ip,
                country_code,
                None,
                &source.chars().take(64).collect::<String>(),
                &now,
            )
            .await
            .map_err(|_| "failed to save IP intelligence result".to_string())?
            .ok_or_else(|| "node exit IP changed before intelligence was saved".to_string())?;
            country_available = true;
            changed |= current.country_code.as_deref() != Some(country_code);
        }
    }

    if intelligence.scamalytics.status == "fresh"
        && let Some(fraud_score) = intelligence.scamalytics.fraud_score
    {
        let risk_status = classify_scamalytics_fraud(fraud_score);
        changed |= current.risk_status.as_deref() != Some(risk_status)
            || current.scamalytics_fraud_score != Some(i64::from(fraud_score));
        node_ip_probe_repo::update_risk_decision(
            &state.db,
            node.id,
            ip,
            current.exit_ip_revision,
            node_ip_probe_repo::RiskDecision {
                status: risk_status,
                fraud_score: i64::from(fraud_score),
                isp_risk_score: intelligence.scamalytics.isp_risk_score.map(i64::from),
                expires_at_unix_ms: intelligence.scamalytics.expires_at_unix_ms,
                now: &now,
            },
        )
        .await
        .map_err(|_| "failed to save node risk decision".to_string())?
        .ok_or_else(|| "node exit IP changed before risk decision was saved".to_string())?;
        risk_decision_available = true;
    } else {
        let message = match intelligence.scamalytics.status.as_str() {
            "stale" => "Scamalytics cache is stale; preserving the last valid decision",
            _ => "Scamalytics assessment is pending",
        };
        node_ip_probe_repo::update_risk_refresh_message(
            &state.db,
            node.id,
            ip,
            current.exit_ip_revision,
            message,
            &now,
        )
        .await
        .map_err(|_| "failed to save node risk refresh state".to_string())?
        .ok_or_else(|| "node exit IP changed before risk state was saved".to_string())?;
    }

    let record = mark_status(
        state,
        node.id,
        ip,
        if country_available && risk_decision_available {
            "enriched"
        } else {
            "pending"
        },
        (!(country_available && risk_decision_available)).then_some(if !country_available {
            "country intelligence is not available yet"
        } else {
            "Scamalytics assessment is not available yet"
        }),
    )
    .await?;
    if changed {
        state.clear_public_export_cache().await;
    }
    Ok(record)
}

async fn sync_node_remote(
    state: &AppState,
    node: &NodeView,
    ip: &str,
    latency_ms: Option<u32>,
    lookup_needed: bool,
) -> Result<Option<BatchIntelligenceItem>, String> {
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
    if !lookup_needed {
        return Ok(None);
    }
    let mut items = lookup_batch(state, &[ip]).await?;
    Ok(items.pop())
}

fn local_risk_cache_is_fresh(record: &NodeIpProbeRecord, ip: &str) -> bool {
    record.risk_ip.as_deref() == Some(ip)
        && matches!(
            record.risk_status.as_deref(),
            Some("zero" | "low" | "normal" | "blocked")
        )
        && record
            .risk_expires_at_unix_ms
            .is_some_and(|expires| expires > unix_time_millis())
}

fn local_risk_traits_cache_is_fresh(record: &NodeIpProbeRecord) -> bool {
    record.risk_ip == record.ip
        && record.risk_traits_json.is_some()
        && record
            .risk_traits_expires_at_unix_ms
            .is_some_and(|expires| expires > unix_time_millis())
}

fn risk_traits_from_profile(profile: &IntelligenceProfile) -> Result<NodeRiskTraits, String> {
    if profile.usage_type.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "residential"
                | "datacenter"
                | "mobile"
                | "business"
                | "education"
                | "government"
                | "unknown"
        )
    }) || profile.threat_level.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "informational" | "low" | "medium" | "high" | "critical"
        )
    }) || profile.conflicts.len() > 32
        || profile
            .conflicts
            .iter()
            .any(|conflict| conflict.is_empty() || conflict.len() > 64)
    {
        return Err("IP intelligence returned invalid risk traits".to_string());
    }
    Ok(NodeRiskTraits {
        usage_type: profile.usage_type.clone(),
        is_proxy: profile.is_proxy,
        is_vpn: profile.is_vpn,
        is_tor: profile.is_tor,
        is_hosting: profile.is_hosting,
        is_abuser: profile.is_abuser,
        is_relay: profile.is_relay,
        threat_level: profile.threat_level.clone(),
        is_botnet_c2: profile.is_botnet_c2,
        conflicts: profile.conflicts.clone(),
    })
}

fn unix_time_millis() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp() * 1_000
}

pub(crate) fn classify_scamalytics_fraud(score: u8) -> &'static str {
    match score {
        0 => "zero",
        1..=20 => "low",
        21..=50 => "normal",
        _ => "blocked",
    }
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

    use axum::{Json, Router, http::HeaderMap, routing::post};
    use serde_json::{Value, json};

    use super::{classify_scamalytics_fraud, lookup_risk_scores, sync_node, truncate_message};
    use crate::{
        config::{AppConfig, DatabaseConfig, IpIntelligenceConfig, SecurityConfig, ServerConfig},
        db::new_database_pool,
        domain::{node::NodeView, node_ip_probe::NodeRiskTraits},
        repository::node_ip_probe_repo,
        state::AppState,
    };

    #[test]
    fn bounds_integration_errors() {
        assert_eq!(truncate_message(&"x".repeat(400)).len(), 300);
    }

    #[test]
    fn classifies_scamalytics_fraud_boundaries() {
        assert_eq!(classify_scamalytics_fraud(0), "zero");
        assert_eq!(classify_scamalytics_fraud(1), "low");
        assert_eq!(classify_scamalytics_fraud(20), "low");
        assert_eq!(classify_scamalytics_fraud(21), "normal");
        assert_eq!(classify_scamalytics_fraud(50), "normal");
        assert_eq!(classify_scamalytics_fraud(51), "blocked");
        assert_eq!(classify_scamalytics_fraud(100), "blocked");
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
                "/api/v1/ips/batch",
                post(
                    move |headers: HeaderMap, Json(body): Json<Value>| async move {
                        lookup_auth(&headers);
                        assert_eq!(body["ips"], json!(["1.1.1.1"]));
                        Json(json!({
                            "items": [{
                                "ip": "1.1.1.1",
                                "status": "fresh",
                                "profile": {
                                    "fresh": true,
                                    "country_code": "JP",
                                    "usage_type": "residential",
                                    "is_proxy": false,
                                    "is_vpn": false,
                                    "is_tor": false,
                                    "is_hosting": false,
                                    "is_abuser": false,
                                    "is_relay": false,
                                    "threat_level": null,
                                    "is_botnet_c2": false,
                                    "conflicts": [],
                                    "field_sources": { "country_code": "ipinfo-lite" }
                                },
                                "scamalytics": {
                                    "status": "fresh",
                                    "fraud_score": 47,
                                    "isp_risk_score": 22,
                                    "expires_at_unix_ms": 1893456000000_i64
                                }
                            }]
                        }))
                    },
                ),
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
        assert_eq!(record.risk_status.as_deref(), Some("normal"));
        assert_eq!(record.scamalytics_fraud_score, Some(47));
        let traits =
            serde_json::from_str::<NodeRiskTraits>(record.risk_traits_json.as_deref().unwrap())
                .unwrap();
        assert_eq!(traits.usage_type.as_deref(), Some("residential"));
        assert_eq!(traits.is_vpn, Some(false));
        assert!(record.risk_traits_expires_at_unix_ms.is_some());
        assert_eq!(
            record.country_source.as_deref(),
            Some("ip-intelligence:ipinfo-lite")
        );
        let risk = lookup_risk_scores(&state, "1.1.1.1").await.unwrap();
        assert_eq!(risk.cache_status, "fresh");
        assert_eq!(risk.scamalytics_fraud_score, Some(47));
        assert_eq!(risk.scamalytics_isp_risk_score, Some(22));

        pool.close().await;
        server.abort();
        let _ = std::fs::remove_file(absolute_path);
    }
}
