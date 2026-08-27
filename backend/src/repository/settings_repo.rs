use std::collections::HashMap;

use sqlx::{Any, QueryBuilder};

use crate::db::{DbKind, DbPool, db_kind};

pub async fn get_many(
    pool: &DbPool,
    keys: &[&str],
) -> Result<HashMap<String, String>, sqlx::Error> {
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    let mut query =
        QueryBuilder::<Any>::new("SELECT `key`, value FROM app_settings WHERE `key` IN (");
    {
        let mut separated = query.separated(", ");
        for key in keys {
            separated.push_bind(*key);
        }
    }
    query.push(")");
    let rows = query
        .build_query_as::<(String, String)>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[tokio::test]
    async fn reads_multiple_settings_in_one_query() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let relative_path = format!(
            "backend/target/test-data/sublinkx-settings-{}-{nonce}.db",
            std::process::id()
        );
        let absolute_path = std::env::current_dir().unwrap().join(&relative_path);
        let pool = crate::db::new_database_pool(&format!("sqlite://{relative_path}"))
            .await
            .expect("test database should initialize");

        set(&pool, "test.alpha", "one", "2026-08-27T00:00:00Z")
            .await
            .unwrap();
        set(&pool, "test.beta", "two", "2026-08-27T00:00:00Z")
            .await
            .unwrap();
        let values = get_many(&pool, &["test.alpha", "test.beta", "test.missing"])
            .await
            .unwrap();

        assert_eq!(values.get("test.alpha").map(String::as_str), Some("one"));
        assert_eq!(values.get("test.beta").map(String::as_str), Some("two"));
        assert!(!values.contains_key("test.missing"));

        pool.close().await;
        let _ = std::fs::remove_file(absolute_path);
    }
}
