use std::collections::HashSet;

use sqlx::{Any, QueryBuilder};

use crate::db::DbPool;
use crate::domain::node::NodeRecord;

const NODE_SELECT_FIELDS: &str = r#"
id, name, protocol, raw_link, server, port, enabled + 0 AS enabled, group_id, source_type, source_ref,
upstream_missing + 0 AS upstream_missing, fingerprint, settings_json, remark, last_latency_ms, last_latency_status,
last_latency_message, last_latency_tested_at, created_at, updated_at
"#;

pub struct NewNodeRecord<'a> {
    pub name: &'a str,
    pub protocol: &'a str,
    pub raw_link: &'a str,
    pub server: &'a str,
    pub port: i64,
    pub enabled: i64,
    pub group_id: Option<i64>,
    pub fingerprint_scope: i64,
    pub source_type: &'a str,
    pub source_ref: Option<&'a str>,
    pub upstream_missing: i64,
    pub fingerprint: &'a str,
    pub settings_json: &'a str,
    pub remark: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

pub struct UpdateNodeRecord<'a> {
    pub name: &'a str,
    pub protocol: &'a str,
    pub raw_link: &'a str,
    pub server: &'a str,
    pub port: i64,
    pub enabled: i64,
    pub group_id: Option<i64>,
    pub fingerprint_scope: i64,
    pub upstream_missing: i64,
    pub fingerprint: &'a str,
    pub settings_json: &'a str,
    pub remark: &'a str,
    pub updated_at: &'a str,
}

pub async fn count_filtered(
    pool: &DbPool,
    group_id: Option<i64>,
    ungrouped: bool,
    enabled: Option<bool>,
) -> Result<i64, sqlx::Error> {
    let mut query = QueryBuilder::<Any>::new("SELECT COUNT(*) FROM nodes WHERE 1 = 1");
    push_list_filters(&mut query, group_id, ungrouped, enabled);
    query.build_query_scalar().fetch_one(pool).await
}

pub async fn list_page(
    pool: &DbPool,
    group_id: Option<i64>,
    ungrouped: bool,
    enabled: Option<bool>,
    limit: i64,
    offset: i64,
) -> Result<Vec<NodeRecord>, sqlx::Error> {
    let mut query = QueryBuilder::<Any>::new(format!(
        "SELECT {NODE_SELECT_FIELDS} FROM nodes WHERE 1 = 1"
    ));
    push_list_filters(&mut query, group_id, ungrouped, enabled);
    query.push(" ORDER BY id DESC LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);
    query.build_query_as().fetch_all(pool).await
}

fn push_list_filters(
    query: &mut QueryBuilder<'_, Any>,
    group_id: Option<i64>,
    ungrouped: bool,
    enabled: Option<bool>,
) {
    if ungrouped {
        query.push(" AND group_id IS NULL");
    } else if let Some(group_id) = group_id {
        query.push(" AND group_id = ");
        query.push_bind(group_id);
    }
    if let Some(enabled) = enabled {
        query.push(" AND enabled = ");
        query.push_bind(if enabled { 1_i64 } else { 0_i64 });
    }
}

pub async fn find_by_ids(pool: &DbPool, ids: &[i64]) -> Result<Vec<NodeRecord>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = QueryBuilder::<Any>::new(format!(
        "SELECT {NODE_SELECT_FIELDS} FROM nodes WHERE id IN ("
    ));
    {
        let mut separated = query.separated(", ");
        for id in ids {
            separated.push_bind(*id);
        }
    }
    query.push(") ORDER BY id ASC");
    query.build_query_as().fetch_all(pool).await
}

pub async fn list_by_group_ids(
    pool: &DbPool,
    group_ids: &[i64],
) -> Result<Vec<NodeRecord>, sqlx::Error> {
    if group_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut query = QueryBuilder::<Any>::new(format!(
        "SELECT {NODE_SELECT_FIELDS} FROM nodes WHERE group_id IN ("
    ));
    {
        let mut separated = query.separated(", ");
        for group_id in group_ids {
            separated.push_bind(*group_id);
        }
    }
    query.push(") ORDER BY id ASC");

    query.build_query_as::<NodeRecord>().fetch_all(pool).await
}

