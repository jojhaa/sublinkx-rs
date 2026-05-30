use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub code: &'static str,
    pub data: LoginTokenData,
}

#[derive(Debug, Serialize)]
pub struct LoginTokenData {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in_hours: i64,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub code: &'static str,
    pub data: MeData,
}

#[derive(Debug, Serialize)]
pub struct MeData {
    pub user_id: i64,
    pub username: String,
    pub nickname: String,
    pub role: String,
    pub status: String,
}
