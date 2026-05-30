use crate::db::DbPool;

use crate::domain::subscription::{SubscriptionNodeRecord, SubscriptionRecord};

pub struct NewSubscriptionRecord<'a> {
    pub name: &'a str,
    pub token: &'a str,
    pub description: &'a str,
    pub default_client: Option<&'a str>,
    pub template_id: Option<i64>,
    pub group_id: Option<i64>,
    pub enabled: i64,
    pub expires_at: Option<&'a str>,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

pub struct UpdateSubscriptionRecord<'a> {
    pub name: &'a str,
    pub token: &'a str,
    pub description: &'a str,
    pub default_client: Option<&'a str>,
    pub template_id: Option<i64>,
    pub group_id: Option<i64>,
    pub enabled: i64,
    pub expires_at: Option<&'a str>,
    pub updated_at: &'a str,
}

pub async fn list(pool: &DbPool) -> Result<Vec<SubscriptionRecord>, sqlx::Error> {
    sqlx::query_as::<_, SubscriptionRecord>(
        r#"
        SELECT id, name, token, description, default_client, template_id, group_id, enabled + 0 AS enabled, expires_at, created_at, updated_at
        FROM subscriptions
        ORDER BY id DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn find_by_id(pool: &DbPool, id: i64) -> Result<Option<SubscriptionRecord>, sqlx::Error> {
    sqlx::query_as::<_, SubscriptionRecord>(
        r#"
        SELECT id, name, token, description, default_client, template_id, group_id, enabled + 0 AS enabled, expires_at, created_at, updated_at
        FROM subscriptions
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_name(
    pool: &DbPool,
    name: &str,
) -> Result<Option<SubscriptionRecord>, sqlx::Error> {
    sqlx::query_as::<_, SubscriptionRecord>(
        r#"
        SELECT id, name, token, description, default_client, template_id, group_id, enabled + 0 AS enabled, expires_at, created_at, updated_at
        FROM subscriptions
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_token(
    pool: &DbPool,
    token: &str,
) -> Result<Option<SubscriptionRecord>, sqlx::Error> {
    sqlx::query_as::<_, SubscriptionRecord>(
        r#"
        SELECT id, name, token, description, default_client, template_id, group_id, enabled + 0 AS enabled, expires_at, created_at, updated_at
        FROM subscriptions
        WHERE token = ?
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
}

pub async fn insert(
    pool: &DbPool,
    item: &NewSubscriptionRecord<'_>,
) -> Result<SubscriptionRecord, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO subscriptions (
            name, token, description, default_client, template_id, group_id, enabled, expires_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(item.name)
    .bind(item.token)
    .bind(item.description)
    .bind(item.default_client)
    .bind(item.template_id)
    .bind(item.group_id)
    .bind(item.enabled)
    .bind(item.expires_at)
    .bind(item.created_at)
    .bind(item.updated_at)
    .execute(pool)
    .await?;

    find_by_token(pool, item.token)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &DbPool,
    id: i64,
    item: &UpdateSubscriptionRecord<'_>,
) -> Result<SubscriptionRecord, sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE subscriptions
        SET name = ?,
            token = ?,
            description = ?,
            default_client = ?,
            template_id = ?,
            group_id = ?,
            enabled = ?,
            expires_at = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(item.name)
    .bind(item.token)
    .bind(item.description)
    .bind(item.default_client)
    .bind(item.template_id)
    .bind(item.group_id)
    .bind(item.enabled)
    .bind(item.expires_at)
    .bind(item.updated_at)
    .bind(id)
    .execute(pool)
    .await?;

    find_by_id(pool, id).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn delete(pool: &DbPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM subscriptions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_subscription_nodes(
    pool: &DbPool,
    subscription_id: i64,
) -> Result<Vec<SubscriptionNodeRecord>, sqlx::Error> {
    sqlx::query_as::<_, SubscriptionNodeRecord>(
        r#"
        SELECT subscription_id, node_id, sort_order
        FROM subscription_nodes
        WHERE subscription_id = ?
        ORDER BY sort_order ASC, node_id ASC
        "#,
    )
    .bind(subscription_id)
    .fetch_all(pool)
    .await
}

pub async fn replace_subscription_nodes(
    pool: &DbPool,
    subscription_id: i64,
    node_ids: &[i64],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM subscription_nodes WHERE subscription_id = ?")
        .bind(subscription_id)
        .execute(&mut *tx)
        .await?;

    for (sort_order, node_id) in node_ids.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO subscription_nodes (subscription_id, node_id, sort_order)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(subscription_id)
        .bind(*node_id)
        .bind(sort_order as i64)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn count_by_template_id(pool: &DbPool, template_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(1)
        FROM subscriptions
        WHERE template_id = ?
        "#,
    )
    .bind(template_id)
    .fetch_one(pool)
    .await
}