pub async fn find_by_id(pool: &DbPool, id: i64) -> Result<Option<NodeRecord>, sqlx::Error> {
    sqlx::query_as::<_, NodeRecord>(
        r#"
        SELECT
               id, name, protocol, raw_link, server, port, enabled + 0 AS enabled, group_id, source_type, source_ref,
               upstream_missing + 0 AS upstream_missing, fingerprint, settings_json, remark, last_latency_ms, last_latency_status,
               last_latency_message, last_latency_tested_at, created_at, updated_at
        FROM nodes
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_fingerprint_in_group(
    pool: &DbPool,
    fingerprint: &str,
    group_id: Option<i64>,
) -> Result<Option<NodeRecord>, sqlx::Error> {
    let query = format!(
        "SELECT {NODE_SELECT_FIELDS} FROM nodes WHERE fingerprint = ? AND fingerprint_scope = ?"
    );

    sqlx::query_as::<_, NodeRecord>(&query)
        .bind(fingerprint)
        .bind(fingerprint_scope(group_id))
        .fetch_optional(pool)
        .await
}

pub async fn find_upstream_by_fingerprint(
    pool: &DbPool,
    url: &str,
    fingerprint: &str,
) -> Result<Option<NodeRecord>, sqlx::Error> {
    let query = format!(
        r#"
        SELECT {NODE_SELECT_FIELDS}
        FROM nodes
        WHERE source_type = 'upstream_subscription'
          AND source_ref = ?
          AND fingerprint = ?
        "#
    );

    sqlx::query_as::<_, NodeRecord>(&query)
        .bind(url)
        .bind(fingerprint)
        .fetch_optional(pool)
        .await
}

pub async fn find_unique_upstream_by_name(
    pool: &DbPool,
    url: &str,
    name: &str,
) -> Result<Option<NodeRecord>, sqlx::Error> {
    let query = format!(
        r#"
        SELECT {NODE_SELECT_FIELDS}
        FROM nodes
        WHERE source_type = 'upstream_subscription'
          AND source_ref = ?
          AND name = ?
        ORDER BY id ASC
        LIMIT 2
        "#
    );

    let records = sqlx::query_as::<_, NodeRecord>(&query)
        .bind(url)
        .bind(name)
        .fetch_all(pool)
        .await?;

    Ok(if records.len() == 1 {
        records.into_iter().next()
    } else {
        None
    })
}

pub async fn existing_fingerprints_in_group(
    pool: &DbPool,
    fingerprints: &[String],
    group_id: Option<i64>,
) -> Result<HashSet<String>, sqlx::Error> {
    if fingerprints.is_empty() {
        return Ok(HashSet::new());
    }

    let mut query =
        QueryBuilder::<Any>::new("SELECT fingerprint FROM nodes WHERE fingerprint_scope = ");
    query.push_bind(fingerprint_scope(group_id));
    query.push(" AND fingerprint IN (");
    {
        let mut separated = query.separated(", ");
        for fingerprint in fingerprints {
            separated.push_bind(fingerprint);
        }
    }
    query.push(")");

    let rows = query.build_query_scalar::<String>().fetch_all(pool).await?;
    Ok(rows.into_iter().collect())
}

pub async fn insert(pool: &DbPool, node: &NewNodeRecord<'_>) -> Result<NodeRecord, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO nodes (
            name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
            upstream_missing, fingerprint, fingerprint_scope, settings_json, remark, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(node.name)
    .bind(node.protocol)
    .bind(node.raw_link)
    .bind(node.server)
    .bind(node.port)
    .bind(node.enabled)
    .bind(node.group_id)
    .bind(node.source_type)
    .bind(node.source_ref)
    .bind(node.upstream_missing)
    .bind(node.fingerprint)
    .bind(node.fingerprint_scope)
    .bind(node.settings_json)
    .bind(node.remark)
    .bind(node.created_at)
    .bind(node.updated_at)
    .execute(pool)
    .await?;

    find_by_fingerprint_in_group(pool, node.fingerprint, node.group_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &DbPool,
    id: i64,
    node: &UpdateNodeRecord<'_>,
) -> Result<NodeRecord, sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE nodes
        SET name = ?,
            protocol = ?,
            raw_link = ?,
            server = ?,
            port = ?,
            enabled = ?,
            group_id = ?,
            upstream_missing = ?,
            fingerprint = ?,
            fingerprint_scope = ?,
            settings_json = ?,
            remark = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(node.name)
    .bind(node.protocol)
    .bind(node.raw_link)
    .bind(node.server)
    .bind(node.port)
    .bind(node.enabled)
    .bind(node.group_id)
    .bind(node.upstream_missing)
    .bind(node.fingerprint)
    .bind(node.fingerprint_scope)
    .bind(node.settings_json)
    .bind(node.remark)
    .bind(node.updated_at)
    .bind(id)
    .execute(pool)
    .await?;

    find_by_id(pool, id).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn delete(pool: &DbPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM nodes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn count_subscriptions_using_node(pool: &DbPool, id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM (
            SELECT subscription_id
            FROM subscription_nodes
            WHERE node_id = ?
            UNION
            SELECT subscription_node_groups.subscription_id
            FROM subscription_node_groups
            INNER JOIN nodes ON nodes.group_id = subscription_node_groups.node_group_id
            WHERE nodes.id = ?
        ) AS subscription_refs
        "#,
    )
    .bind(id)
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn count_subscriptions_using_upstream_source_ref(
    pool: &DbPool,
    url: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM (
            SELECT subscription_nodes.subscription_id
            FROM subscription_nodes
            INNER JOIN nodes ON nodes.id = subscription_nodes.node_id
            WHERE nodes.source_type = 'upstream_subscription'
              AND nodes.source_ref = ?
            UNION
            SELECT subscription_node_groups.subscription_id
            FROM subscription_node_groups
            WHERE EXISTS (
                SELECT 1
                FROM nodes
                WHERE nodes.group_id = subscription_node_groups.node_group_id
                  AND nodes.source_type = 'upstream_subscription'
                  AND nodes.source_ref = ?
            )
        ) AS subscription_refs
        "#,
    )
    .bind(url)
    .bind(url)
    .fetch_one(pool)
    .await
}

pub async fn detach_upstream_source_ref(
    pool: &DbPool,
    url: &str,
    updated_at: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE nodes
        SET source_type = 'manual',
            source_ref = NULL,
            updated_at = ?
        WHERE source_type = 'upstream_subscription'
          AND source_ref = ?
        "#,
    )
    .bind(updated_at)
    .bind(url)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn delete_upstream_source_ref(pool: &DbPool, url: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM nodes
        WHERE source_type = 'upstream_subscription'
          AND source_ref = ?
        "#,
    )
    .bind(url)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn disable_stale_upstream_nodes(
    pool: &DbPool,
    url: &str,
    fingerprints: &[String],
    updated_at: &str,
) -> Result<u64, sqlx::Error> {
    let mut query = QueryBuilder::<Any>::new(
        "UPDATE nodes SET enabled = 0, upstream_missing = 1, updated_at = ",
    );
    query.push_bind(updated_at);
    query.push(" WHERE source_type = 'upstream_subscription' AND source_ref = ");
    query.push_bind(url);
    query.push(" AND (enabled <> 0 OR upstream_missing = 0)");
    if !fingerprints.is_empty() {
        query.push(" AND fingerprint NOT IN (");
        {
            let mut separated = query.separated(", ");
            for fingerprint in fingerprints {
                separated.push_bind(fingerprint);
            }
        }
        query.push(")");
    }

    let result = query.build().execute(pool).await?;
    Ok(result.rows_affected())
}

pub async fn list_enabled(pool: &DbPool) -> Result<Vec<NodeRecord>, sqlx::Error> {
    let query = format!(
        r#"
        SELECT {NODE_SELECT_FIELDS}
        FROM nodes
        WHERE enabled = 1
        ORDER BY id DESC
        "#
    );
    sqlx::query_as::<_, NodeRecord>(&query)
        .fetch_all(pool)
        .await
}

pub async fn update_latency(
    pool: &DbPool,
    id: i64,
    latency_ms: Option<i64>,
    status: &str,
    message: Option<&str>,
    tested_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE nodes
        SET last_latency_ms = ?,
            last_latency_status = ?,
            last_latency_message = ?,
            last_latency_tested_at = ?
        WHERE id = ?
        "#,
    )
    .bind(latency_ms)
    .bind(status)
    .bind(message)
    .bind(tested_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_group_for_ids(
    pool: &DbPool,
    ids: &[i64],
    group_id: Option<i64>,
    updated_at: &str,
) -> Result<Vec<NodeRecord>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut update = QueryBuilder::<Any>::new("UPDATE nodes SET group_id = ");
    update.push_bind(group_id);
    update.push(", updated_at = ");
    update.push_bind(updated_at);
    update.push(", fingerprint_scope = ");
    update.push_bind(fingerprint_scope(group_id));
    update.push(" WHERE id IN (");
    {
        let mut separated = update.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
    }
    update.push(")");
    update.build().execute(pool).await?;

    let mut select = QueryBuilder::<Any>::new("SELECT ");
    select.push(NODE_SELECT_FIELDS);
    select.push(" FROM nodes WHERE id IN (");
    {
        let mut separated = select.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
    }
    select.push(") ORDER BY id DESC");
    select.build_query_as::<NodeRecord>().fetch_all(pool).await
}

pub fn fingerprint_scope(group_id: Option<i64>) -> i64 {
    group_id.unwrap_or(0)
}
