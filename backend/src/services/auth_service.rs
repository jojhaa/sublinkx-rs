use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::http::{HeaderMap, header};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::{
    domain::user::User,
    dto::auth::{LoginRequest, LoginResponse, LoginTokenData, MeData, MeResponse},
    errors::AppError,
    repository::user_repo,
    state::AppState,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: i64,
    username: String,
    role: String,
    exp: usize,
}

pub async fn login(state: &AppState, payload: LoginRequest) -> Result<LoginResponse, AppError> {
    if payload.username.trim().is_empty() || payload.password.is_empty() {
        return Err(AppError::BadRequest(
            "username and password are required".to_string(),
        ));
    }

    let user = user_repo::find_by_username(&state.db, payload.username.trim())
        .await?
        .ok_or(AppError::Unauthorized)?;

    if user.status != "active" {
        return Err(AppError::Unauthorized);
    }

    verify_password(&payload.password, &user.password_hash)?;

    let access_token = issue_token(state, user.id, &user.username, &user.role)?;

    Ok(LoginResponse {
        code: "00000",
        data: LoginTokenData {
            access_token,
            token_type: "Bearer",
            expires_in_hours: state.config.security.jwt_exp_hours,
        },
    })
}

pub async fn current_user(state: &AppState, headers: &HeaderMap) -> Result<MeResponse, AppError> {
    let user = require_user(state, headers).await?;

    Ok(MeResponse {
        code: "00000",
        data: MeData {
            user_id: user.id,
            username: user.username,
            nickname: user.nickname,
            role: user.role,
            status: user.status,
        },
    })
}

pub async fn require_user(state: &AppState, headers: &HeaderMap) -> Result<User, AppError> {
    let claims = decode_bearer_token(headers, &state.config.security.jwt_secret)?;

    user_repo::find_by_id(&state.db, claims.sub)
        .await?
        .ok_or(AppError::Unauthorized)
}

fn verify_password(password: &str, password_hash: &str) -> Result<(), AppError> {
    let parsed_hash = PasswordHash::new(password_hash)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized)
}

fn issue_token(
    state: &AppState,
    user_id: i64,
    username: &str,
    role: &str,
) -> Result<String, AppError> {
    let expiry = OffsetDateTime::now_utc() + Duration::hours(state.config.security.jwt_exp_hours);
    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        role: role.to_string(),
        exp: expiry.unix_timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.security.jwt_secret.as_bytes()),
    )
    .map_err(AppError::from)
}

fn decode_bearer_token(headers: &HeaderMap, jwt_secret: &str) -> Result<Claims, AppError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let token = value
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(data.claims)
}
