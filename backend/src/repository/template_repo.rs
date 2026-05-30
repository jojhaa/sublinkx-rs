use sqlx::SqlitePool;

use crate::domain::template::TemplateRecord;

pub struct NewTemplateRecord<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub content: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

pub struct UpdateTemplateRecord<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub content: &'a str,
    pub updated_at: &'a str,
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<TemplateRecord>, sqlx::Error> {
    sqlx::query_as::<_, TemplateRecord>(
        r#"
        SELECT id, name, kind, content, created_at, updated_at
        FROM templates
        ORDER BY id DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Option<TemplateRecord>, sqlx::Error> {
    sqlx::query_as::<_, TemplateRecord>(
        r#"
        SELECT id, name, kind, content, created_at, updated_at
        FROM templates
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_name(
    pool: &SqlitePool,
    name: &str,
) -> Result<Option<TemplateRecord>, sqlx::Error> {
    sqlx::query_as::<_, TemplateRecord>(
        r#"
        SELECT id, name, kind, content, created_at, updated_at
        FROM templates
        WHERE name = ?1
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub async fn insert(
    pool: &SqlitePool,
    item: &NewTemplateRecord<'_>,
) -> Result<TemplateRecord, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO templates (name, kind, content, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(item.name)
    .bind(item.kind)
    .bind(item.content)
    .bind(item.created_at)
    .bind(item.updated_at)
    .execute(pool)
    .await?;

    find_by_id(pool, result.last_insert_rowid())
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &SqlitePool,
    id: i64,
    item: &UpdateTemplateRecord<'_>,
) -> Result<TemplateRecord, sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE templates
        SET name = ?1,
            kind = ?2,
            content = ?3,
            updated_at = ?4
        WHERE id = ?5
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

pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM templates WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
