use sqlx::{Any, QueryBuilder};

use crate::db::DbPool;
use crate::domain::node::NodeRecord;

const NODE_SELECT_FIELDS: &str = r#"
id, name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
fingerprint, settings_json, remark, last_latency_ms, last_latency_status,
last_latency_message, last_latency_tested_at, created_at, updated_at
"#;

pub struct NewNodeRecord<'a> {
    pub name: &'a str,
    pub protocol: &'a str,
    pub raw_link: &'a str,
    pub server: &'a str,
    pub port: i64,
    pub enabled: bool,
    pub group_id: Option<i64>,
    pub source_type: &'a str,
    pub source_ref: Option<&'a str>,
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
    pub enabled: bool,
    pub group_id: Option<i64>,
    pub fingerprint: &'a str,
    pub settings_json: &'a str,
    pub remark: &'a str,
    pub updated_at: &'a str,
}

pub async fn list(pool: &DbPool) -> Result<Vec<NodeRecord>, sqlx::Error> {
    sqlx::query_as::<_, NodeRecord>(
        r#"
        SELECT
               id, name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
               fingerprint, settings_json, remark, last_latency_ms, last_latency_status,
               last_latency_message, last_latency_tested_at, created_at, updated_at
        FROM nodes
        ORDER BY id DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn find_by_id(pool: &DbPool, id: i64) -> Result<Option<NodeRecord>, sqlx::Error> {
    sqlx::query_as::<_, NodeRecord>(
        r#"
        SELECT
               id, name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
               fingerprint, settings_json, remark, last_latency_ms, last_latency_status,
               last_latency_message, last_latency_tested_at, created_at, updated_at
        FROM nodes
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_fingerprint(
    pool: &DbPool,
    fingerprint: &str,
) -> Result<Option<NodeRecord>, sqlx::Error> {
    sqlx::query_as::<_, NodeRecord>(
        r#"
        SELECT
               id, name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
               fingerprint, settings_json, remark, last_latency_ms, last_latency_status,
               last_latency_message, last_latency_tested_at, created_at, updated_at
        FROM nodes
        WHERE fingerprint = ?
        "#,
    )
    .bind(fingerprint)
    .fetch_optional(pool)
    .await
}

pub async fn insert(pool: &DbPool, node: &NewNodeRecord<'_>) -> Result<NodeRecord, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO nodes (
            name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
            fingerprint, settings_json, remark, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
    .bind(node.fingerprint)
    .bind(node.settings_json)
    .bind(node.remark)
    .bind(node.created_at)
    .bind(node.updated_at)
    .execute(pool)
    .await?;

    find_by_fingerprint(pool, node.fingerprint)
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
            fingerprint = ?,
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
    .bind(node.fingerprint)
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
