use crate::db::{DbPool, DbQueryBuilder, query, query_as, query_scalar};

use crate::domain::subscription::{
    SubscriptionNodeGroupRecord, SubscriptionNodeRecord, SubscriptionRecord,
};

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

pub async fn count_filtered(
    pool: &DbPool,
    group_id: Option<i64>,
    ungrouped: bool,
) -> Result<i64, sqlx::Error> {
    let mut query = DbQueryBuilder::new("SELECT COUNT(*) FROM subscriptions WHERE 1 = 1");
    push_group_filter(&mut query, group_id, ungrouped);
    query.build_query_scalar().fetch_one(pool).await
}

pub async fn list_page(
    pool: &DbPool,
    group_id: Option<i64>,
    ungrouped: bool,
    limit: i64,
    offset: i64,
) -> Result<Vec<SubscriptionRecord>, sqlx::Error> {
    let mut query = DbQueryBuilder::new(
        "SELECT id, name, token, description, default_client, template_id, group_id, enabled + 0 AS enabled, expires_at, created_at, updated_at FROM subscriptions WHERE 1 = 1",
    );
    push_group_filter(&mut query, group_id, ungrouped);
    query.push(" ORDER BY id DESC LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);
    query.build_query_as().fetch_all(pool).await
}

fn push_group_filter(query: &mut DbQueryBuilder<'_>, group_id: Option<i64>, ungrouped: bool) {
    if ungrouped {
        query.push(" AND group_id IS NULL");
    } else if let Some(group_id) = group_id {
        query.push(" AND group_id = ");
        query.push_bind(group_id);
    }
}

pub async fn find_by_id(pool: &DbPool, id: i64) -> Result<Option<SubscriptionRecord>, sqlx::Error> {
    query_as::<SubscriptionRecord>(
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
    query_as::<SubscriptionRecord>(
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
    query_as::<SubscriptionRecord>(
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

pub async fn insert_with_nodes(
    pool: &DbPool,
    item: &NewSubscriptionRecord<'_>,
    node_ids: &[i64],
    node_group_ids: &[i64],
) -> Result<SubscriptionRecord, sqlx::Error> {
    let mut tx = pool.begin().await?;

    query(
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
    .execute(&mut *tx)
    .await?;

    let record = query_as::<SubscriptionRecord>(
        r#"
        SELECT id, name, token, description, default_client, template_id, group_id, enabled + 0 AS enabled, expires_at, created_at, updated_at
        FROM subscriptions
        WHERE token = ?
        "#,
    )
    .bind(item.token)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(sqlx::Error::RowNotFound)?;

    replace_subscription_nodes_in_tx(&mut tx, record.id, node_ids).await?;
    replace_subscription_node_groups_in_tx(&mut tx, record.id, node_group_ids).await?;
    tx.commit().await?;

    Ok(record)
}

pub async fn update(
    pool: &DbPool,
    id: i64,
    item: &UpdateSubscriptionRecord<'_>,
) -> Result<SubscriptionRecord, sqlx::Error> {
    query(
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

pub async fn update_with_nodes(
    pool: &DbPool,
    id: i64,
    item: &UpdateSubscriptionRecord<'_>,
    node_ids: &[i64],
    node_group_ids: &[i64],
) -> Result<SubscriptionRecord, sqlx::Error> {
    let mut tx = pool.begin().await?;

    query(
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
    .execute(&mut *tx)
    .await?;

    replace_subscription_nodes_in_tx(&mut tx, id, node_ids).await?;
    replace_subscription_node_groups_in_tx(&mut tx, id, node_group_ids).await?;

    let record = query_as::<SubscriptionRecord>(
        r#"
        SELECT id, name, token, description, default_client, template_id, group_id, enabled + 0 AS enabled, expires_at, created_at, updated_at
        FROM subscriptions
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(sqlx::Error::RowNotFound)?;

    tx.commit().await?;
    Ok(record)
}

pub async fn delete(pool: &DbPool, id: i64) -> Result<(), sqlx::Error> {
    query("DELETE FROM subscriptions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_subscription_nodes(
    pool: &DbPool,
    subscription_id: i64,
) -> Result<Vec<SubscriptionNodeRecord>, sqlx::Error> {
    query_as::<SubscriptionNodeRecord>(
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

pub async fn list_subscription_node_groups(
    pool: &DbPool,
    subscription_id: i64,
) -> Result<Vec<SubscriptionNodeGroupRecord>, sqlx::Error> {
    query_as::<SubscriptionNodeGroupRecord>(
        r#"
        SELECT subscription_id, node_group_id, sort_order
        FROM subscription_node_groups
        WHERE subscription_id = ?
        ORDER BY sort_order ASC, node_group_id ASC
        "#,
    )
    .bind(subscription_id)
    .fetch_all(pool)
    .await
}

pub async fn list_subscription_nodes_batch(
    pool: &DbPool,
    subscription_ids: &[i64],
) -> Result<Vec<SubscriptionNodeRecord>, sqlx::Error> {
    if subscription_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = DbQueryBuilder::new(
        "SELECT subscription_id, node_id, sort_order FROM subscription_nodes WHERE subscription_id IN (",
    );
    push_ids(&mut query, subscription_ids);
    query.push(") ORDER BY subscription_id, sort_order, node_id");
    query.build_query_as().fetch_all(pool).await
}

pub async fn list_subscription_node_groups_batch(
    pool: &DbPool,
    subscription_ids: &[i64],
) -> Result<Vec<SubscriptionNodeGroupRecord>, sqlx::Error> {
    if subscription_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = DbQueryBuilder::new(
        "SELECT subscription_id, node_group_id, sort_order FROM subscription_node_groups WHERE subscription_id IN (",
    );
    push_ids(&mut query, subscription_ids);
    query.push(") ORDER BY subscription_id, sort_order, node_group_id");
    query.build_query_as().fetch_all(pool).await
}

fn push_ids(query: &mut DbQueryBuilder<'_>, ids: &[i64]) {
    let mut separated = query.separated(", ");
    for id in ids {
        separated.push_bind(*id);
    }
}

async fn replace_subscription_nodes_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    subscription_id: i64,
    node_ids: &[i64],
) -> Result<(), sqlx::Error> {
    query("DELETE FROM subscription_nodes WHERE subscription_id = ?")
        .bind(subscription_id)
        .execute(&mut **tx)
        .await?;

    for (sort_order, node_id) in node_ids.iter().enumerate() {
        query(
            r#"
            INSERT INTO subscription_nodes (subscription_id, node_id, sort_order)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(subscription_id)
        .bind(*node_id)
        .bind(sort_order as i64)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn replace_subscription_node_groups_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    subscription_id: i64,
    node_group_ids: &[i64],
) -> Result<(), sqlx::Error> {
    query("DELETE FROM subscription_node_groups WHERE subscription_id = ?")
        .bind(subscription_id)
        .execute(&mut **tx)
        .await?;

    for (sort_order, node_group_id) in node_group_ids.iter().enumerate() {
        query(
            r#"
            INSERT INTO subscription_node_groups (subscription_id, node_group_id, sort_order)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(subscription_id)
        .bind(*node_group_id)
        .bind(sort_order as i64)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub async fn count_by_template_id(pool: &DbPool, template_id: i64) -> Result<i64, sqlx::Error> {
    query_scalar::<i64>(
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[tokio::test]
    async fn persists_followed_node_groups_and_resolves_their_nodes() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let database_relative_path = format!(
            "backend/target/test-data/sublinkx-subscription-groups-{}-{nonce}.db",
            std::process::id()
        );
        let database_path = std::env::current_dir()
            .expect("current directory should exist")
            .join(&database_relative_path);
        let database_url = format!("sqlite://{database_relative_path}");
        let pool = crate::db::new_database_pool(&database_url)
            .await
            .expect("test database should initialize");
        let now = "2026-08-20T00:00:00Z";

        sqlx::query(
            "INSERT INTO node_groups (name, sort_order, created_at, updated_at) VALUES (?, 0, ?, ?)",
        )
        .bind(format!("upstream-{nonce}"))
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("node group should insert");
        let node_group_id =
            sqlx::query_scalar::<_, i64>("SELECT id FROM node_groups WHERE name = ?")
                .bind(format!("upstream-{nonce}"))
                .fetch_one(&pool)
                .await
                .expect("node group should exist");

        sqlx::query(
            r#"
            INSERT INTO nodes (
                name, protocol, raw_link, server, port, enabled, group_id, source_type,
                source_ref, upstream_missing, fingerprint, fingerprint_scope, settings_json,
                remark, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, 1, ?, 'upstream_subscription', ?, 0, ?, ?, '{}', '', ?, ?)
            "#,
        )
        .bind("group-node")
        .bind("trojan")
        .bind("trojan://test@example.com:443")
        .bind("example.com")
        .bind(443_i64)
        .bind(node_group_id)
        .bind("https://example.com/sub")
        .bind(format!("fingerprint-{nonce}"))
        .bind(node_group_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("node should insert");

        let subscription_name = format!("subscription-{nonce}");
        let subscription_token = format!("token-{nonce}");
        let record = insert_with_nodes(
            &pool,
            &NewSubscriptionRecord {
                name: &subscription_name,
                token: &subscription_token,
                description: "",
                default_client: Some("mihomo"),
                template_id: None,
                group_id: None,
                enabled: 1,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            &[],
            &[node_group_id],
        )
        .await
        .expect("subscription should insert");

        let followed_groups = list_subscription_node_groups(&pool, record.id)
            .await
            .expect("followed groups should load");
        assert_eq!(followed_groups.len(), 1);
        assert_eq!(followed_groups[0].node_group_id, node_group_id);

        let page = list_page(&pool, None, false, 10, 0)
            .await
            .expect("subscription page should load");
        assert_eq!(page.len(), 1);
        assert_eq!(count_filtered(&pool, None, false).await.unwrap(), 1);
        assert_eq!(
            list_subscription_node_groups_batch(&pool, &[record.id])
                .await
                .expect("followed groups should batch load")
                .len(),
            1
        );

        let group_nodes = crate::repository::node_repo::list_by_group_ids(&pool, &[node_group_id])
            .await
            .expect("group nodes should resolve");
        assert_eq!(group_nodes.len(), 1);
        assert_eq!(group_nodes[0].name, "group-node");
        assert_eq!(
            crate::repository::node_repo::count_subscriptions_using_node(&pool, group_nodes[0].id)
                .await
                .expect("group-followed node references should count"),
            1
        );
        assert_eq!(
            crate::repository::node_repo::count_subscriptions_using_upstream_source_ref(
                &pool,
                "https://example.com/sub"
            )
            .await
            .expect("group-followed upstream references should count"),
            1
        );

        pool.close().await;
        let _ = std::fs::remove_file(database_path);
    }
}
