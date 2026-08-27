use crate::{db::DbPool, domain::upstream_subscription::UpstreamSubscriptionRecord};

const SELECT_FIELDS: &str = r#"
id, name, url, group_id, enabled + 0 AS enabled, remark, last_imported_at,
sync_enabled + 0 AS sync_enabled, sync_interval_minutes, last_import_status,
last_import_message, last_import_imported, last_import_updated, last_import_disabled,
last_import_skipped, last_import_failed, template_id, template_name, created_at, updated_at
"#;

pub struct NewUpstreamSubscriptionRecord<'a> {
    pub name: &'a str,
    pub url: &'a str,
    pub group_id: Option<i64>,
    pub enabled: i64,
    pub sync_enabled: i64,
    pub sync_interval_minutes: i64,
    pub remark: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

pub struct UpdateUpstreamSubscriptionRecord<'a> {
    pub name: &'a str,
    pub url: &'a str,
    pub group_id: Option<i64>,
    pub enabled: i64,
    pub sync_enabled: i64,
    pub sync_interval_minutes: i64,
    pub remark: &'a str,
    pub updated_at: &'a str,
}

pub struct ImportResultRecord<'a> {
    pub status: &'a str,
    pub message: &'a str,
    pub imported: i64,
    pub updated: i64,
    pub disabled: i64,
    pub skipped: i64,
    pub failed: i64,
    pub template_id: Option<i64>,
    pub template_name: Option<&'a str>,
    pub imported_at: &'a str,
}

pub struct ExistingNodeSourceRef {
    pub url: String,
    pub group_id: Option<i64>,
    pub node_count: i64,
}

pub async fn count(pool: &DbPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM upstream_subscriptions")
        .fetch_one(pool)
        .await
}

