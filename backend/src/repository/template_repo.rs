use crate::db::{DbPool, DbQueryBuilder, query, query_as};

use crate::domain::template::TemplateRecord;

pub struct NewTemplateRecord<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub content: &'a str,
    pub is_builtin: i64,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

pub struct UpdateTemplateRecord<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub content: &'a str,
    pub updated_at: &'a str,
}

pub struct UpsertUpstreamTemplateResult {
    pub template: TemplateRecord,
    pub removed_templates: usize,
}

pub async fn count_filtered(pool: &DbPool, kind: Option<&str>) -> Result<i64, sqlx::Error> {
    let mut query = DbQueryBuilder::new("SELECT COUNT(*) FROM templates WHERE 1 = 1");
    if let Some(kind) = kind.filter(|value| !value.is_empty()) {
        query.push(" AND kind = ");
        query.push_bind(kind);
    }
    query.build_query_scalar().fetch_one(pool).await
}

pub async fn count_by_kind(pool: &DbPool) -> Result<Vec<(String, i64)>, sqlx::Error> {
    sqlx::query_as("SELECT kind, COUNT(*) FROM templates GROUP BY kind ORDER BY kind")
        .fetch_all(pool)
        .await
}

pub async fn list_page(
    pool: &DbPool,
    kind: Option<&str>,
    compact: bool,
    limit: i64,
    offset: i64,
) -> Result<Vec<TemplateRecord>, sqlx::Error> {
    let content = if compact {
        "CASE WHEN content LIKE '%x-sublinkx-upstream-template: true%' THEN 'x-sublinkx-upstream-template: true' ELSE substr(content, 1, 320) END AS content"
    } else {
        "content"
    };
    let mut query = DbQueryBuilder::new(format!(
        "SELECT id, name, kind, {content}, is_builtin + 0 AS is_builtin, created_at, updated_at FROM templates WHERE 1 = 1"
    ));
    if let Some(kind) = kind.filter(|value| !value.is_empty()) {
        query.push(" AND kind = ");
        query.push_bind(kind);
    }
    query.push(" ORDER BY id DESC LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);
    query.build_query_as().fetch_all(pool).await
}

pub async fn find_by_id(pool: &DbPool, id: i64) -> Result<Option<TemplateRecord>, sqlx::Error> {
    query_as::<TemplateRecord>(
        r#"
        SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
        FROM templates
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
) -> Result<Option<TemplateRecord>, sqlx::Error> {
    query_as::<TemplateRecord>(
        r#"
        SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
        FROM templates
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub async fn find_latest_builtin_by_kind(
    pool: &DbPool,
    kind: &str,
) -> Result<Option<TemplateRecord>, sqlx::Error> {
    query_as::<TemplateRecord>(
        r#"
        SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
        FROM templates
        WHERE kind = ? AND is_builtin = 1
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(kind)
    .fetch_optional(pool)
    .await
}

pub async fn list_upstream_passthrough(pool: &DbPool) -> Result<Vec<TemplateRecord>, sqlx::Error> {
    query_as::<TemplateRecord>(
        r#"
        SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
        FROM templates
        WHERE kind = 'mihomo'
          AND is_builtin = 0
          AND content LIKE '%x-sublinkx-upstream-template: true%'
        ORDER BY id DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn upsert_latest_upstream_passthrough(
    pool: &DbPool,
    source_digest: &str,
    item: &NewTemplateRecord<'_>,
) -> Result<UpsertUpstreamTemplateResult, sqlx::Error> {
    let exact_suffix = format!("% {source_digest}");
    let revision_suffix = format!("% {source_digest} #%");
    let mut tx = pool.begin().await?;
    let candidates = query_as::<TemplateRecord>(
        r#"
        SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
        FROM templates
        WHERE kind = 'mihomo'
          AND is_builtin = 0
          AND content LIKE '%x-sublinkx-upstream-template: true%'
          AND (name LIKE ? OR name LIKE ?)
        ORDER BY id DESC
        "#,
    )
    .bind(&exact_suffix)
    .bind(&revision_suffix)
    .fetch_all(&mut *tx)
    .await?;

    let occupied_base = query_as::<TemplateRecord>(
        r#"
        SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
        FROM templates
        WHERE name = ?
        "#,
    )
    .bind(item.name)
    .fetch_optional(&mut *tx)
    .await?;
    let base_is_available = occupied_base.as_ref().is_none_or(|occupied| {
        candidates
            .iter()
            .any(|candidate| candidate.id == occupied.id)
    });
    let target_name = if base_is_available {
        item.name.to_string()
    } else if let Some(latest) = candidates.first() {
        latest.name.clone()
    } else {
        let mut available_name = None;
        for revision in 2..1000 {
            let candidate_name = format!("{} #{revision}", item.name);
            let exists = query_as::<TemplateRecord>(
                r#"
                SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
                FROM templates
                WHERE name = ?
                "#,
            )
            .bind(&candidate_name)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
            if !exists {
                available_name = Some(candidate_name);
                break;
            }
        }
        available_name.ok_or(sqlx::Error::RowNotFound)?
    };

    let latest_id = if let Some(latest) = candidates.first() {
        for old in candidates.iter().skip(1) {
            query(
                r#"
                UPDATE subscriptions
                SET template_id = ?, updated_at = ?
                WHERE template_id = ?
                "#,
            )
            .bind(latest.id)
            .bind(item.updated_at)
            .bind(old.id)
            .execute(&mut *tx)
            .await?;

            query(
                r#"
                UPDATE upstream_subscriptions
                SET template_id = ?, template_name = ?, updated_at = ?
                WHERE template_id = ?
                "#,
            )
            .bind(latest.id)
            .bind(&target_name)
            .bind(item.updated_at)
            .bind(old.id)
            .execute(&mut *tx)
            .await?;

            query("DELETE FROM templates WHERE id = ? AND is_builtin = 0")
                .bind(old.id)
                .execute(&mut *tx)
                .await?;
        }

        query(
            r#"
            UPDATE templates
            SET name = ?, kind = ?, content = ?, updated_at = ?
            WHERE id = ? AND is_builtin = 0
            "#,
        )
        .bind(&target_name)
        .bind(item.kind)
        .bind(item.content)
        .bind(item.updated_at)
        .bind(latest.id)
        .execute(&mut *tx)
        .await?;

        query(
            r#"
            UPDATE upstream_subscriptions
            SET template_name = ?, updated_at = ?
            WHERE template_id = ?
            "#,
        )
        .bind(&target_name)
        .bind(item.updated_at)
        .bind(latest.id)
        .execute(&mut *tx)
        .await?;

        latest.id
    } else {
        query(
            r#"
            INSERT INTO templates (name, kind, content, is_builtin, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&target_name)
        .bind(item.kind)
        .bind(item.content)
        .bind(item.is_builtin)
        .bind(item.created_at)
        .bind(item.updated_at)
        .execute(&mut *tx)
        .await?;

        query_as::<TemplateRecord>(
            r#"
            SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
            FROM templates
            WHERE name = ?
            "#,
        )
        .bind(&target_name)
        .fetch_one(&mut *tx)
        .await?
        .id
    };

    let template = query_as::<TemplateRecord>(
        r#"
        SELECT id, name, kind, content, is_builtin + 0 AS is_builtin, created_at, updated_at
        FROM templates
        WHERE id = ?
        "#,
    )
    .bind(latest_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(UpsertUpstreamTemplateResult {
        template,
        removed_templates: candidates.len().saturating_sub(1),
    })
}

pub async fn insert(
    pool: &DbPool,
    item: &NewTemplateRecord<'_>,
) -> Result<TemplateRecord, sqlx::Error> {
    query(
        r#"
        INSERT INTO templates (name, kind, content, is_builtin, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(item.name)
    .bind(item.kind)
    .bind(item.content)
    .bind(item.is_builtin)
    .bind(item.created_at)
    .bind(item.updated_at)
    .execute(pool)
    .await?;

    find_by_name(pool, item.name)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &DbPool,
    id: i64,
    item: &UpdateTemplateRecord<'_>,
) -> Result<TemplateRecord, sqlx::Error> {
    query(
        r#"
        UPDATE templates
        SET name = ?,
            kind = ?,
            content = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(item.name)
    .bind(item.kind)
    .bind(item.content)
    .bind(item.updated_at)
    .bind(id)
    .execute(pool)
    .await?;

    find_by_id(pool, id).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn delete(pool: &DbPool, id: i64) -> Result<(), sqlx::Error> {
    query("DELETE FROM templates WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::db::{query, query_scalar};

    #[tokio::test]
    async fn keeps_latest_upstream_template_and_rebinds_references() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let database_relative_path = format!(
            "target/test-data/sublinkx-upstream-template-{}-{nonce}.db",
            std::process::id()
        );
        let database_path = std::env::current_dir()
            .expect("current directory should exist")
            .join(&database_relative_path);
        let database_url = format!("sqlite://{database_relative_path}");
        let pool = crate::db::new_database_pool(&database_url)
            .await
            .expect("test database should initialize");
        let now = "2026-08-31T00:00:00Z";
        let digest = "0f512f8a";
        let base_name = format!("Upstream Mihomo GAT {digest}");

        let first = insert(
            &pool,
            &NewTemplateRecord {
                name: &base_name,
                kind: "mihomo",
                content: "x-sublinkx-upstream-template: true\nproxies: [old-1]\n",
                is_builtin: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("first upstream template should insert");
        let second_name = format!("{base_name} #2");
        let second = insert(
            &pool,
            &NewTemplateRecord {
                name: &second_name,
                kind: "mihomo",
                content: "x-sublinkx-upstream-template: true\nproxies: [old-2]\n",
                is_builtin: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("second upstream template should insert");
        let unrelated = insert(
            &pool,
            &NewTemplateRecord {
                name: "Upstream Mihomo Other deadbeef",
                kind: "mihomo",
                content: "x-sublinkx-upstream-template: true\nproxies: [other]\n",
                is_builtin: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("unrelated template should insert");

        for (id, template_id) in [(1_i64, first.id), (2_i64, second.id)] {
            query(
                r#"
                INSERT INTO subscriptions (
                    id, name, token, description, template_id, enabled, created_at, updated_at
                ) VALUES (?, ?, ?, '', ?, 1, ?, ?)
                "#,
            )
            .bind(id)
            .bind(format!("subscription-{id}"))
            .bind(format!("token-{id}"))
            .bind(template_id)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .expect("subscription should insert");

            query(
                r#"
                INSERT INTO upstream_subscriptions (
                    id, name, url, enabled, sync_enabled, sync_interval_minutes, remark,
                    template_id, template_name, created_at, updated_at
                ) VALUES (?, ?, ?, 1, 1, 60, '', ?, ?, ?, ?)
                "#,
            )
            .bind(id)
            .bind(format!("upstream-{id}"))
            .bind(format!("https://example.test/{id}"))
            .bind(template_id)
            .bind(if id == 1 { &base_name } else { &second_name })
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .expect("upstream subscription should insert");
        }

        let result = upsert_latest_upstream_passthrough(
            &pool,
            digest,
            &NewTemplateRecord {
                name: &base_name,
                kind: "mihomo",
                content: "x-sublinkx-upstream-template: true\nproxies: [latest]\n",
                is_builtin: 0,
                created_at: now,
                updated_at: "2026-08-31T01:00:00Z",
            },
        )
        .await
        .expect("upstream templates should consolidate");

        assert_eq!(result.template.id, second.id);
        assert_eq!(result.template.name, base_name);
        assert!(result.template.content.contains("[latest]"));
        assert_eq!(result.removed_templates, 1);
        assert!(find_by_id(&pool, first.id).await.unwrap().is_none());
        assert!(find_by_id(&pool, unrelated.id).await.unwrap().is_some());

        let subscription_bindings =
            query_scalar::<i64>("SELECT COUNT(*) FROM subscriptions WHERE template_id = ?")
                .bind(second.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(subscription_bindings, 2);
        let upstream_bindings = query_scalar::<i64>(
            "SELECT COUNT(*) FROM upstream_subscriptions WHERE template_id = ? AND template_name = ?",
        )
        .bind(second.id)
        .bind(&base_name)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(upstream_bindings, 2);

        let long_content = "preview-".repeat(100);
        let preview_template = insert(
            &pool,
            &NewTemplateRecord {
                name: "Large preview template",
                kind: "mihomo",
                content: &long_content,
                is_builtin: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("large template should insert");
        let compact_page = list_page(&pool, None, true, 100, 0)
            .await
            .expect("compact template page should load");
        let compact_preview = compact_page
            .iter()
            .find(|item| item.id == preview_template.id)
            .expect("compact page should include the large template");
        assert_eq!(compact_preview.content.chars().count(), 320);
        assert!(long_content.starts_with(&compact_preview.content));

        let upstream_preview = compact_page
            .iter()
            .find(|item| item.id == unrelated.id)
            .expect("compact page should include the upstream template");
        assert_eq!(
            upstream_preview.content,
            "x-sublinkx-upstream-template: true"
        );

        let full_page = list_page(&pool, None, false, 100, 0)
            .await
            .expect("full template page should load");
        assert_eq!(
            full_page
                .iter()
                .find(|item| item.id == preview_template.id)
                .expect("full page should include the large template")
                .content,
            long_content
        );

        pool.close().await;
        drop(pool);
        for attempt in 0..10 {
            match std::fs::remove_file(&database_path) {
                Ok(()) => break,
                Err(error) if attempt < 9 => {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    if error.kind() == std::io::ErrorKind::NotFound {
                        break;
                    }
                }
                Err(error) => panic!("test database should be removable: {error}"),
            }
        }
    }
}
