use crate::db::{DbKind, DbPool, db_kind};

pub async fn get(pool: &DbPool, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT value FROM app_settings WHERE `key` = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
}

pub async fn set(
    pool: &DbPool,
    key: &str,
    value: &str,
    updated_at: &str,
) -> Result<(), sqlx::Error> {
    let sql = match db_kind() {
        DbKind::Sqlite => {
            r#"
            INSERT INTO app_settings (`key`, value, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(`key`) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            "#
        }
        DbKind::MySql => {
            r#"
            INSERT INTO app_settings (`key`, value, updated_at)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE
                value = VALUES(value),
                updated_at = VALUES(updated_at)
            "#
        }
    };

    sqlx::query(sql)
        .bind(key)
        .bind(value)
        .bind(updated_at)
        .execute(pool)
        .await?;
    Ok(())
}
