use crate::db::DbPool;
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use tracing::warn;

use crate::{config::SecurityConfig, domain::user::User};

pub async fn bootstrap_admin(pool: &DbPool, security: &SecurityConfig) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        if let Some(user) = find_by_username(pool, &security.bootstrap_admin_username).await?
            && user.must_change_credentials != 0
        {
            let password_hash =
                hash_password(&security.bootstrap_admin_password).map_err(|_| {
                    sqlx::Error::Protocol("failed to hash bootstrap admin password".into())
                })?;
            reset_bootstrap_admin_password(pool, user.id, &password_hash).await?;
        }
        return Ok(());
    }

    let password_hash = hash_password(&security.bootstrap_admin_password)
        .map_err(|_| sqlx::Error::Protocol("failed to hash bootstrap admin password".into()))?;
    let now = crate::utils::time::now_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO users (username, password_hash, nickname, role, status, must_change_credentials, created_at, updated_at)
        VALUES (?, ?, ?, 'admin', 'active', 1, ?, ?)
        "#,
    )
    .bind(&security.bootstrap_admin_username)
    .bind(password_hash)
    .bind("Administrator")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    warn!(
        "bootstrap admin created with username '{}'; change the password immediately",
        security.bootstrap_admin_username
    );

    Ok(())
}

pub async fn find_by_username(pool: &DbPool, username: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, nickname, role, status, must_change_credentials, created_at, updated_at
        FROM users
        WHERE username = ?
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_id(pool: &DbPool, id: i64) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, nickname, role, status, must_change_credentials, created_at, updated_at
        FROM users
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn username_exists_for_other_user(
    pool: &DbPool,
    username: &str,
    user_id: i64,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM users
        WHERE username = ? AND id <> ?
        "#,
    )
    .bind(username)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

pub async fn update_credentials(
    pool: &DbPool,
    user_id: i64,
    username: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    let now = crate::utils::time::now_rfc3339();

    sqlx::query(
        r#"
        UPDATE users
        SET username = ?,
            password_hash = ?,
            nickname = ?,
            must_change_credentials = 0,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(username)
    .bind(password_hash)
    .bind(username)
    .bind(now)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn reset_bootstrap_admin_password(
    pool: &DbPool,
    user_id: i64,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    let now = crate::utils::time::now_rfc3339();

    sqlx::query(
        r#"
        UPDATE users
        SET password_hash = ?,
            must_change_credentials = 1,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(password_hash)
    .bind(now)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt_bytes = rand::random::<[u8; 16]>();
    let salt = SaltString::encode_b64(&salt_bytes)?;
    let argon2 = Argon2::default();

    Ok(argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}
