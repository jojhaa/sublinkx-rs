use crate::{
    db::{DbPool, DbQueryBuilder, database_sql_owned, query, query_scalar},
    domain::node_ip_probe::NodeIpProbeRecord,
};

const SELECT_FIELDS: &str = r#"
node_id, status, ip, ip_version, exit_ip_revision, country_code, country_name, country_source,
intelligence_status, intelligence_message, risk_ip, risk_status, scamalytics_fraud_score,
scamalytics_isp_risk_score, risk_checked_at, risk_expires_at_unix_ms, risk_message,
 risk_traits_json, risk_traits_expires_at_unix_ms,
message, probed_at, country_updated_at, intelligence_updated_at, updated_at
"#;

pub struct RiskDecision<'a> {
    pub status: &'a str,
    pub fraud_score: i64,
    pub isp_risk_score: Option<i64>,
    pub expires_at_unix_ms: Option<i64>,
    pub now: &'a str,
}

pub struct RiskTraitsSnapshot<'a> {
    pub json: &'a str,
    pub expires_at_unix_ms: i64,
    pub now: &'a str,
}

pub async fn list(pool: &DbPool) -> Result<Vec<NodeIpProbeRecord>, sqlx::Error> {
    sqlx::query_as::<_, NodeIpProbeRecord>(&format!(
        "SELECT {SELECT_FIELDS} FROM node_ip_probes ORDER BY node_id ASC"
    ))
    .fetch_all(pool)
    .await
}

pub async fn find_by_node_id(
    pool: &DbPool,
    node_id: i64,
) -> Result<Option<NodeIpProbeRecord>, sqlx::Error> {
    let sql = database_sql_owned(format!(
        "SELECT {SELECT_FIELDS} FROM node_ip_probes WHERE node_id = ?"
    ));
    sqlx::query_as::<_, NodeIpProbeRecord>(&sql)
        .bind(node_id)
        .fetch_optional(pool)
        .await
}

