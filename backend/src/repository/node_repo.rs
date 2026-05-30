use sqlx::SqlitePool;

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

pub async fn list(pool: &SqlitePool) -> Result<Vec<NodeRecord>, sqlx::Error> {
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

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Option<NodeRecord>, sqlx::Error> {
    sqlx::query_as::<_, NodeRecord>(
        r#"
        SELECT
               id, name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
               fingerprint, settings_json, remark, last_latency_ms, last_latency_status,
               last_latency_message, last_latency_tested_at, created_at, updated_at
        FROM nodes
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_fingerprint(
    pool: &SqlitePool,
    fingerprint: &str,
) -> Result<Option<NodeRecord>, sqlx::Error> {
    sqlx::query_as::<_, NodeRecord>(
        r#"
        SELECT
               id, name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
               fingerprint, settings_json, remark, last_latency_ms, last_latency_status,
               last_latency_message, last_latency_tested_at, created_at, updated_at
        FROM nodes
        WHERE fingerprint = ?1
        "#,
    )
    .bind(fingerprint)
    .fetch_optional(pool)
    .await
}

pub async fn insert(
    pool: &SqlitePool,
    node: &NewNodeRecord<'_>,
) -> Result<NodeRecord, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO nodes (
            name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref,
            fingerprint, settings_json, remark, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
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

    find_by_id(pool, result.last_insert_rowid())
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &SqlitePool,
    id: i64,
    node: &UpdateNodeRecord<'_>,
) -> Result<NodeRecord, sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE nodes
        SET name = ?1,
            protocol = ?2,
            raw_link = ?3,
            server = ?4,
            port = ?5,
            enabled = ?6,
            group_id = ?7,
            fingerprint = ?8,
            settings_json = ?9,
            remark = ?10,
            updated_at = ?11
        WHERE id = ?12
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

pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM nodes WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_enabled(pool: &SqlitePool) -> Result<Vec<NodeRecord>, sqlx::Error> {
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
    pool: &SqlitePool,
    id: i64,
    latency_ms: Option<i64>,
    status: &str,
    message: Option<&str>,
    tested_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE nodes
        SET last_latency_ms = ?1,
            last_latency_status = ?2,
            last_latency_message = ?3,
            last_latency_tested_at = ?4
        WHERE id = ?5
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
