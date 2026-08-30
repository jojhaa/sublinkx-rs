use sqlx::{Any, QueryBuilder};

use crate::db::DbPool;

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

pub async fn count_filtered(pool: &DbPool, kind: Option<&str>) -> Result<i64, sqlx::Error> {
    let mut query = QueryBuilder::<Any>::new("SELECT COUNT(*) FROM templates WHERE 1 = 1");
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
        "CASE WHEN content LIKE '%x-sublinkx-upstream-template: true%' THEN 'x-sublinkx-upstream-template: true' ELSE '' END AS content"
    } else {
        "content"
    };
    let mut query = QueryBuilder::<Any>::new(format!(
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
    sqlx::query_as::<_, TemplateRecord>(
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
    sqlx::query_as::<_, TemplateRecord>(
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

pub async fn insert(
    pool: &DbPool,
    item: &NewTemplateRecord<'_>,
) -> Result<TemplateRecord, sqlx::Error> {
    sqlx::query(
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
    sqlx::query(
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
    sqlx::query("DELETE FROM templates WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