pub async fn save_success(
    pool: &DbPool,
    node_id: i64,
    ip: &str,
    ip_version: i64,
    now: &str,
) -> Result<NodeIpProbeRecord, sqlx::Error> {
    let updated = query(
        r#"
        UPDATE node_ip_probes
        SET status = 'ok',
            exit_ip_revision = CASE WHEN ip = ? THEN exit_ip_revision ELSE exit_ip_revision + 1 END,
            country_code = CASE WHEN ip = ? THEN country_code ELSE NULL END,
            country_name = CASE WHEN ip = ? THEN country_name ELSE NULL END,
            country_source = CASE WHEN ip = ? THEN country_source ELSE NULL END,
            country_updated_at = CASE WHEN ip = ? THEN country_updated_at ELSE NULL END,
            intelligence_status = CASE WHEN ip = ? THEN intelligence_status ELSE NULL END,
            intelligence_message = CASE WHEN ip = ? THEN intelligence_message ELSE NULL END,
            intelligence_updated_at = CASE WHEN ip = ? THEN intelligence_updated_at ELSE NULL END,
            risk_ip = CASE WHEN ip = ? THEN risk_ip ELSE ? END,
            risk_status = CASE WHEN ip = ? THEN risk_status ELSE 'pending' END,
            scamalytics_fraud_score = CASE WHEN ip = ? THEN scamalytics_fraud_score ELSE NULL END,
            scamalytics_isp_risk_score = CASE WHEN ip = ? THEN scamalytics_isp_risk_score ELSE NULL END,
            risk_checked_at = CASE WHEN ip = ? THEN risk_checked_at ELSE NULL END,
            risk_expires_at_unix_ms = CASE WHEN ip = ? THEN risk_expires_at_unix_ms ELSE NULL END,
            risk_message = CASE WHEN ip = ? THEN risk_message ELSE 'awaiting risk assessment' END,
            risk_traits_json = CASE WHEN ip = ? THEN risk_traits_json ELSE NULL END,
            risk_traits_expires_at_unix_ms = CASE WHEN ip = ? THEN risk_traits_expires_at_unix_ms ELSE NULL END,
            ip = ?, ip_version = ?, message = NULL, probed_at = ?, updated_at = ?
        WHERE node_id = ?
        "#,
    )
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip)
    .bind(ip_version)
    .bind(now)
    .bind(now)
    .bind(node_id)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 0 {
        query(
            r#"
            INSERT INTO node_ip_probes (
              node_id, status, ip, ip_version, exit_ip_revision,
              country_code, country_name, country_source,
              intelligence_status, intelligence_message,
              risk_ip, risk_status, risk_message, message, probed_at,
              country_updated_at, intelligence_updated_at, updated_at
            ) VALUES (?, 'ok', ?, ?, 1, NULL, NULL, NULL, NULL, NULL,
              ?, 'pending', 'awaiting risk assessment', NULL, ?, NULL, NULL, ?)
            "#,
        )
        .bind(node_id)
        .bind(ip)
        .bind(ip_version)
        .bind(ip)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    }

    find_by_node_id(pool, node_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn invalidate_risk_for_node_ids(
    pool: &DbPool,
    node_ids: &[i64],
    now: &str,
) -> Result<u64, sqlx::Error> {
    if node_ids.is_empty() {
        return Ok(0);
    }
    let mut builder = DbQueryBuilder::new(
        r#"
        UPDATE node_ip_probes
        SET exit_ip_revision = exit_ip_revision + 1,
            risk_status = 'pending', risk_message = 'awaiting exit IP revalidation',
            risk_expires_at_unix_ms = NULL, risk_traits_json = NULL,
            risk_traits_expires_at_unix_ms = NULL, updated_at =
        "#,
    );
    builder.push_bind(now);
    builder.push(" WHERE node_id IN (");
    {
        let mut separated = builder.separated(", ");
        for node_id in node_ids {
            separated.push_bind(*node_id);
        }
    }
    builder.push(")");
    Ok(builder.build().execute(pool).await?.rows_affected())
}

pub async fn update_risk_decision(
    pool: &DbPool,
    node_id: i64,
    ip: &str,
    exit_ip_revision: i64,
    decision: RiskDecision<'_>,
) -> Result<Option<NodeIpProbeRecord>, sqlx::Error> {
    let result = query(
        r#"
        UPDATE node_ip_probes
        SET risk_ip = ?, risk_status = ?, scamalytics_fraud_score = ?,
            scamalytics_isp_risk_score = ?, risk_checked_at = ?,
            risk_expires_at_unix_ms = ?, risk_message = NULL, updated_at = ?
        WHERE node_id = ? AND ip = ? AND exit_ip_revision = ?
        "#,
    )
    .bind(ip)
    .bind(decision.status)
    .bind(decision.fraud_score)
    .bind(decision.isp_risk_score)
    .bind(decision.now)
    .bind(decision.expires_at_unix_ms)
    .bind(decision.now)
    .bind(node_id)
    .bind(ip)
    .bind(exit_ip_revision)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    find_by_node_id(pool, node_id).await
}

pub async fn update_risk_refresh_message(
    pool: &DbPool,
    node_id: i64,
    ip: &str,
    exit_ip_revision: i64,
    message: &str,
    now: &str,
) -> Result<Option<NodeIpProbeRecord>, sqlx::Error> {
    let result = query(
        r#"
        UPDATE node_ip_probes
        SET risk_message = ?, updated_at = ?
        WHERE node_id = ? AND ip = ? AND exit_ip_revision = ?
        "#,
    )
    .bind(message)
    .bind(now)
    .bind(node_id)
    .bind(ip)
    .bind(exit_ip_revision)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    find_by_node_id(pool, node_id).await
}

pub async fn update_risk_traits(
    pool: &DbPool,
    node_id: i64,
    ip: &str,
    exit_ip_revision: i64,
    snapshot: RiskTraitsSnapshot<'_>,
) -> Result<Option<NodeIpProbeRecord>, sqlx::Error> {
    let result = query(
        r#"
        UPDATE node_ip_probes
        SET risk_traits_json = ?, risk_traits_expires_at_unix_ms = ?, updated_at = ?
        WHERE node_id = ? AND ip = ? AND exit_ip_revision = ?
        "#,
    )
    .bind(snapshot.json)
    .bind(snapshot.expires_at_unix_ms)
    .bind(snapshot.now)
    .bind(node_id)
    .bind(ip)
    .bind(exit_ip_revision)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    find_by_node_id(pool, node_id).await
}

pub fn risk_allows_export(record: Option<&NodeIpProbeRecord>, enforcement_enabled: bool) -> bool {
    !enforcement_enabled
        || record.is_some_and(|record| {
            record.risk_ip == record.ip
                && matches!(
                    record.risk_status.as_deref(),
                    Some("zero" | "low" | "normal")
                )
        })
}

pub async fn save_failure(
    pool: &DbPool,
    node_id: i64,
    message: &str,
    now: &str,
) -> Result<NodeIpProbeRecord, sqlx::Error> {
    let updated = query(
        "UPDATE node_ip_probes SET status = 'error', message = ?, probed_at = ?, updated_at = ? WHERE node_id = ?",
    )
    .bind(message)
    .bind(now)
    .bind(now)
    .bind(node_id)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 0 {
        query(
            r#"
            INSERT INTO node_ip_probes (
              node_id, status, ip, ip_version, country_code, country_name, country_source,
              message, probed_at, country_updated_at, updated_at
            ) VALUES (?, 'error', NULL, NULL, NULL, NULL, NULL, ?, ?, NULL, ?)
            "#,
        )
        .bind(node_id)
        .bind(message)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    }

    find_by_node_id(pool, node_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update_country(
    pool: &DbPool,
    node_id: i64,
    ip: &str,
    country_code: &str,
    country_name: Option<&str>,
    source: &str,
    now: &str,
) -> Result<Option<NodeIpProbeRecord>, sqlx::Error> {
    let result = query(
        r#"
        UPDATE node_ip_probes
        SET country_code = ?, country_name = ?, country_source = ?,
            country_updated_at = ?, intelligence_status = 'enriched',
            intelligence_message = NULL, intelligence_updated_at = ?, updated_at = ?
        WHERE node_id = ? AND ip = ?
        "#,
    )
    .bind(country_code)
    .bind(country_name)
    .bind(source)
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(node_id)
    .bind(ip)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }
    find_by_node_id(pool, node_id).await
}

pub async fn update_intelligence_status(
    pool: &DbPool,
    node_id: i64,
    ip: &str,
    status: &str,
    message: Option<&str>,
    now: &str,
) -> Result<Option<NodeIpProbeRecord>, sqlx::Error> {
    let result = query(
        r#"
        UPDATE node_ip_probes
        SET intelligence_status = ?, intelligence_message = ?,
            intelligence_updated_at = ?, updated_at = ?
        WHERE node_id = ? AND ip = ?
        "#,
    )
    .bind(status)
    .bind(message)
    .bind(now)
    .bind(now)
    .bind(node_id)
    .bind(ip)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }
    find_by_node_id(pool, node_id).await
}

pub async fn countries_by_node_ids(
    pool: &DbPool,
    node_ids: &[i64],
) -> Result<std::collections::HashMap<i64, String>, sqlx::Error> {
    if node_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut query = DbQueryBuilder::new(
        "SELECT node_id, country_code FROM node_ip_probes WHERE status = 'ok' AND ip IS NOT NULL AND country_code IS NOT NULL AND intelligence_status = 'enriched' AND node_id IN (",
    );
    {
        let mut separated = query.separated(", ");
        for node_id in node_ids {
            separated.push_bind(*node_id);
        }
    }
    query.push(")");
    let rows = query.build().fetch_all(pool).await?;
    let mut countries = std::collections::HashMap::with_capacity(rows.len());
    for row in rows {
        use sqlx::Row;
        countries.insert(row.try_get("node_id")?, row.try_get("country_code")?);
    }
    Ok(countries)
}

pub async fn records_by_node_ids(
    pool: &DbPool,
    node_ids: &[i64],
) -> Result<std::collections::HashMap<i64, NodeIpProbeRecord>, sqlx::Error> {
    if node_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut builder = DbQueryBuilder::new(format!(
        "SELECT {SELECT_FIELDS} FROM node_ip_probes WHERE node_id IN ("
    ));
    {
        let mut separated = builder.separated(", ");
        for node_id in node_ids {
            separated.push_bind(*node_id);
        }
    }
    builder.push(")");
    let records = builder
        .build_query_as::<NodeIpProbeRecord>()
        .fetch_all(pool)
        .await?;
    Ok(records
        .into_iter()
        .map(|record| (record.node_id, record))
        .collect())
}

pub async fn intelligence_refresh_candidates(
    pool: &DbPool,
    stale_before: &str,
    limit: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    query_scalar(
        r#"
        SELECT probe.node_id
        FROM node_ip_probes AS probe
        INNER JOIN nodes AS node ON node.id = probe.node_id
        WHERE probe.status = 'ok'
          AND probe.ip IS NOT NULL
          AND node.enabled = 1
          AND node.upstream_missing = 0
          AND (
            probe.intelligence_status IS NULL
            OR probe.intelligence_status <> 'enriched'
            OR probe.intelligence_updated_at IS NULL
            OR probe.intelligence_updated_at < ?
          )
        ORDER BY COALESCE(probe.intelligence_updated_at, '') ASC, probe.node_id ASC
        LIMIT ?
        "#,
    )
    .bind(stale_before)
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[tokio::test]
    async fn country_data_follows_the_exact_probed_ip() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let relative_path = format!(
            "backend/target/test-data/sublinkx-ip-probes-{}-{nonce}.db",
            std::process::id()
        );
        let absolute_path = std::env::current_dir()
            .expect("current directory should exist")
            .join(&relative_path);
        let pool = crate::db::new_database_pool(&format!("sqlite://{relative_path}"))
            .await
            .expect("test database should initialize");
        let now = "2026-08-28T00:00:00Z";

        sqlx::query(
            r#"
            INSERT INTO nodes (
              name, protocol, raw_link, server, port, enabled, group_id, source_type,
              source_ref, upstream_missing, fingerprint, fingerprint_scope, settings_json,
              remark, created_at, updated_at
            ) VALUES ('probe', 'vless', 'vless://test', 'example.com', 443, 1, NULL,
              'manual', NULL, 0, ?, 0, '{}', '', ?, ?)
            "#,
        )
        .bind(format!("ip-probe-{nonce}"))
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("node should insert");
        let node_id = sqlx::query_scalar::<_, i64>("SELECT id FROM nodes WHERE fingerprint = ?")
            .bind(format!("ip-probe-{nonce}"))
            .fetch_one(&pool)
            .await
            .expect("node should exist");

        let first = save_success(&pool, node_id, "1.1.1.1", 4, now)
            .await
            .expect("first probe should save");
        assert_eq!(first.exit_ip_revision, 1);
        assert_eq!(first.risk_status.as_deref(), Some("pending"));
        let _assessed = update_risk_decision(
            &pool,
            node_id,
            "1.1.1.1",
            first.exit_ip_revision,
            RiskDecision {
                status: "zero",
                fraud_score: 0,
                isp_risk_score: Some(5),
                expires_at_unix_ms: Some(1_893_456_000_000),
                now,
            },
        )
        .await
        .expect("risk decision should execute")
        .expect("risk decision should persist");
        let assessed = update_risk_traits(
            &pool,
            node_id,
            "1.1.1.1",
            first.exit_ip_revision,
            RiskTraitsSnapshot {
                json: r#"{"usage_type":"residential","is_proxy":false,"is_vpn":false,"is_tor":false,"is_hosting":false,"is_abuser":false,"is_relay":false,"threat_level":null,"is_botnet_c2":false,"conflicts":[]}"#,
                expires_at_unix_ms: 1_893_456_000_000,
                now,
            },
        )
        .await
        .expect("risk traits should execute")
        .expect("risk traits should persist");
        assert!(assessed.risk_traits_json.is_some());
        assert!(risk_allows_export(Some(&assessed), true));
        assert!(
            update_country(
                &pool,
                node_id,
                "8.8.8.8",
                "US",
                Some("United States"),
                "test",
                now,
            )
            .await
            .expect("stale update should execute")
            .is_none()
        );
        let enriched = update_country(
            &pool,
            node_id,
            "1.1.1.1",
            "AU",
            Some("Australia"),
            "test",
            now,
        )
        .await
        .expect("matching update should execute")
        .expect("matching update should persist");
        assert_eq!(enriched.country_code.as_deref(), Some("AU"));
        assert_eq!(
            countries_by_node_ids(&pool, &[node_id])
                .await
                .expect("country lookup should succeed")
                .get(&node_id)
                .map(String::as_str),
            Some("AU")
        );

        save_failure(&pool, node_id, "probe failed", now)
            .await
            .expect("failed probe should save");
        assert!(
            countries_by_node_ids(&pool, &[node_id])
                .await
                .expect("country lookup should succeed")
                .is_empty()
        );

        save_success(&pool, node_id, "1.1.1.1", 4, now)
            .await
            .expect("same IP should recover");
        assert_eq!(
            countries_by_node_ids(&pool, &[node_id])
                .await
                .expect("country lookup should succeed")
                .get(&node_id)
                .map(String::as_str),
            Some("AU")
        );

        let changed = save_success(&pool, node_id, "8.8.8.8", 4, now)
            .await
            .expect("changed probe should save");
        assert_eq!(changed.ip.as_deref(), Some("8.8.8.8"));
        assert!(changed.country_code.is_none());
        assert!(changed.country_name.is_none());
        assert!(changed.country_source.is_none());
        assert_eq!(changed.exit_ip_revision, first.exit_ip_revision + 1);
        assert_eq!(changed.risk_status.as_deref(), Some("pending"));
        assert!(changed.risk_traits_json.is_none());
        assert!(changed.risk_traits_expires_at_unix_ms.is_none());
        assert!(!risk_allows_export(Some(&changed), true));
        assert!(
            update_risk_decision(
                &pool,
                node_id,
                "8.8.8.8",
                first.exit_ip_revision,
                RiskDecision {
                    status: "blocked",
                    fraud_score: 80,
                    isp_risk_score: None,
                    expires_at_unix_ms: None,
                    now,
                },
            )
            .await
            .expect("stale risk decision should execute")
            .is_none()
        );
        assert!(
            countries_by_node_ids(&pool, &[node_id])
                .await
                .expect("country lookup should succeed")
                .is_empty()
        );

        pool.close().await;
        let _ = std::fs::remove_file(absolute_path);
    }
}
