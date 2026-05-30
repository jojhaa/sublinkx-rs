use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use sqlx::SqlitePool;
use tracing::warn;

use crate::{config::SecurityConfig, domain::user::User};

pub async fn bootstrap_admin(
    pool: &SqlitePool,
    security: &SecurityConfig,
) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        return Ok(());
    }

    let password_hash = hash_password(&security.bootstrap_admin_password)
        .map_err(|_| sqlx::Error::Protocol("failed to hash bootstrap admin password".into()))?;
    let now = crate::utils::time::now_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO users (username, password_hash, nickname, role, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, 'admin', 'active', ?4, ?5)
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

pub async fn find_by_username(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, nickname, role, status, created_at, updated_at
        FROM users
        WHERE username = ?1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, nickname, role, status, created_at, updated_at
        FROM users
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt_bytes = rand::random::<[u8; 16]>();
    let salt = SaltString::encode_b64(&salt_bytes)?;
    let argon2 = Argon2::default();

    Ok(argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}
