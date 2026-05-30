use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub nickname: String,
    pub role: String,
    pub status: String,
    pub must_change_credentials: i64,
    pub created_at: String,
    pub updated_at: String,
}
