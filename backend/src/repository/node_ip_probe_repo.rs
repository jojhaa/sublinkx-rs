use crate::{db::DbPool, domain::node_ip_probe::NodeIpProbeRecord};

const SELECT_FIELDS: &str = r#"
node_id, status, ip, ip_version, country_code, country_name, country_source,
intelligence_status, intelligence_message, message, probed_at, country_updated_at,
intelligence_updated_at, updated_at
"#;

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
    sqlx::query_as::<_, NodeIpProbeRecord>(&format!(
        "SELECT {SELECT_FIELDS} FROM node_ip_probes WHERE node_id = ?"
    ))
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
    let updated = sqlx::query(
        r#"
        UPDATE node_ip_probes
        SET status = 'ok',
            country_code = CASE WHEN ip = ? THEN country_code ELSE NULL END,
            country_name = CASE WHEN ip = ? THEN country_name ELSE NULL END,
            country_source = CASE WHEN ip = ? THEN country_source ELSE NULL END,
            country_updated_at = CASE WHEN ip = ? THEN country_updated_at ELSE NULL END,
            intelligence_status = CASE WHEN ip = ? THEN intelligence_status ELSE NULL END,
            intelligence_message = CASE WHEN ip = ? THEN intelligence_message ELSE NULL END,
            intelligence_updated_at = CASE WHEN ip = ? THEN intelligence_updated_at ELSE NULL END,
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
    .bind(ip_version)
    .bind(now)
    .bind(now)
    .bind(node_id)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 0 {
        sqlx::query(
            r#"
            INSERT INTO node_ip_probes (
              node_id, status, ip, ip_version, country_code, country_name, country_source,
              intelligence_status, intelligence_message, message, probed_at,
              country_updated_at, intelligence_updated_at, updated_at
            ) VALUES (?, 'ok', ?, ?, NULL, NULL, NULL, NULL, NULL, NULL, ?, NULL, NULL, ?)
            "#,
        )
        .bind(node_id)
        .bind(ip)
        .bind(ip_version)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    }

    find_by_node_id(pool, node_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn save_failure(
    pool: &DbPool,
    node_id: i64,
    message: &str,
    now: &str,
) -> Result<NodeIpProbeRecord, sqlx::Error> {
    let updated = sqlx::query(
        "UPDATE node_ip_probes SET status = 'error', message = ?, probed_at = ?, updated_at = ? WHERE node_id = ?",
    )
    .bind(message)
    .bind(now)
    .bind(now)
    .bind(node_id)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 0 {
        sqlx::query(
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
    let result = sqlx::query(
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
    let result = sqlx::query(
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
    let mut query = sqlx::QueryBuilder::<sqlx::Any>::new(
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

pub async fn intelligence_refresh_candidates(
    pool: &DbPool,
    stale_before: &str,
    limit: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar(
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

        save_success(&pool, node_id, "1.1.1.1", 4, now)
            .await
            .expect("first probe should save");
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