pub async fn list_page(
    pool: &DbPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<UpstreamSubscriptionRecord>, sqlx::Error> {
    let query = format!(
        "SELECT {SELECT_FIELDS} FROM upstream_subscriptions ORDER BY id DESC LIMIT ? OFFSET ?"
    );
    sqlx::query_as(&query)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
}

pub async fn find_by_id(
    pool: &DbPool,
    id: i64,
) -> Result<Option<UpstreamSubscriptionRecord>, sqlx::Error> {
    let query = format!(
        r#"
        SELECT {SELECT_FIELDS}
        FROM upstream_subscriptions
        WHERE id = ?
        "#
    );
    sqlx::query_as::<_, UpstreamSubscriptionRecord>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn find_by_url(
    pool: &DbPool,
    url: &str,
) -> Result<Option<UpstreamSubscriptionRecord>, sqlx::Error> {
    let query = format!(
        r#"
        SELECT {SELECT_FIELDS}
        FROM upstream_subscriptions
        WHERE url = ?
        "#
    );
    sqlx::query_as::<_, UpstreamSubscriptionRecord>(&query)
        .bind(url)
        .fetch_optional(pool)
        .await
}

pub async fn insert(
    pool: &DbPool,
    item: &NewUpstreamSubscriptionRecord<'_>,
) -> Result<UpstreamSubscriptionRecord, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO upstream_subscriptions (
            name, url, group_id, enabled, sync_enabled, sync_interval_minutes, remark, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(item.name)
    .bind(item.url)
    .bind(item.group_id)
    .bind(item.enabled)
    .bind(item.sync_enabled)
    .bind(item.sync_interval_minutes)
    .bind(item.remark)
    .bind(item.created_at)
    .bind(item.updated_at)
    .execute(pool)
    .await?;

    find_by_url(pool, item.url)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &DbPool,
    id: i64,
    item: &UpdateUpstreamSubscriptionRecord<'_>,
) -> Result<UpstreamSubscriptionRecord, sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE upstream_subscriptions
        SET name = ?,
            url = ?,
            group_id = ?,
            enabled = ?,
            sync_enabled = ?,
            sync_interval_minutes = ?,
            remark = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(item.name)
    .bind(item.url)
    .bind(item.group_id)
    .bind(item.enabled)
    .bind(item.sync_enabled)
    .bind(item.sync_interval_minutes)
    .bind(item.remark)
    .bind(item.updated_at)
    .bind(id)
    .execute(pool)
    .await?;

    find_by_id(pool, id).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn delete(pool: &DbPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM upstream_subscriptions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_import_result(
    pool: &DbPool,
    id: i64,
    result: &ImportResultRecord<'_>,
) -> Result<UpstreamSubscriptionRecord, sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE upstream_subscriptions
        SET last_imported_at = ?,
            last_import_status = ?,
            last_import_message = ?,
            last_import_imported = ?,
            last_import_updated = ?,
            last_import_disabled = ?,
            last_import_skipped = ?,
            last_import_failed = ?,
            template_id = ?,
            template_name = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(result.imported_at)
    .bind(result.status)
    .bind(result.message)
    .bind(result.imported)
    .bind(result.updated)
    .bind(result.disabled)
    .bind(result.skipped)
    .bind(result.failed)
    .bind(result.template_id)
    .bind(result.template_name)
    .bind(result.imported_at)
    .bind(id)
    .execute(pool)
    .await?;

    find_by_id(pool, id).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn upsert_import_result_by_url(
    pool: &DbPool,
    name: &str,
    url: &str,
    group_id: Option<i64>,
    remark: &str,
    result: &ImportResultRecord<'_>,
) -> Result<UpstreamSubscriptionRecord, sqlx::Error> {
    if let Some(existing) = find_by_url(pool, url).await? {
        sqlx::query(
            r#"
            UPDATE upstream_subscriptions
            SET group_id = ?,
                remark = ?,
                last_imported_at = ?,
                last_import_status = ?,
                last_import_message = ?,
                last_import_imported = ?,
                last_import_updated = ?,
                last_import_disabled = ?,
                last_import_skipped = ?,
                last_import_failed = ?,
                template_id = ?,
                template_name = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(group_id)
        .bind(remark)
        .bind(result.imported_at)
        .bind(result.status)
        .bind(result.message)
        .bind(result.imported)
        .bind(result.updated)
        .bind(result.disabled)
        .bind(result.skipped)
        .bind(result.failed)
        .bind(result.template_id)
        .bind(result.template_name)
        .bind(result.imported_at)
        .bind(existing.id)
        .execute(pool)
        .await?;

        return find_by_id(pool, existing.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound);
    }

    sqlx::query(
        r#"
        INSERT INTO upstream_subscriptions (
            name, url, group_id, enabled, remark, last_imported_at, last_import_status,
            last_import_message, last_import_imported, last_import_updated, last_import_disabled,
            last_import_skipped, last_import_failed,
            template_id, template_name, created_at, updated_at
        ) VALUES (?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(name)
    .bind(url)
    .bind(group_id)
    .bind(remark)
    .bind(result.imported_at)
    .bind(result.status)
    .bind(result.message)
    .bind(result.imported)
    .bind(result.updated)
    .bind(result.disabled)
    .bind(result.skipped)
    .bind(result.failed)
    .bind(result.template_id)
    .bind(result.template_name)
    .bind(result.imported_at)
    .bind(result.imported_at)
    .execute(pool)
    .await?;

    find_by_url(pool, url)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn list_existing_node_source_refs(
    pool: &DbPool,
) -> Result<Vec<ExistingNodeSourceRef>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, Option<i64>, i64)>(
        r#"
        SELECT source_ref, MIN(group_id), COUNT(*)
        FROM nodes
        WHERE source_type = 'upstream_subscription'
          AND source_ref IS NOT NULL
          AND source_ref <> ''
        GROUP BY source_ref
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(url, group_id, node_count)| ExistingNodeSourceRef {
            url,
            group_id,
            node_count,
        })
        .collect())
}

pub async fn list_sync_enabled(
    pool: &DbPool,
) -> Result<Vec<UpstreamSubscriptionRecord>, sqlx::Error> {
    let query = format!(
        r#"
        SELECT {SELECT_FIELDS}
        FROM upstream_subscriptions
        WHERE enabled = 1
          AND sync_enabled = 1
        ORDER BY id ASC
        "#
    );
    sqlx::query_as::<_, UpstreamSubscriptionRecord>(&query)
        .fetch_all(pool)
        .await
}
